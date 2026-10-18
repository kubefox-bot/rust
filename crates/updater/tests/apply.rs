#![cfg(unix)]
use astra_updater::{
    apply::{run, Host, Outcome},
    control::State,
    journal::{Phase, Transaction},
    Error,
};
use hmail_cleaner_protocol::browser::updates::UpdatePolicy;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
    time::Duration,
};
use uuid::Uuid;
struct Fixture {
    events: Mutex<Vec<String>>,
    fail_health: bool,
    fail_expose: bool,
    busy: bool,
    fail_phase: Option<Phase>,
    lose_abort_reply: AtomicBool,
    data: AtomicUsize,
}
impl Fixture {
    fn new() -> Self {
        Self {
            events: Mutex::new(vec![]),
            fail_health: false,
            fail_expose: false,
            busy: false,
            fail_phase: None,
            lose_abort_reply: AtomicBool::new(false),
            data: AtomicUsize::new(0),
        }
    }
    fn event(&self, event: &str) {
        self.events.lock().unwrap().push(event.into());
    }
}
impl Host for Fixture {
    fn record(&self, tx: &Transaction) -> Result<(), Error> {
        self.event(&format!("{:?}", tx.phase));
        if self.fail_phase == Some(tx.phase) {
            Err(Error::Storage)
        } else {
            Ok(())
        }
    }
    fn graceful(&self) -> Duration {
        Duration::from_millis(1)
    }
    async fn begin(&self, _: &Transaction) -> Result<State, Error> {
        self.event("begin");
        Ok(if self.busy {
            State::Draining
        } else {
            State::Ready
        })
    }
    async fn status(&self, _: &Transaction) -> Result<State, Error> {
        Ok(State::Draining)
    }
    async fn abort(&self, _: &Transaction) -> Result<(), Error> {
        self.event("abort");
        if self.lose_abort_reply.swap(false, Ordering::SeqCst) {
            self.data.store(99, Ordering::SeqCst); // A write accepted after gate removal.
            return Err(Error::Transport);
        }
        Ok(())
    }
    async fn stop(&self) -> Result<(), Error> {
        self.event("stop");
        Ok(())
    }
    async fn backup(&self, _: &Transaction) -> Result<(), Error> {
        self.event("backup");
        Ok(())
    }
    async fn start_candidate(&self, _: &Transaction) -> Result<(), Error> {
        self.event("start");
        Ok(())
    }
    async fn verify(&self, _: &Transaction) -> Result<(), Error> {
        self.event("health");
        if self.fail_health {
            Err(Error::Verification)
        } else {
            Ok(())
        }
    }
    async fn expose(&self, _: &Transaction) -> Result<(), Error> {
        self.event("open");
        if self.fail_expose {
            Err(Error::Transport)
        } else {
            Ok(())
        }
    }
    async fn restore(&self, _: &Transaction) -> Result<(), Error> {
        self.event("restore");
        self.data.store(0, Ordering::SeqCst);
        Ok(())
    }
    async fn restart_previous(&self, _: &Transaction) -> Result<(), Error> {
        self.event("previous");
        Ok(())
    }
    fn quarantine(&self, _: &Transaction) -> Result<(), Error> {
        self.event("quarantine");
        Ok(())
    }
    fn commit(&self, _: &Transaction) -> Result<(), Error> {
        self.event("commit");
        Ok(())
    }
}
fn transaction() -> Transaction {
    Transaction {
        version: 1,
        id: Uuid::new_v4(),
        phase: Phase::Staged,
        previous_image: "old".into(),
        previous_build: "old".into(),
        candidate_image: "new".into(),
        candidate_build: "new".into(),
        sequence: 2,
        backup: "/tmp/backup".into(),
        candidate_export: "/tmp/export".into(),
        previous_unit: "/tmp/unit".into(),
    }
}
#[tokio::test]
async fn success_orders_drain_backup_health_and_durable_exposure() {
    let host = Fixture::new();
    let mut tx = transaction();
    assert_eq!(
        run(&host, &mut tx, UpdatePolicy::default()).await.unwrap(),
        Outcome::Committed
    );
    let events = host.events.lock().unwrap();
    let index = |value: &str| events.iter().position(|event| event == value).unwrap();
    assert!(index("backup") < index("start"));
    assert!(index("health") < index("Exposing"));
    assert!(index("Exposing") < index("open"));
    assert!(index("open") < index("commit"));
}
#[tokio::test]
async fn busy_deadline_defers_without_stopping_current_work() {
    let mut host = Fixture::new();
    host.busy = true;
    let mut tx = transaction();
    assert_eq!(
        run(&host, &mut tx, UpdatePolicy::default()).await.unwrap(),
        Outcome::Deferred
    );
    let events = host.events.lock().unwrap();
    assert!(!events.iter().any(|event| event == "stop"));
    assert!(events.iter().any(|event| event == "abort"));
}
#[tokio::test]
async fn health_failure_restores_but_lost_expose_reply_never_does() {
    let mut host = Fixture::new();
    host.fail_health = true;
    let mut tx = transaction();
    assert_eq!(
        run(&host, &mut tx, UpdatePolicy::default()).await.unwrap(),
        Outcome::Restored
    );
    assert!(host
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|value| value == "restore"));
    let mut host = Fixture::new();
    host.fail_expose = true;
    let mut tx = transaction();
    assert!(run(&host, &mut tx, UpdatePolicy::default()).await.is_err());
    assert_eq!(tx.phase, Phase::Exposing);
    assert!(!host
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|value| value == "restore"));
}
#[tokio::test]
async fn journal_failure_prevents_the_next_side_effect() {
    for (phase, forbidden) in [
        (Phase::Stopped, "stop"),
        (Phase::Starting, "start"),
        (Phase::Exposing, "open"),
    ] {
        let mut host = Fixture::new();
        host.fail_phase = Some(phase);
        let mut tx = transaction();
        assert!(run(&host, &mut tx, UpdatePolicy::default()).await.is_err());
        assert!(!host
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|value| value == forbidden));
    }
}

