use super::event::Event;
use hmail_cleaner_core::cleanup::Summary;
use std::io::{self, Write};
fn counters(s: &Summary) -> String {
    format!("selected={}, files={}, bytes={}, absent={}, messages={}, metadata={}, recipients={}, orphan_metadata={}/{}, orphan_recipients={}/{}, scanned={}, orphan_files={}/{}, directories={}, failed={}",s.selected,s.removed_files,s.removed_bytes,s.absent_files,s.deleted_messages,s.deleted_metadata,s.deleted_recipients,s.orphan_metadata,s.selected_orphan_metadata,s.orphan_recipients,s.selected_orphan_recipients,s.scanned_files,s.removed_orphan_files,s.selected_orphan_files,s.removed_directories,s.failed)
}
pub(super) fn write(event: &Event<'_>) -> io::Result<()> {
    match event {
        Event::Server {
            mysql_version,
            cutoff,
        } => writeln!(
            io::stderr().lock(),
            "MySQL {mysql_version}; cutoff {cutoff} (server-local, strict <)"
        ),
        Event::Progress(p) => match &p.failure {
            Some(failure) => writeln!(io::stderr().lock(), "{:?}: {:?}", p.stage, failure),
            None => writeln!(
                io::stderr().lock(),
                "{:?}: {}",
                p.stage,
                counters(&p.summary)
            ),
        },
        Event::Result(r) => writeln!(
            io::stdout().lock(),
            "{}{:?}: {}{}",
            r.mode.map(|mode| format!("{mode:?} ")).unwrap_or_default(),
            r.status,
            counters(&r.summary),
            r.error
                .as_ref()
                .map(|e| format!("; {e}"))
                .unwrap_or_default()
        ),
    }
}
