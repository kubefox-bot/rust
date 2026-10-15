use hmail_cleaner_core::{
    cleanup::{Cancellation, PortResult},
    maintenance::mail_service::*,
};
use std::{sync::Mutex, time::Duration};
struct Control {
    state: Mutex<MailState>,
    trace: Mutex<Vec<&'static str>>,
    reject_stop: bool,
    fail_wait: bool,
    fail_restore: bool,
}
impl Control {
    fn new(state: MailState) -> Self {
        Self {
            state: Mutex::new(state),
            trace: Mutex::new(vec![]),
            reject_stop: false,
            fail_wait: false,
            fail_restore: false,
        }
    }
}
impl MailControl for Control {
    async fn state(&self) -> PortResult<MailState> {
        Ok(*self.state.lock().unwrap())
    }
    async fn stop(&self) -> PortResult<()> {
        self.trace.lock().unwrap().push("stop");
        if self.reject_stop {
            return Err("stop denied".into());
        }
        *self.state.lock().unwrap() = MailState::Transitioning;
        Ok(())
    }
    async fn wait_for_stopped(&self, timeout: Duration) -> PortResult<()> {
        assert_eq!(timeout.as_secs(), 60);
        self.trace.lock().unwrap().push("wait");
        if self.fail_wait {
            return Err("stop timeout".into());
        }
        *self.state.lock().unwrap() = MailState::Stopped;
        Ok(())
    }
    async fn restore_running(&self, timeout: Duration) -> PortResult<()> {
        assert_eq!(timeout.as_secs(), 60);
        self.trace.lock().unwrap().push("restore");
        if self.fail_restore {
            return Err("restore timeout".into());
        }
        *self.state.lock().unwrap() = MailState::Running;
        Ok(())
    }
}
#[tokio::test]
async fn stopped_before_run_is_not_started() {
    let control = Control::new(MailState::Stopped);
    let result = with_stopped(&control, &Cancellation::default(), || async { Ok(7) }).await;
    assert_eq!(result.result.unwrap(), 7);
    assert_eq!(*control.state.lock().unwrap(), MailState::Stopped);
    assert!(control.trace.lock().unwrap().is_empty());
}
#[tokio::test]
async fn running_mail_is_restored_after_success_error_and_cancel() {
    for mode in 0..3 {
        let control = Control::new(MailState::Running);
        let cancel = Cancellation::default();
        let result = with_stopped(&control, &cancel, || async {
            assert_eq!(*control.state.lock().unwrap(), MailState::Stopped);
            control.trace.lock().unwrap().push("work");
            if mode == 2 {
                cancel.request();
            }
            if mode == 1 {
                Err("cleanup failed".into())
            } else {
                Ok(7)
            }
        })
        .await;
        assert_eq!(result.result.is_err(), mode == 1);
        assert!(result.restore_error.is_none());
        assert_eq!(*control.state.lock().unwrap(), MailState::Running);
        assert_eq!(
            *control.trace.lock().unwrap(),
            vec!["stop", "wait", "work", "restore"]
        );
    }
}
#[tokio::test]
async fn stop_rejected_keeps_running() {
    let control = Control {
        reject_stop: true,
        ..Control::new(MailState::Running)
    };
    let result: ManagedResult<()> = with_stopped(&control, &Cancellation::default(), || async {
        panic!("must not run");
    })
    .await;
    assert!(result.result.is_err());
    assert!(result.restore_error.is_none());
    assert_eq!(*control.state.lock().unwrap(), MailState::Running);
    assert_eq!(*control.trace.lock().unwrap(), vec!["stop"]);
}
#[tokio::test]
async fn accepted_stop_timeout_still_attempts_restore() {
    let control = Control {
        fail_wait: true,
        ..Control::new(MailState::Running)
    };
    let result = with_stopped(&control, &Cancellation::default(), || async { Ok(()) }).await;
    assert!(result
        .result
        .unwrap_err()
        .to_string()
        .contains("stop timeout"));
    assert_eq!(*control.state.lock().unwrap(), MailState::Running);
    assert_eq!(
        *control.trace.lock().unwrap(),
        vec!["stop", "wait", "restore"]
    );
}
#[tokio::test]
async fn restore_timeout_preserves_cleanup_error() {
    let control = Control {
        fail_restore: true,
        ..Control::new(MailState::Running)
    };
    let result: ManagedResult<()> = with_stopped(&control, &Cancellation::default(), || async {
        Err("cleanup failed".into())
    })
    .await;
    assert!(result
        .result
        .unwrap_err()
        .to_string()
        .contains("cleanup failed"));
    assert!(result
        .restore_error
        .unwrap()
        .to_string()
        .contains("restore timeout"));
}
#[tokio::test]
async fn cancelled_preflight_never_stops_mail() {
    let control = Control::new(MailState::Running);
    let cancel = Cancellation::default();
    cancel.request();
    let result = with_stopped(&control, &cancel, || async { Ok(()) }).await;
    assert!(result.result.is_err());
    assert!(control.trace.lock().unwrap().is_empty());
}
