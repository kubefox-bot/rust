use std::{env, ffi::OsString, io, path::PathBuf};
pub(super) const TARGET: &str = "x86_64-pc-windows-msvc";
pub(super) const RUST: &str = "1.99.1";
pub(super) const BUILDER: &str = "messense/cargo-xwin:0.23.1";
pub(super) const AGENT_EXECUTABLE: &str = "hmail-cleaner-agent.exe";
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Component {
    Cli,
    Agent,
}
impl Component {
    pub fn arguments(self) -> &'static [&'static str] {
        match self {
            Self::Cli => &[],
            Self::Agent => &["--no-default-features", "--features", "agent"],
        }
    }
    pub fn executable(self) -> &'static str {
        match self {
            Self::Cli => "hmail-cleaner.exe",
            Self::Agent => AGENT_EXECUTABLE,
        }
    }
}
#[derive(Clone)]
pub(super) struct Settings {
    pub root: PathBuf,
    pub docker_context: Option<String>,
    pub component: Component,
    pub build_version: Option<String>,
}
impl Settings {
    pub fn read() -> io::Result<Self> {
        let component = component(env::args_os().skip(1).collect())?;
        let docker_context = match env::var("HMAIL_BUILD_DOCKER_CONTEXT") {
            Ok(value) if !value.trim().is_empty() => Some(value),
            Ok(_) | Err(env::VarError::NotUnicode(_)) => {
                return Err(io::Error::other("Некорректный HMAIL_BUILD_DOCKER_CONTEXT"))
            }
            Err(env::VarError::NotPresent) => None,
        };
        let root = env::current_dir()?;
        let build_version = match component {
            Component::Cli => None,
            Component::Agent => Some(super::manifest::version(&root)?),
        };
        Ok(Self {
            root,
            docker_context,
            component,
            build_version,
        })
    }
}
fn component(arguments: Vec<OsString>) -> io::Result<Component> {
    match arguments.as_slice() {
        [] => Ok(Component::Cli),
        [option, value] if option == "--component" => match value.to_str() {
            Some("cli") => Ok(Component::Cli),
            Some("agent") => Ok(Component::Agent),
            _ => Err(io::Error::other("Unknown build component")),
        },
        _ => Err(io::Error::other("Expected --component cli|agent")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn component_selection_preserves_default_and_isolates_agent_features() {
        assert!(matches!(component(vec![]).unwrap(), Component::Cli));
        assert_eq!(Component::Cli.arguments(), Vec::<&str>::new());
        assert!(matches!(
            component(vec!["--component".into(), "agent".into()]).unwrap(),
            Component::Agent
        ));
        assert_eq!(
            Component::Agent.arguments(),
            ["--no-default-features", "--features", "agent"]
        );
        assert!(component(vec!["--component".into(), "unknown".into()]).is_err());
    }
}
