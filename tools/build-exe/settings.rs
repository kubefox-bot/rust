use std::{env, io, path::PathBuf};
pub(super) const TARGET: &str = "x86_64-pc-windows-msvc";
pub(super) const RUST: &str = "1.99.1";
pub(super) const BUILDER: &str = "messense/cargo-xwin:0.23.1";
#[derive(Clone)]
pub(super) struct Settings {
    pub root: PathBuf,
    pub docker_context: Option<String>,
}
impl Settings {
    pub fn read() -> io::Result<Self> {
        let docker_context = match env::var("HMAIL_BUILD_DOCKER_CONTEXT") {
            Ok(value) if !value.trim().is_empty() => Some(value),
            Ok(_) | Err(env::VarError::NotUnicode(_)) => {
                return Err(io::Error::other("Некорректный HMAIL_BUILD_DOCKER_CONTEXT"))
            }
            Err(env::VarError::NotPresent) => None,
        };
        Ok(Self {
            root: env::current_dir()?,
            docker_context,
        })
    }
}
