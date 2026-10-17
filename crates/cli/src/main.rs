mod app;
mod args;
mod commands;
mod config;
mod menu;
mod output;
use args::{Args, Mode};
use clap::Parser;
use commands::Report;
use config::{RunIntent, ServicePolicy};
use hmail_cleaner_core::cleanup::Cancellation;
use output::{Output, OutputPolicy};
use std::{io::IsTerminal, process::ExitCode, sync::Arc};
#[tokio::main]
async fn main() -> ExitCode {
    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) => {
            let code = error.exit_code() as u8;
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    if args.is_service() {
        eprintln!("Очистка по расписанию удалена. Используйте preview/clean или агент центра.");
        return ExitCode::from(2);
    }
    if args.is_menu()
        && (!std::io::stdin().is_terminal() || args.options().silence || args.options().json)
    {
        eprintln!("Меню требует интерактивной консоли и текстового вывода. Для автоматического запуска используйте preview или clean");
        return ExitCode::from(2);
    }
    let launch = match config::load_launch(&args) {
        Ok(s) => s,
        Err(error) => {
            if args.options().json {
                if let Ok(output) =
                    Output::open(OutputPolicy::Json, None, Arc::new(Cancellation::default()))
                {
                    output.finish(&Report::invalid(error));
                }
            } else {
                eprintln!("{error}");
            }
            return ExitCode::from(2);
        }
    };
    let cancel = Arc::new(Cancellation::default());
    let policy = if args.options().silence {
        OutputPolicy::Silent
    } else if args.options().json {
        OutputPolicy::Json
    } else if args.options().plain {
        OutputPolicy::Plain
    } else {
        OutputPolicy::Text
    };
    let output = match Output::open(policy, launch.log_file.as_deref(), cancel.clone()) {
        Ok(o) => o,
        Err(error) => {
            eprintln!("Не удалось открыть вывод: {error}");
            return ExitCode::from(2);
        }
    };
    let settings = match launch.cleanup {
        Ok(settings) => settings,
        Err(error) => {
            output.finish(&Report::invalid(error));
            return ExitCode::from(if output.failed() { 1 } else { 2 });
        }
    };
    let signal_cancel = cancel.clone();
    let signal = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            signal_cancel.request();
        }
    });
    let code = if args.is_menu() {
        run_menu(settings, &output, &cancel).await
    } else {
        let intent = RunIntent {
            mode: args.mode(),
            scan: settings.scan,
            service_policy: if args.options().manage_service {
                ServicePolicy::Manage
            } else {
                ServicePolicy::RequireStopped
            },
        };
        let report = run_intent(&settings, intent, &output, &cancel)
            .await
            .into_report();
        output.finish(&report);
        report.exit_code
    };
    signal.abort();
    let _ = signal.await;
    ExitCode::from(if output.failed() { 1 } else { code })
}
async fn run_intent(
    settings: &config::Settings,
    intent: RunIntent,
    output: &Output,
    cancel: &Cancellation,
) -> app::RunResult {
    match settings.for_run(intent, chrono::Local::now()) {
        Ok(config) => app::run_once(&config, output, cancel).await,
        Err(error) => app::RunResult::Finished(Report::invalid(error)),
    }
}
async fn run_menu(mut settings: config::Settings, output: &Output, cancel: &Cancellation) -> u8 {
    loop {
        if cancel.is_requested() || output.failed() {
            return 130;
        }
        let action = {
            let mut input = std::io::stdin().lock();
            let mut writer = std::io::stdout().lock();
            menu::next_action(&mut input, &mut writer, &settings)
        };
        let intent = match action {
            Ok(menu::MenuAction::Exit) => return 0,
            Ok(menu::MenuAction::SetRetention(age)) => {
                settings = settings.with_retention(age);
                continue;
            }
            Ok(menu::MenuAction::Preview(scan)) => RunIntent {
                mode: Mode::Preview,
                scan,
                service_policy: ServicePolicy::RequireStopped,
            },
            Ok(menu::MenuAction::Clean {
                scan,
                service_policy,
            }) => RunIntent {
                mode: Mode::Clean,
                scan,
                service_policy,
            },
            Err(error) => {
                output.finish(&Report::invalid(error));
                return 2;
            }
        };
        let result = run_intent(&settings, intent, output, cancel).await;
        let fatal = result.is_fatal();
        let report = result.into_report();
        output.finish(&report);
        if !cancel.is_requested() {
            use std::io::Write;
            print!("Нажмите Enter для возврата в меню...");
            let _ = std::io::stdout().flush();
            let mut line = String::new();
            if std::io::stdin().read_line(&mut line).unwrap_or(0) == 0 {
                return report.exit_code;
            }
        }
        if fatal {
            return report.exit_code.max(1);
        }
    }
}
