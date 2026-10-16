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
        writeln!(writer,"\n hMail Cleaner {}\nКорень: {}\nПериод: {}\n1 Предпросмотр — быстрый\n2 Предпросмотр — полный\n3 Быстрая очистка\n4 Полная очистка\n5 Настройки текущего запуска: период\n0 Выход", env!("HMAIL_BUILD_VERSION"),settings.data_dir.display(),period::description(settings.retention))?;
        let Some(choice) = input::ask(reader, writer, "Выберите действие: ")?
        else {
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
                    "Остановить и восстановить hMailServer автоматически? [да/нет]: ",
                )?
                else {
                    return Ok(MenuAction::Exit);
                };
                let service_policy = match manage.as_str() {
                    "yes" | "да" => ServicePolicy::Manage,
                    "no" | "нет" => ServicePolicy::RequireStopped,
                    _ => {
                        writeln!(writer, "Введите да или нет.")?;
                        continue;
                    }
                };
                let cutoff = hmail_cleaner_core::selection::cutoff(
                    chrono::Local::now().naive_local(),
                    settings.retention,
                )
                .map_err(io::Error::other)?;
                writeln!(writer, "Дата создания строго раньше: {cutoff} (местное время). Для периода граница пересчитывается после подтверждения.")?;
                writeln!(
                    writer,
                    "Очистка: {}; управление службой: {}; период: {}.",
                    match scan {
                        ScanMode::Fast => "быстрая",
                        ScanMode::Full => "полная",
                    },
                    match service_policy {
                        ServicePolicy::Manage => "автоматическое",
                        ServicePolicy::RequireStopped => "служба уже остановлена",
                    },
                    period::description(settings.retention)
                )?;
                let Some(confirmation) = input::ask(
                    reader,
                    writer,
                    "Удалить выбранные письма безвозвратно? Введите УДАЛИТЬ: ",
                )?
                else {
                    return Ok(MenuAction::Exit);
                };
                if matches!(confirmation.as_str(), "DELETE" | "УДАЛИТЬ") {
                    return Ok(MenuAction::Clean {
                        scan,
                        service_policy,
                    });
                }
                writeln!(writer, "Очистка отменена.")?;
            }
            "5" => {
                let Some(raw) =
                    input::ask(reader, writer, "Период (30d/1mo/1y) или точная дата: ")?
                else {
                    return Ok(MenuAction::Exit);
                };
                match period::parse(&raw) {
                    Ok(retention) => return Ok(MenuAction::SetRetention(retention)),
                    Err(error) => writeln!(writer, "{error}")?,
                }
            }
            _ => writeln!(writer, "Неверный выбор.")?,
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
            .contains("Неверный выбор"));
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
