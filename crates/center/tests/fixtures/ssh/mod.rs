use hmail_cleaner_protocol::browser::enrollment::SshTarget;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
pub mod sftp;
#[derive(Default)]
struct Handler {
    auth: Arc<AtomicUsize>,
    channels: std::collections::HashMap<russh::ChannelId, russh::Channel<russh::server::Msg>>,
    sink: Arc<sftp::Sink>,
}
impl russh::server::Handler for Handler {
    type Error = russh::Error;
    async fn channel_open_session(
        &mut self,
        channel: russh::Channel<russh::server::Msg>,
        reply: russh::server::ChannelOpenHandle,
        _: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: russh::ChannelId,
        name: &str,
        session: &mut russh::server::Session,
    ) -> Result<(), Self::Error> {
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        let channel = self.channels.remove(&id).unwrap();
        session.channel_success(id)?;
        russh_sftp::server::run_with_config(
            channel.into_stream(),
            sftp::Receiver(self.sink.clone()),
            russh_sftp::server::Config {
                max_client_packet_len: 64 * 1024,
            },
        )
        .await;
        Ok(())
    }
    async fn auth_password(
        &mut self,
        user: &str,
        password: &str,
    ) -> Result<russh::server::Auth, Self::Error> {
        self.auth.fetch_add(1, Ordering::SeqCst);
        Ok(
            if user == "Админ; &literal" && password == "SSH-PASSWORD-ONLY-IN-AUTH" {
                russh::server::Auth::Accept
            } else {
                russh::server::Auth::reject()
            },
        )
    }
}
impl russh::server::Server for Handler {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        Self {
            auth: self.auth.clone(),
            channels: Default::default(),
            sink: self.sink.clone(),
        }
    }
}
pub struct SshFixture {
    pub target: SshTarget,
    pub fingerprint: String,
    pub auth: Arc<AtomicUsize>,
    pub sink: Arc<sftp::Sink>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for SshFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub async fn start() -> SshFixture {
    use russh::server::Server;
    let key =
        russh::keys::PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519).unwrap();
    let fingerprint = key
        .public_key()
        .fingerprint(russh::keys::HashAlg::Sha256)
        .to_string();
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: std::time::Duration::ZERO,
        auth_rejection_time_initial: Some(std::time::Duration::ZERO),
        inactivity_timeout: Some(std::time::Duration::from_secs(5)),
        ..Default::default()
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut handler = Handler::default();
    let auth = handler.auth.clone();
    let sink = handler.sink.clone();
    let task = tokio::spawn(async move {
        handler.run_on_socket(config, &listener).await.unwrap();
    });
    SshFixture {
        target: SshTarget {
            host: "127.0.0.1".into(),
            port,
        },
        fingerprint,
        auth,
        sink,
        task,
    }
}
