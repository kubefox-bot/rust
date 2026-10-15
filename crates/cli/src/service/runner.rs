use super::schedule;
use crate::{
    app::RunResult,
    args::Mode,
    config::{RunIntent, ServicePolicy, ServiceSettings},
};
use chrono::{DateTime, Local};
use hmail_cleaner_core::{
    cleanup::{Cancellation, PortResult},
    maintenance::ScanMode,
};
use std::{future::Future, sync::Arc, time::Duration};
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WaitResult {
    Reached,
    Cancelled,
}
pub(super) trait ServiceClock {
    fn now(&self) -> DateTime<Local>;
    fn wait_until(
        &self,
        deadline: DateTime<Local>,
        stop: &Cancellation,
    ) -> impl Future<Output = PortResult<WaitResult>> + Send;
}
pub(super) trait ScheduledJob {
    fn execute(
        &mut self,
        intent: RunIntent,
        stop: &Cancellation,
    ) -> impl Future<Output = RunResult> + Send;
}
pub(super) struct SystemClock;
impl ServiceClock for SystemClock {
    fn now(&self) -> DateTime<Local> {
        Local::now()
    }
    async fn wait_until(
        &self,
        deadline: DateTime<Local>,
        stop: &Cancellation,
    ) -> PortResult<WaitResult> {
        const POLL: Duration = Duration::from_secs(1);
        loop {
            if stop.is_requested() {
                return Ok(WaitResult::Cancelled);
            }
            let now = Local::now();
            if now >= deadline {
                return Ok(WaitResult::Reached);
            }
            let delay = (deadline - now).to_std()?.min(POLL);
            tokio::time::sleep(delay).await;
        }
    }
}
pub(super) async fn run<C: ServiceClock, J: ScheduledJob>(
    settings: &ServiceSettings,
    clock: &C,
    job: &mut J,
    stop: Arc<Cancellation>,
) -> PortResult<()> {
    let mut last_local_date = None;
    loop {
        if stop.is_requested() {
            return Ok(());
        }
        let slot = schedule::next_run(clock.now(), settings.run_at, last_local_date)?;
        if clock.wait_until(slot, &stop).await? == WaitResult::Cancelled || stop.is_requested() {
            return Ok(());
        }
        if clock.now().date_naive() != slot.date_naive() {
            continue;
        }
        last_local_date = Some(slot.date_naive());
        let result = job
            .execute(
                RunIntent {
                    mode: Mode::Clean,
                    scan: ScanMode::Fast,
                    service_policy: ServicePolicy::Manage,
                },
                &stop,
            )
            .await;
        if result.is_fatal() {
            return Err("scheduled cleanup stopped after fatal restore or log failure".into());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands::Report, config};
    use chrono::TimeZone;
    use clap::Parser;
    use std::sync::Mutex;
    struct Clock {
        now: Mutex<DateTime<Local>>,
        slots: Mutex<Vec<DateTime<Local>>>,
        cancel_wait: bool,
    }
    impl ServiceClock for Clock {
        fn now(&self) -> DateTime<Local> {
            *self.now.lock().unwrap()
        }
        async fn wait_until(
            &self,
            deadline: DateTime<Local>,
            stop: &Cancellation,
        ) -> PortResult<WaitResult> {
            self.slots.lock().unwrap().push(deadline);
            if self.cancel_wait {
                stop.request();
                return Ok(WaitResult::Cancelled);
            }
            *self.now.lock().unwrap() = deadline;
            Ok(WaitResult::Reached)
        }
    }
    struct Job<'a> {
        clock: &'a Clock,
        count: u8,
        fatal: bool,
        long: bool,
    }
    impl ScheduledJob for Job<'_> {
        async fn execute(&mut self, intent: RunIntent, stop: &Cancellation) -> RunResult {
            assert_eq!(intent.mode, Mode::Clean);
            assert_eq!(intent.scan, ScanMode::Fast);
            assert_eq!(intent.service_policy, ServicePolicy::Manage);
            self.count += 1;
            if self.long {
                *self.clock.now.lock().unwrap() += chrono::Duration::days(3);
            }
            if self.count >= 2 {
                stop.request();
            }
            if self.fatal {
                RunResult::Fatal(Report::invalid("restore failed"))
            } else {
                RunResult::Finished(Report::invalid("ordinary partial"))
            }
        }
    }
    fn settings() -> ServiceSettings {
        let args = crate::args::Args::try_parse_from(["hmail-cleaner", "service", "run"]).unwrap();
        #[cfg(windows)]
        let (root, ini, log) = (
            r"C:\Data",
            r"C:\Bin\hMailServer.ini",
            r"C:\private\clean.ndjson",
        );
        #[cfg(not(windows))]
        let (root, ini, log) = ("/Data", "/Bin/hMailServer.ini", "/private/clean.ndjson");
        let values = std::collections::BTreeMap::from([
            (
                "DATABASE_URL".into(),
                "mysql://fixture:dummy@localhost/hmaildb".into(),
            ),
            ("HMAIL_DATA_DIR".into(), root.into()),
            ("HMAIL_INI".into(), ini.into()),
            ("HMAIL_OLDER_THAN".into(), "30d".into()),
            ("HMAIL_LOG_FILE".into(), log.into()),
        ]);
        ServiceSettings {
            cleanup: config::load_with_sources(&args, &values, &Default::default()).unwrap(),
            run_at: chrono::NaiveTime::from_hms_opt(2, 0, 0).unwrap(),
        }
    }
    fn clock(cancel_wait: bool) -> Clock {
        Clock {
            now: Mutex::new(Local.with_ymd_and_hms(2026, 10, 15, 3, 0, 0).unwrap()),
            slots: Mutex::new(vec![]),
            cancel_wait,
        }
    }
    #[tokio::test]
    async fn stop_interrupts_wait() {
        let clock = clock(true);
        let mut job = Job {
            clock: &clock,
            count: 0,
            fatal: false,
            long: false,
        };
        run(
            &settings(),
            &clock,
            &mut job,
            Arc::new(Cancellation::default()),
        )
        .await
        .unwrap();
        assert_eq!(job.count, 0);
        assert_eq!(
            clock.slots.lock().unwrap()[0].date_naive(),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 16).unwrap()
        );
    }
    #[tokio::test]
    async fn fatal_restore_failure_ends_loop() {
        let clock = clock(false);
        let mut job = Job {
            clock: &clock,
            count: 0,
            fatal: true,
            long: false,
        };
        assert!(run(
            &settings(),
            &clock,
            &mut job,
            Arc::new(Cancellation::default())
        )
        .await
        .is_err());
        assert_eq!(job.count, 1);
        assert_eq!(clock.slots.lock().unwrap().len(), 1);
    }
    #[tokio::test]
    async fn long_job_does_not_enqueue_jobs() {
        let clock = clock(false);
        let mut job = Job {
            clock: &clock,
            count: 0,
            fatal: false,
            long: true,
        };
        run(
            &settings(),
            &clock,
            &mut job,
            Arc::new(Cancellation::default()),
        )
        .await
        .unwrap();
        assert_eq!(job.count, 2);
        let slots = clock.slots.lock().unwrap();
        assert_eq!(slots.len(), 2);
        assert!(slots[1] - slots[0] >= chrono::Duration::days(4));
    }
}
