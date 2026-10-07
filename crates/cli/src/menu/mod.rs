mod input;
mod period;
use crate::config::{ServicePolicy, Settings};
use hmail_cleaner_core::{maintenance::ScanMode, selection::Retention};
use std::io::{self, BufRead, Write};
pub(crate) type MenuError = io::Error;
#[derive(Debug)]
pub(crate) enum MenuAction {
    Preview(ScanMode),
    Clean {
        scan: ScanMode,
        service_policy: ServicePolicy,
    },
    SetRetention(Retention),
    Exit,
}
pub(crate) fn next_action(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    settings: &Settings,
) -> Result<MenuAction, MenuError> {
    loop {
        writeln!(writer,"\nData: {}\nRetention: {:?}\n1 Preview fast\n2 Preview full (hMailServer STOPPED)\n3 Clean fast\n4 Clean full\n5 Change retention\n0 Exit",settings.data_dir.display(),settings.retention)?;
        let Some(choice) = input::ask(reader, writer, "Choose: ")? else {
            return Ok(MenuAction::Exit);
        };
        match choice.as_str() {
            "0" => return Ok(MenuAction::Exit),
            "1" => return Ok(MenuAction::Preview(ScanMode::Fast)),
            "2" => return Ok(MenuAction::Preview(ScanMode::Full)),
            "3" | "4" => {
                let scan = if choice == "3" {
                    ScanMode::Fast
                } else {
                    ScanMode::Full
                };
                let Some(manage) = input::ask(
                    reader,
                    writer,
                    "Stop and restore hMailServer automatically? [yes/no]: ",
                )?
                else {
                    return Ok(MenuAction::Exit);
                };
                let service_policy = match manage.as_str() {
                    "yes" => ServicePolicy::Manage,
                    "no" => ServicePolicy::RequireStopped,
                    _ => {
                        writeln!(writer, "Enter yes or no.")?;
                        continue;
                    }
                };
                let cutoff = hmail_cleaner_core::selection::cutoff(
                    chrono::Local::now().naive_local(),
                    settings.retention,
                )
                .map_err(io::Error::other)?;
                writeln!(writer, "Local cutoff now: {cutoff}; strictly earlier. Age cutoff is recalculated after confirmation.")?;
                writeln!(
                    writer,
                    "Cleanup {:?}; service {:?}; retention {:?}.",
                    scan, service_policy, settings.retention
                )?;
                let Some(confirmation) = input::ask(
                    reader,
                    writer,
                    "Permanently delete selected mail? Type DELETE: ",
                )?
                else {
                    return Ok(MenuAction::Exit);
                };
                if confirmation == "DELETE" {
                    return Ok(MenuAction::Clean {
                        scan,
                        service_policy,
                    });
                }
                writeln!(writer, "Cleanup cancelled.")?;
            }
            "5" => {
                let Some(raw) = input::ask(reader, writer, "Age (30d/1mo/1y) or local date: ")?
                else {
                    return Ok(MenuAction::Exit);
                };
                match period::parse(&raw) {
                    Ok(retention) => return Ok(MenuAction::SetRetention(retention)),
                    Err(error) => writeln!(writer, "{error}")?,
                }
            }
            _ => writeln!(writer, "Invalid choice.")?,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{args::Args, config};
    use clap::Parser;
    fn settings() -> Settings {
        #[cfg(windows)]
        let (root, ini) = (r"C:\Data", r"C:\Bin\hMailServer.ini");
        #[cfg(not(windows))]
        let (root, ini) = ("/Data", "/Bin/hMailServer.ini");
        let args = Args::try_parse_from([
            "hmail-cleaner",
            "menu",
            "--data-dir",
            root,
            "--hmail-ini",
            ini,
            "--older-than",
            "30d",
        ])
        .unwrap();
        let values = std::collections::BTreeMap::from([(
            "DATABASE_URL".into(),
            "mysql://fixture:dummy@localhost/hmaildb".into(),
        )]);
        config::load_with_sources(&args, &values, &Default::default()).unwrap()
    }
    #[test]
    fn menu_reprompts_invalid_choice() {
        let mut output = Vec::new();
        assert!(matches!(
            next_action(&mut &b"bad\n1\n"[..], &mut output, &settings()).unwrap(),
            MenuAction::Preview(ScanMode::Fast)
        ));
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("Invalid choice"));
    }
    #[test]
    fn menu_eof_exits_without_cleanup() {
        assert!(matches!(
            next_action(&mut &b""[..], &mut Vec::new(), &settings()).unwrap(),
            MenuAction::Exit
        ));
    }
    #[test]
    fn menu_clean_requires_confirmation() {
        for raw in ["3\nno\nNO\n0\n", "3\nno\n"] {
            assert!(matches!(
                next_action(&mut raw.as_bytes(), &mut Vec::new(), &settings()).unwrap(),
                MenuAction::Exit
            ));
        }
        assert!(matches!(
            next_action(&mut &b"3\nno\nDELETE\n"[..], &mut Vec::new(), &settings()).unwrap(),
            MenuAction::Clean { .. }
        ));
    }
    #[test]
    fn period_changes_session_without_rewriting_env() {
        let settings = settings();
        let action = next_action(&mut &b"5\n1mo\n"[..], &mut Vec::new(), &settings).unwrap();
        let MenuAction::SetRetention(age) = action else {
            panic!("expected age")
        };
        let revised = settings.with_retention(age);
        assert!(matches!(revised.retention, Retention::Months(_)));
        assert!(matches!(settings.retention, Retention::Days(_)));
    }
}
