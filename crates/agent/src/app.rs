use crate::{
    config::AgentConfig,
    execution::{AgentError, NativeExecutor, Worker},
    http,
    jobs::JobRepository,
};
use hmail_cleaner_core::cleanup::Cancellation;
use std::{sync::Arc, time::Duration};
const SHUTDOWN_POLL: Duration = Duration::from_millis(250);
const HTTP_DRAIN: Duration = Duration::from_secs(1);

pub async fn run(config: AgentConfig, cancellation: Arc<Cancellation>) -> Result<(), AgentError> {
    let repository =
        Arc::new(JobRepository::open(&config.data_directory.join("jobs.sqlite")).await?);
    repository.recover_interrupted().await?;
    let worker = Worker::new(config.agent_id, repository, NativeExecutor);
    let handle = axum_server::Handle::new();
    let listener = std::net::TcpListener::bind(config.listen)?;
    let server = http::transport::server(listener, config.tls, handle.clone())?;
    let serving = server.serve(http::router(worker.clone()).into_make_service());
    tokio::pin!(serving);
    loop {
        tokio::select! {
            result = &mut serving => { worker.shutdown().await; return result.map_err(Into::into); }
            _ = tokio::time::sleep(SHUTDOWN_POLL) => {
                if cancellation.is_requested() {
                    worker.shutdown().await;
                    handle.graceful_shutdown(Some(HTTP_DRAIN));
                    return serving.await.map_err(Into::into);
                }
            }
        }
    }
}
