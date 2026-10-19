//! Local resource fixture. The measured center is a separate native release process.
#[path = "resource_fixture/control.rs"]
mod control;
#[path = "../tests/fixtures/mod.rs"]
pub mod fixtures;
#[path = "resource_fixture/history.rs"]
mod history;
#[path = "resource_fixture/setup.rs"]
mod setup;
use base64ct::{Base64, Encoding};
use hmail_cleaner_center::{config::CenterConfig, storage::DataDirectory};
use rustls::pki_types::pem::PemObject;
use std::{
    io::{BufRead, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
};
struct CenterProcess(Child);
impl Drop for CenterProcess {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn repository_artifact(root: &std::path::Path, relative: &str, provided: PathBuf) -> PathBuf {
    let expected = root.join(relative);
    assert_eq!(provided.canonicalize().unwrap(), expected);
    assert_eq!(expected.canonicalize().unwrap(), expected);
    expected
}
#[tokio::main]
async fn main() {
    let root = std::env::current_dir().unwrap().canonicalize().unwrap();
    let mut arguments = std::env::args().skip(1);
    let binary = repository_artifact(
        &root,
        "target/release/hmail-cleaner",
        PathBuf::from(arguments.next().expect("native release center path")),
    );
    let static_dir = repository_artifact(
        &root,
        "apps/web/dist",
        PathBuf::from(arguments.next().expect("static build path")),
    );
    let temporary_parent = PathBuf::from(arguments.next().expect("owned temporary fixture parent"));
    let temporary_parent = temporary_parent.canonicalize().unwrap();
    assert!(temporary_parent.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(temporary_parent
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("hmail-mvp-synthetic-resource-owner-"));
    assert!(
        arguments.next().is_none(),
        "No arbitrary fixture options are accepted"
    );
    let center = setup::center(&temporary_parent).await;
    let (first, one) = fixtures::job_agent::start(&center).await;
    let (second, two) = fixtures::job_agent::start(&center).await;
    let first_server = setup::server(
        &center,
        first.credentials.agent_id,
        first.credentials.endpoint.as_str(),
        "One",
    )
    .await;
    let second_server = setup::server(
        &center,
        second.credentials.agent_id,
        second.credentials.endpoint.as_str(),
        "Two",
    )
    .await;
    history::seed(&center, &first_server).await;
    one.restore_on_cancel();
    two.restore_on_cancel();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("https://localhost:{}", address.port());
    CenterConfig::create(center.root.path(), &origin, address).unwrap();
    let configuration_path = center.root.path().join("center.json");
    let mut configuration: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&configuration_path).unwrap()).unwrap();
    configuration["static_dir"] = serde_json::json!(static_dir);
    std::fs::write(
        configuration_path,
        serde_json::to_vec(&configuration).unwrap(),
    )
    .unwrap();
    let fixture_id = uuid::Uuid::new_v4();
    std::fs::create_dir(center.root.path().join("synthetic-mail")).unwrap();
    std::fs::write(
        center.root.path().join(".hmail-mvp-fixture.json"),
        serde_json::to_vec(&serde_json::json!({"fixture_id":fixture_id,"kind":"synthetic-only"}))
            .unwrap(),
    )
    .unwrap();
    let ca = std::fs::read_to_string(center.root.path().join("ca-cert.pem")).unwrap();
    let ca_der = rustls::pki_types::CertificateDer::from_pem_slice(ca.as_bytes()).unwrap();
    let (_, certificate) = x509_parser::parse_x509_certificate(&ca_der).unwrap();
    let pin = Base64::encode_string(
        ring::digest::digest(&ring::digest::SHA256, certificate.public_key().raw).as_ref(),
    );
    let fixtures::CenterFixture {
        root: temporary_root,
        store,
        authority,
        lease,
        token,
        recovery,
    } = center;
    authority.revoke(lease.generation());
    drop((store, authority, lease, token, recovery));
    // Prove fixture preparation no longer owns the data lock before launching the center.
    drop(DataDirectory::acquire(temporary_root.path()).unwrap());
    drop(listener);
    let process = CenterProcess(
        Command::new(binary)
            .args(["center", "run", "--data-dir"])
            .arg(temporary_root.path())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    let ready = serde_json::json!({"origin":origin,"spki":pin,"center_pid":process.0.id(),"fixture_id":fixture_id,"root":temporary_root.path(),"servers":[{"id":first_server.server_id,"name":first_server.name,"agent_id":first_server.agent_id,"profile_id":first_server.profile_id},{"id":second_server.server_id,"name":second_server.name,"agent_id":second_server.agent_id,"profile_id":second_server.profile_id}]});
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
        (first_server.server_id, first, one),
        (second_server.server_id, second, two),
    ];
    while let Some(line) = commands.recv().await {
        let response = control::execute(&agents, &line);
        println!("{response}");
        std::io::stdout().flush().unwrap();
    }
    let _ = reader.join();
    drop(process);
    drop(agents);
    drop(temporary_root);
}