#[tokio::test]
async fn recovery_of_drain_never_stops_a_job_and_exposure_never_restores_data() {
    let host = Fixture::new();
    let mut tx = transaction();
    tx.phase = Phase::Draining;
    astra_updater::apply::recover::resume(&host, &mut tx)
        .await
        .unwrap();
    assert_eq!(*host.events.lock().unwrap(), vec!["abort", "Failed"]);
    let mut host = Fixture::new();
    host.fail_health = true;
    let mut tx = transaction();
    tx.phase = Phase::Exposing;
    assert!(astra_updater::apply::recover::resume(&host, &mut tx)
        .await
        .is_err());
    assert!(!host
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|value| value == "restore" || value == "stop"));
}

#[tokio::test]
async fn lost_rollback_open_reply_never_repeats_restore() {
    let mut host = Fixture::new();
    host.fail_health = true;
    host.lose_abort_reply.store(true, Ordering::SeqCst);
    let mut tx = transaction();
    assert!(run(&host, &mut tx, UpdatePolicy::default()).await.is_err());
    assert!(
        !tx.can_restore(),
        "rollback exposure must be durable before opening writes"
    );
    astra_updater::apply::recover::resume(&host, &mut tx)
        .await
        .unwrap();
    assert_eq!(host.data.load(Ordering::SeqCst), 99);
    let events = host.events.lock().unwrap();
    assert_eq!(events.iter().filter(|event| *event == "restore").count(), 1);
    assert_eq!(tx.phase, Phase::Quarantined);
}

#[tokio::test]
async fn rollback_barrier_failure_keeps_writes_closed() {
    let mut host = Fixture::new();
    host.fail_health = true;
    host.fail_phase = Some(Phase::RollbackExposing);
    let mut tx = transaction();
    assert!(run(&host, &mut tx, UpdatePolicy::default()).await.is_err());
    assert!(!host
        .events
        .lock()
        .unwrap()
        .iter()
        .any(|event| event == "abort"));
}
#[tokio::test]
async fn previous_restart_exposure_never_repeats_stop_after_lost_reply() {
    let host = Fixture::new();
    host.lose_abort_reply.store(true, Ordering::SeqCst);
    let mut tx = transaction();
    tx.phase = Phase::Stopped;
    assert!(astra_updater::apply::recover::resume(&host, &mut tx)
        .await
        .is_err());
    assert_eq!(tx.phase, Phase::PreviousExposing);
    assert!(!tx.can_restore());
    astra_updater::apply::recover::resume(&host, &mut tx)
        .await
        .unwrap();
    assert_eq!(host.data.load(Ordering::SeqCst), 99);
    let events = host.events.lock().unwrap();
    assert_eq!(events.iter().filter(|event| *event == "stop").count(), 1);
    assert!(!events.iter().any(|event| event == "restore"));
}
