use std::io;

#[cfg(unix)]
pub(super) async fn wait() -> io::Result<()> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result,
        result = terminate.recv() => result.ok_or_else(|| io::Error::other("termination signal stream closed")),
    }
}
#[cfg(not(unix))]
pub(super) async fn wait() -> io::Result<()> {
    tokio::signal::ctrl_c().await
}
