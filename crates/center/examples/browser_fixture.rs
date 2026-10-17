#[path = "browser_fixture/control.rs"]
mod control;
#[path = "../tests/fixtures/mod.rs"]
pub mod fixtures;
#[path = "browser_fixture/setup.rs"]
mod setup;
use base64ct::{Base64, Encoding};
use hmail_cleaner_center::{
    app::AppState,
    config::{CenterArgs, CenterConfig},
    http::static_files::StaticDir,
};
use rustls::pki_types::pem::PemObject;
use std::{
    io::{BufRead, Write},
    sync::Arc,
};
#[tokio::main]
async fn main() {
    let static_dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("absolute static build directory"),
    );
    let f = fixtures::center().await;
    let (first, one) = fixtures::job_agent::start(&f).await;
    let (second, two) = fixtures::job_agent::start(&f).await;
    let first_server = fixtures::jobs::configured_named(
        &f,
        first.credentials.agent_id,
        first.credentials.endpoint.as_str(),
        "Mail One",
    )
    .await;
    let second_server = setup::unconfigured(
        &f,
        second.credentials.agent_id,
        second.credentials.endpoint.as_str(),
    )
    .await;
    let history_count = std::env::args()
        .nth(2)
        .map(|value| value.parse::<usize>().expect("fixture history row count"))
        .unwrap_or(0);
    setup::history(&f, &first_server, history_count).await;
    let packages = fixtures::enrollment::packages();
    one.restore_on_cancel();
    two.restore_on_cancel();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("https://localhost:{}", address.port());
    CenterConfig::create(f.root.path(), &origin, address).unwrap();
    let config = CenterConfig::load(&CenterArgs {
        data_dir: f.root.path().to_path_buf(),
    })
    .unwrap();
    let mut state = AppState::new(f.store.clone(), config.origin.clone())
        .await
        .unwrap()
        .with_authority(f.authority.clone())
        .with_static_dir(StaticDir::open(&static_dir).unwrap());
    state.enrollment = hmail_cleaner_center::enrollment::EnrollmentService::new(
        f.store.clone(),
        packages.catalog.clone(),
        hmail_cleaner_center::enrollment::NativeSsh,
    );
    let cancel = Arc::new(hmail_cleaner_core::cleanup::Cancellation::default());
    f.authority.revoke(f.lease.generation());
    let server_cancel = cancel.clone();
    let server = tokio::spawn(async move {
        hmail_cleaner_center::app::serve(listener, &config, state, server_cancel)
            .await
            .unwrap();
    });
    let ca = std::fs::read_to_string(f.root.path().join("ca-cert.pem")).unwrap();
    let ca_der = rustls::pki_types::CertificateDer::from_pem_slice(ca.as_bytes()).unwrap();
    let (_, certificate) = x509_parser::parse_x509_certificate(&ca_der).unwrap();
    let pin = Base64::encode_string(
        ring::digest::digest(&ring::digest::SHA256, certificate.public_key().raw).as_ref(),
    );
    let ready = serde_json::json!({"origin":origin,"spki":pin,"servers":[{"id":first_server.server_id,"name":"Mail One","agent_id":first_server.agent_id,"profile_id":first_server.profile_id},{"id":second_server.server_id,"name":"Mail Two","agent_id":second_server.agent_id,"profile_id":second_server.profile_id}]});
    println!("{ready}");
    std::io::stdout().flush().unwrap();
    let (sender, mut commands) = tokio::sync::mpsc::channel(8);
    let reader = std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if sender.blocking_send(line).is_err() {
                break;
            }
        }
    });
    let agents = [
        (first_server.server_id, one),
        (second_server.server_id, two),
    ];
    while let Some(line) = commands.recv().await {
        let response = control::execute(&agents, &line);
        println!("{response}");
        std::io::stdout().flush().unwrap();
    }
    cancel.request();
    let _ = server.await;
    let _ = reader.join();
    drop((first, second));
    drop(packages);
}
