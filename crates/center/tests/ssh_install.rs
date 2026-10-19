pub mod fixtures;
use hmail_cleaner_center::enrollment::{NativeSsh, SshTarget};
#[tokio::test]
async fn invalid_targets_are_rejected_before_connecting() {
    let center = fixtures::center().await;
    for host in [
        "",
        "server; whoami",
        "host/name",
        "user@server",
        "host\nother",
    ] {
        assert!(NativeSsh::probe(
            SshTarget {
                host: host.into(),
                port: 22
            },
            &center.lease
        )
        .await
        .is_err());
    }
}
#[tokio::test]
async fn host_key_probe_is_unauthenticated_and_changed_pin_blocks_credentials() {
    use hmail_cleaner_protocol::browser::enrollment::SshCredentials;
    use std::sync::atomic::Ordering;
    let center = fixtures::center().await;
    let remote = fixtures::ssh::start().await;
    let challenge = NativeSsh::probe(remote.target.clone(), &center.lease)
        .await
        .unwrap();
    assert_eq!(challenge.fingerprint, remote.fingerprint);
    assert_eq!(remote.auth.load(Ordering::SeqCst), 0);
    let wrong = "SHA256:".to_owned() + &"A".repeat(43);
    let credentials = || SshCredentials::Password {
        username: "Админ; &literal".into(),
        password: zeroize::Zeroizing::new("SSH-PASSWORD-ONLY-IN-AUTH".into()),
    };
    assert!(
        NativeSsh::connect(remote.target.clone(), wrong, credentials(), &center.lease)
            .await
            .is_err()
    );
    assert_eq!(remote.auth.load(Ordering::SeqCst), 0);
    let session = NativeSsh::connect(
        remote.target.clone(),
        challenge.fingerprint,
        credentials(),
        &center.lease,
    )
    .await
    .unwrap();
    assert_eq!(remote.auth.load(Ordering::SeqCst), 1);
    session.close().await.unwrap();
}
#[tokio::test]
async fn sftp_stream_is_bounded_and_propagates_a_partial_write_failure() {
    use hmail_cleaner_center::enrollment::remote::Remote;
    use hmail_cleaner_protocol::browser::enrollment::SshCredentials;
    use std::sync::atomic::Ordering;
    let center = fixtures::center().await;
    let package = fixtures::enrollment::packages();
    for fail_after in [usize::MAX, 1] {
        let remote = fixtures::ssh::start().await;
        remote.sink.fail_after.store(fail_after, Ordering::SeqCst);
        let session = NativeSsh::connect(
            remote.target.clone(),
            remote.fingerprint.clone(),
            SshCredentials::Password {
                username: "Админ; &literal".into(),
                password: zeroize::Zeroizing::new("SSH-PASSWORD-ONLY-IN-AUTH".into()),
            },
            &center.lease,
        )
        .await
        .unwrap();
        let mut file = package
            .catalog
            .open(fixtures::enrollment::BUILD)
            .await
            .unwrap();
        let result = session
            .upload(
                "C:/ProgramData/hMailCleanerAgentStage/fixture.exe",
                &mut file,
            )
            .await;
        if fail_after == usize::MAX {
            result.unwrap();
            assert_eq!(remote.sink.bytes.load(Ordering::SeqCst), 100_000);
            assert_eq!(remote.sink.digest(), file.metadata().sha256);
        } else {
            assert!(result.is_err());
            assert!(remote.sink.bytes.load(Ordering::SeqCst) < 100_000);
        }
        assert!(remote.sink.max_write.load(Ordering::SeqCst) <= 32 * 1024);
        session.close().await.unwrap();
    }
}
#[tokio::test]
async fn http_probe_requires_session_and_csrf_and_never_authenticates_the_os_user() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use std::sync::atomic::Ordering;
    use tower::ServiceExt;
    let center = fixtures::center().await;
    let remote = fixtures::ssh::start().await;
    let mut state = hmail_cleaner_center::http::AppState::new(
        center.store.clone(),
        hmail_cleaner_center::config::BrowserOrigin::parse("https://localhost:8443").unwrap(),
    )
    .await
    .unwrap();
    state = state.with_authority(center.authority.clone());
    let app = hmail_cleaner_center::http::router(state);
    for (cookie, csrf, expected) in [(false, true, 401), (true, false, 403), (true, true, 200)] {
        let request = Request::post("/api/v1/enrollment/probe")
            .header("host", "localhost:8443")
            .header("content-type", "application/json")
            .header("origin", "https://localhost:8443");
        let request = if cookie {
            request.header(
                "cookie",
                format!("__Host-hmail-session={}", center.token.as_str()),
            )
        } else {
            request
        };
        let request = if csrf {
            request.header("x-hmail-request", "1")
        } else {
            request
        };
        let response = app
            .clone()
            .oneshot(
                request
                    .body(Body::from(serde_json::to_vec(&remote.target).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), expected);
        assert_eq!(response.headers()["cache-control"], "no-store");
        if expected == 200 {
            let bytes = to_bytes(response.into_body(), 8192).await.unwrap();
            let challenge: hmail_cleaner_protocol::browser::enrollment::HostKeyChallenge =
                serde_json::from_slice(&bytes).unwrap();
            assert_eq!(challenge.fingerprint, remote.fingerprint);
        }
    }
    assert_eq!(remote.auth.load(Ordering::SeqCst), 0);
}
