use super::{handler, status, ServiceJob};
use hmail_cleaner_core::cleanup::{Cancellation, PortResult};
use std::{
    ffi::OsString,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{mpsc, Arc, Mutex, OnceLock},
    time::Duration,
};
use windows_service::{define_windows_service, service::ServiceState, service_dispatcher};
struct Context {
    name: String,
    job: ServiceJob,
    outcome: Mutex<Option<Result<(), String>>>,
}
static CONTEXT: OnceLock<Context> = OnceLock::new();
define_windows_service!(ffi_main, service_main);
pub(super) fn host(name: &str, job: ServiceJob) -> PortResult<()> {
    CONTEXT
        .set(Context {
            name: name.to_string(),
            job,
            outcome: Mutex::new(None),
        })
        .map_err(|_| "SCM host can only start once per process")?;
    service_dispatcher::start(name, ffi_main)?;
    let context = CONTEXT.get().expect("registered SCM context");
    match context
        .outcome
        .lock()
        .map_err(|_| "SCM outcome poisoned")?
        .take()
    {
        Some(Ok(())) => Ok(()),
        Some(Err(error)) => Err(error.into()),
        None => Err("SCM callback did not finish".into()),
    }
}
fn service_main(_arguments: Vec<OsString>) {
    let context = CONTEXT.get().expect("registered SCM context");
    let result = run(context);
    if let Ok(mut outcome) = context.outcome.lock() {
        *outcome = Some(result.map_err(|error| error.to_string()));
    }
}
fn run(context: &'static Context) -> PortResult<()> {
    let stop = Arc::new(Cancellation::default());
    let handle = handler::register(&context.name, stop.clone())?;
    let result = run_worker(context, stop, &handle);
    let stopped = status::set(&handle, ServiceState::Stopped, 0, result.is_err());
    result?;
    stopped
}
fn run_worker(
    context: &'static Context,
    stop: Arc<Cancellation>,
    handle: &windows_service::service_control_handler::ServiceStatusHandle,
) -> PortResult<()> {
    status::set(handle, ServiceState::StartPending, 1, false)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker_stop = stop.clone();
    let worker = std::thread::Builder::new()
        .name("hmail-cleaner-job".into())
        .spawn(move || {
            let result = catch_unwind(AssertUnwindSafe(|| (context.job)(worker_stop)))
                .unwrap_or_else(|_| Err("SCM job panicked".into()));
            let _ = sender.send(result);
        })?;
    let mut status_error = status::set(handle, ServiceState::Running, 0, false).err();
    if status_error.is_some() {
        stop.request();
    }
    let mut checkpoint = 0u32;
    let result = loop {
        match receiver.recv_timeout(Duration::from_secs(1)) {
            Ok(result) => break result,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break Err("SCM worker disconnected".into())
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if stop.is_requested() && status_error.is_none() {
            checkpoint = checkpoint.saturating_add(1);
            if let Err(error) = status::set(handle, ServiceState::StopPending, checkpoint, false) {
                status_error = Some(error);
            }
        }
    };
    // Never report Stopped while cleanup or mail restoration is still owned by the worker.
    let joined = worker
        .join()
        .map_err(|_| "SCM worker panicked while returning");
    joined?;
    result?;
    if let Some(error) = status_error {
        return Err(error);
    }
    Ok(())
}
