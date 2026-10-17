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
    let listener = std::net::TcpListener::bind(config.listen)?;
    run_bound(config, cancellation, listener).await
}
pub async fn run_bound(
    config: AgentConfig,
    cancellation: Arc<Cancellation>,
    listener: std::net::TcpListener,
) -> Result<(), AgentError> {
    if listener.local_addr()? != config.listen {
        return Err(AgentError::Invalid);
    }
    let _process_owner = crate::runtime_lock::acquire(&config.data_directory)?;
    let repository =
        Arc::new(JobRepository::open(&config.data_directory.join("jobs.sqlite")).await?);
    repository.recover_interrupted().await?;
    if repository.in_maintenance().await? {
        return Err(AgentError::Unavailable);
    }
    let identity = Arc::new(crate::identity::IdentityManager::new(
        &config,
        repository.clone(),
    ));
    let worker = Worker::configured(
        config.agent_id,
        repository,
        NativeExecutor,
        config.build_version,
    );
    let handle = axum_server::Handle::new();
    let installation = crate::http::installation::InstallationReader {
        directory: config.data_directory.clone(),
        executable: config.executable,
    };
    let server = http::transport::server(listener, config.tls, handle.clone())?;
    let serving = server.serve(
        http::router(worker.clone())
            .layer(axum::Extension(identity))
            .layer(axum::Extension(installation))
            .into_make_service(),
    );
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
