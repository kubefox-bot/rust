use hmail_cleaner_protocol::{
    browser::{agents::*, auth::*, enrollment::*, jobs::*, profiles::*, settings::*, updates::*},
    *,
};
use schemars::JsonSchema;
/// Explicit browser allowlist. Never derive the internal application state or agent payloads here.
#[derive(JsonSchema)]
pub struct BrowserContracts {
    pub cleanup_context: CleanupContext,
    pub counter: Counter,
    pub server_id: ServerId,
    pub phase: JobPhase,
    pub summary: SummaryDto,
    pub session: SessionView,
    pub login: LoginRequest,
    pub change_password: ChangePasswordInput,
    pub prepare_recovery: PrepareRecoveryInput,
    pub recovery_candidate: RecoveryCandidate,
    pub confirm_recovery: ConfirmRecoveryInput,
    pub session_settings: SessionSettings,
    pub update_policy: UpdatePolicy,
    pub servers: Vec<ServerView>,
    pub agents: Vec<AgentView>,
    pub profile: EditableProfile,
    pub save_profile: SaveProfileRequest,
    pub profile_version: ProfileVersion,
    pub profile_validation: ProfileValidationResponse,
    pub retention: RetentionRequest,
    pub cutoff: ResolvedCutoff,
    pub submit: SubmitRequest,
    pub job: JobView,
    pub job_event: JobEvent,
    pub preview_files_page: PreviewFilesPage,
    pub history: HistoryPage,
    pub host_key: HostKeyChallenge,
    pub ssh_target: SshTarget,
    pub install: InstallRequest,
    pub update: UpdateRequest,
    pub enrollment: EnrollmentView,
    pub enrollments: Vec<EnrollmentView>,
    pub packages: Vec<AgentPackageView>,
    pub identity_receipt: IdentityReceipt,
    pub error: PublicError,
}
fn main() {
    let schema = schemars::schema_for!(BrowserContracts);
    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("browser schema serialization")
    );
}
