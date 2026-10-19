#![cfg(unix)]
use astra_updater::{
    journal::{Phase, Transaction},
    retention::expired,
};
#[test]
fn keeps_two_recent_successful_backups_and_all_unresolved_failures() {
    let base = Transaction {
        version: 1,
        id: uuid::Uuid::new_v4(),
        phase: Phase::Committed,
        previous_image: String::new(),
        previous_build: String::new(),
        candidate_image: String::new(),
        candidate_build: String::new(),
        sequence: 1,
        backup: "/tmp/backup".into(),
        candidate_export: "/tmp/export".into(),
        previous_unit: "/tmp/unit".into(),
    };
    let history: Vec<_> = [
        (5, Phase::Quarantined),
        (4, Phase::Committed),
        (2, Phase::Committed),
        (1, Phase::Committed),
        (3, Phase::Restoring),
    ]
    .into_iter()
    .map(|(sequence, phase)| Transaction {
        sequence,
        phase,
        ..base.clone()
    })
    .collect();
    assert_eq!(
        expired(history)
            .iter()
            .map(|tx| tx.sequence)
            .collect::<Vec<_>>(),
        vec![1]
    );
}
