use hmail_cleaner_core::cleanup::{Cancellation, PortResult};
use std::sync::Arc;
use windows_service::{
    service::ServiceControl,
    service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle},
};
pub(super) fn register(name: &str, stop: Arc<Cancellation>) -> PortResult<ServiceStatusHandle> {
    Ok(service_control_handler::register(
        name,
        move |event| match event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stop.request();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        },
    )?)
}
