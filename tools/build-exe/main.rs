mod artifact;
#[path = "../../crates/cli/build_support.rs"]
mod build_identity;
mod container;
mod manifest;
mod package;
mod settings;
mod source;
use settings::{Settings, TARGET};
use std::{
    io,
    process::{Command, ExitCode},
};
fn run(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "Команда завершилась с ошибкой: {status}"
        )))
    }
}
fn build() -> io::Result<()> {
    let settings = Settings::read()?;
    let root = &settings.root;
    let source = if cfg!(windows) {
        run(Command::new("cargo")
            .args([
                "build",
                "--release",
                "-p",
                "hmail-cleaner",
                "--target",
                TARGET,
                "--locked",
            ])
            .args(settings.component.arguments())
            .env("CARGO_TARGET_DIR", root.join("target"))
            .envs(
                settings
                    .build_version
                    .as_ref()
                    .map(|version| ("HMAIL_BUILD_VERSION", version)),
            )
            .env("RUSTFLAGS", "-C target-feature=+crt-static"))?;
        root.join("target")
            .join(TARGET)
            .join("release/hmail-cleaner.exe")
    } else {
        println!("Сборка Windows EXE через cargo-xwin 0.23.1; первый запуск загружает Rust и Windows SDK.");
        container::build(&settings)?
    };
    match settings.component {
        settings::Component::Cli => {
            artifact::publish(&source, root, settings.component.executable())
        }
        settings::Component::Agent => {
            let version = settings
                .build_version
                .as_deref()
                .ok_or_else(|| io::Error::other("Missing build version"))?;
            package::publish(&source, &root.join("dist/agent"), |stage| {
                manifest::write(root, stage, version)
            })
        }
    }
}
fn main() -> ExitCode {
    match build() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "Ошибка сборки: {error}. Предыдущий EXE не заменяется при неудачной компиляции."
            );
            ExitCode::FAILURE
        }
    }
}
