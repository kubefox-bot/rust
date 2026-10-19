#![cfg(unix)]
use astra_updater::{
    staging::{self, Host},
    Error,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
struct Disk {
    consumed: AtomicBool,
    events: Mutex<Vec<&'static str>>,
}
impl Host for Disk {
    async fn preflight(&self) -> Result<(), Error> {
        self.events.lock().unwrap().push("space");
        if self.consumed.load(Ordering::SeqCst) {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
    async fn pull(&self) -> Result<(), Error> {
        self.events.lock().unwrap().push("pull");
        self.consumed.store(true, Ordering::SeqCst);
        Ok(())
    }
}
#[tokio::test]
async fn image_consuming_backup_headroom_is_rejected_before_admission() {
    let disk = Disk {
        consumed: AtomicBool::new(false),
        events: Mutex::new(vec![]),
    };
    let result = staging::run(&disk).await;
    if result.is_ok() {
        disk.events.lock().unwrap().push("begin");
    }
    assert!(matches!(result, Err(Error::Storage)));
    assert_eq!(*disk.events.lock().unwrap(), vec!["space", "pull", "space"]);
}
