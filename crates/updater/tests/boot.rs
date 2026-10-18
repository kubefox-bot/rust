#![cfg(unix)]
use astra_updater::{
    apply::boot::{self, Host},
    journal::{Phase, Transaction},
    Error,
};
use std::{path::PathBuf, sync::Mutex};
struct Generator {
    unit: PathBuf,
    effective: Mutex<String>,
    events: Mutex<Vec<&'static str>>,
}
impl Host for Generator {
    fn select_previous(&self, tx: &Transaction) -> Result<(), Error> {
        self.events.lock().unwrap().push("select");
        std::fs::copy(&tx.previous_unit, &self.unit).map_err(|_| Error::Storage)?;
        Ok(())
    }
    async fn reload(&self) -> Result<(), Error> {
        self.events.lock().unwrap().push("reload");
        *self.effective.lock().unwrap() =
            std::fs::read_to_string(&self.unit).map_err(|_| Error::Storage)?;
        Ok(())
    }
    async fn mark(&self, _: &Transaction) -> Result<(), Error> {
        self.events.lock().unwrap().push("mark");
        Ok(())
    }
    async fn restore_offline(&self, _: &mut Transaction) -> Result<(), Error> {
        self.events.lock().unwrap().push("restore");
        Ok(())
    }
}
#[tokio::test]
async fn dependent_start_uses_previous_generated_unit_after_boot_recovery() {
    for phase in [
        Phase::Stopped,
        Phase::Starting,
        Phase::Restoring,
        Phase::RollbackExposing,
        Phase::PreviousExposing,
    ] {
        let directory = tempfile::tempdir().unwrap();
        let previous_unit = directory.path().join("previous.container");
        std::fs::write(&previous_unit, "ExecStart=previous").unwrap();
        let host = Generator {
            unit: directory.path().join("center.container"),
            effective: Mutex::new("ExecStart=candidate".into()),
            events: Mutex::new(vec![]),
        };
        let mut tx = Transaction {
            version: 1,
            id: uuid::Uuid::new_v4(),
            phase,
            previous_image: "old".into(),
            previous_build: "old".into(),
            candidate_image: "new".into(),
            candidate_build: "new".into(),
            sequence: 2,
            backup: directory.path().join("backup"),
            candidate_export: directory.path().join("export"),
            previous_unit,
        };
        boot::prepare(&host, &mut tx).await.unwrap();
        host.events.lock().unwrap().push("dependent_start");
        assert_eq!(
            *host.effective.lock().unwrap(),
            "ExecStart=previous",
            "{phase:?}: generator cache must be refreshed before startup"
        );
        let events = host.events.lock().unwrap();
        let index = |event| events.iter().position(|value| *value == event).unwrap();
        assert!(index("select") < index("reload"));
        assert!(index("reload") < index("dependent_start"));
        if matches!(phase, Phase::RollbackExposing | Phase::PreviousExposing) {
            assert!(!events.contains(&"restore"));
            assert!(!events.contains(&"mark"));
        }
    }
}
