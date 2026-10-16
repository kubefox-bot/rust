mod artifact;
mod container;
mod settings;
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
            .env("CARGO_TARGET_DIR", root.join("target"))
            .env("RUSTFLAGS", "-C target-feature=+crt-static"))?;
        root.join("target")
            .join(TARGET)
            .join("release/hmail-cleaner.exe")
    } else {
        println!("Сборка Windows EXE через cargo-xwin 0.23.1; первый запуск загружает Rust и Windows SDK.");
        container::build(&settings)?
    };
    artifact::publish(&source, root)
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
