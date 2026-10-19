mod install;
mod package;
pub mod remote;
mod repository;
mod service;
mod ssh;
mod target;
mod types;
mod update;
mod validation;
pub use hmail_cleaner_protocol::browser::enrollment::{
    HostKeyChallenge, SshCredentials, SshTarget,
};
pub use package::{AgentPackage, PackageCatalog};
pub use service::EnrollmentService;
pub use ssh::{NativeSsh, VerifiedSsh};
pub use types::EnrollmentError;

pub(crate) mod http;
mod verify;

mod reconcile;
mod recover_identity;

mod failure;
mod recent;
mod restart;
