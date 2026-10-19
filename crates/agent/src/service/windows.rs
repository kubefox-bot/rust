use super::{InstallError, Presence, ServiceRegistration, ServiceRegistry, ServiceState};
use std::{
    ffi::OsString,
    os::windows::ffi::OsStrExt,
    time::{Duration, Instant},
};
use windows_service::{
    service::{
        Service, ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType,
        ServiceState as NativeState, ServiceType,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};
const SERVICE_MISSING: i32 = 1060;
const STOP_TIMEOUT: Duration = Duration::from_secs(120);
const START_TIMEOUT: Duration = Duration::from_secs(30);
const STATUS_POLL: Duration = Duration::from_millis(100);
const LOCAL_SYSTEM_ACCOUNTS: &[&str] = &["LocalSystem", r"NT AUTHORITY\SYSTEM"];

pub(super) struct WindowsRegistry(ServiceManager);
impl WindowsRegistry {
    pub fn new() -> Result<Self, InstallError> {
        ServiceManager::local_computer(
            None::<&str>,
            ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE,
        )
        .map(Self)
        .map_err(|_| InstallError::Native)
    }
    fn open(&self, registration: &ServiceRegistration) -> Result<Option<Service>, InstallError> {
        let access = ServiceAccess::QUERY_CONFIG
            | ServiceAccess::QUERY_STATUS
            | ServiceAccess::CHANGE_CONFIG
            | ServiceAccess::START
            | ServiceAccess::STOP;
        match self.0.open_service(&registration.name, access) {
            Ok(service) => Ok(Some(service)),
            Err(windows_service::Error::Winapi(error))
                if error.raw_os_error() == Some(SERVICE_MISSING) =>
            {
                Ok(None)
            }
            Err(_) => Err(InstallError::Native),
        }
    }
    fn owned(&self, registration: &ServiceRegistration) -> Result<Service, InstallError> {
        if !matches!(self.inspect(registration)?, Presence::Owned(_)) {
            return Err(InstallError::Collision);
        }
        self.open(registration)?.ok_or(InstallError::Collision)
    }
}
fn arguments(command: &std::path::Path) -> Result<Vec<OsString>, InstallError> {
    use windows_sys::Win32::{Foundation::LocalFree, UI::Shell::CommandLineToArgvW};
    let wide = command
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut count = 0;
    // Windows owns the returned null-terminated strings in one LocalAlloc block.
    let pointer = unsafe { CommandLineToArgvW(wide.as_ptr(), &mut count) };
    if pointer.is_null() {
        return Err(InstallError::Native);
    }
    let result = if count < 1 {
        Err(InstallError::Invalid)
    } else {
        // The API guarantees count valid pointers; copy before releasing its block.
        let items = unsafe { std::slice::from_raw_parts(pointer, count as usize) };
        Ok(items
            .iter()
            .map(|item| unsafe { widestring::U16CStr::from_ptr_str(*item).to_os_string() })
            .collect())
    };
    unsafe {
        LocalFree(pointer.cast());
    }
    result
}
fn wait(service: &Service, state: NativeState, timeout: Duration) -> Result<(), InstallError> {
    let deadline = Instant::now() + timeout;
    loop {
        let actual = service
            .query_status()
            .map_err(|_| InstallError::Native)?
            .current_state;
        if actual == state {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(InstallError::Native);
        }
        std::thread::sleep(STATUS_POLL);
    }
}
fn info(registration: &ServiceRegistration) -> ServiceInfo {
    ServiceInfo {
        name: registration.name.clone().into(),
        display_name: registration.name.clone().into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: registration.executable.clone(),
        launch_arguments: registration.arguments.clone(),
        dependencies: vec![],
        account_name: None,
        account_password: None,
    }
}
impl ServiceRegistry for WindowsRegistry {
    fn inspect(&self, registration: &ServiceRegistration) -> Result<Presence, InstallError> {
        let Some(service) = self.open(registration)? else {
            return Ok(Presence::Absent);
        };
        let config = service.query_config().map_err(|_| InstallError::Native)?;
        if registration.name == super::SERVICE_NAME
            && !config.account_name.as_ref().is_some_and(|account| {
                LOCAL_SYSTEM_ACCOUNTS
                    .iter()
                    .any(|expected| account.to_string_lossy().eq_ignore_ascii_case(expected))
            })
        {
            return Ok(Presence::Collision);
        }
        let expected = std::iter::once(registration.executable.as_os_str().to_owned())
            .chain(registration.arguments.iter().cloned())
            .collect::<Vec<_>>();
        if config.service_type != ServiceType::OWN_PROCESS
            || arguments(&config.executable_path)? != expected
        {
            return Ok(Presence::Collision);
        }
        let state = match service
            .query_status()
            .map_err(|_| InstallError::Native)?
            .current_state
        {
            NativeState::Running => ServiceState::Running,
            NativeState::Stopped => ServiceState::Stopped,
            _ => ServiceState::Transitioning,
        };
        Ok(Presence::Owned(state))
    }
    fn stop_and_wait(&self, registration: &ServiceRegistration) -> Result<(), InstallError> {
        let service = self.owned(registration)?;
        if service
            .query_status()
            .map_err(|_| InstallError::Native)?
            .current_state
            == NativeState::Running
        {
            service.stop().map_err(|_| InstallError::Native)?;
        }
        wait(&service, NativeState::Stopped, STOP_TIMEOUT)
    }
    fn disable(&self, registration: &ServiceRegistration) -> Result<(), InstallError> {
        use windows_sys::Win32::System::Services::{
            ChangeServiceConfigW, SERVICE_DISABLED, SERVICE_NO_CHANGE,
        };
        let service = self.owned(registration)?;
        // Change only startup type. Null pointers preserve credentials, arguments,
        // dependencies and all other settings of the recognized legacy service.
        let success = unsafe {
            ChangeServiceConfigW(
                service.raw_handle(),
                SERVICE_NO_CHANGE,
                SERVICE_DISABLED,
                SERVICE_NO_CHANGE,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if success == 0 {
            return Err(InstallError::Native);
        }
        Ok(())
    }
    fn configure_and_start(&self, registration: &ServiceRegistration) -> Result<(), InstallError> {
        let service = match self.inspect(registration)? {
            Presence::Collision
            | Presence::Owned(ServiceState::Running | ServiceState::Transitioning) => {
                return Err(InstallError::Collision)
            }
            Presence::Owned(ServiceState::Stopped) => {
                let service = self.owned(registration)?;
                service
                    .change_config(&info(registration))
                    .map_err(|_| InstallError::Native)?;
                service
            }
            Presence::Absent => self
                .0
                .create_service(
                    &info(registration),
                    ServiceAccess::QUERY_STATUS
                        | ServiceAccess::START
                        | ServiceAccess::CHANGE_CONFIG,
                )
                .map_err(|_| InstallError::Native)?,
        };
        service
            .set_description("Manual hMailServer cleanup agent; protocol v1")
            .map_err(|_| InstallError::Native)?;
        service
            .start::<&str>(&[])
            .map_err(|_| InstallError::Native)?;
        wait(&service, NativeState::Running, START_TIMEOUT)
    }
}
