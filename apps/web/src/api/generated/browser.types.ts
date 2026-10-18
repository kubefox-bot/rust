/* Generated from Rust browser DTOs. Do not edit. */

export type AgentId = string;
export type ServerId = string;
export type DispatchState = 'unknown' | 'acknowledged';
export type JobId = string;
export type Operation = 'preview' | 'clean';
export type Counter = string;
export type Scan = 'fast' | 'full';
export type ErrorCode =
  | 'bad_request'
  | 'unauthorized'
  | 'busy'
  | 'conflict'
  | 'unavailable'
  | 'interrupted'
  | 'cancelled'
  | 'cleanup_failed'
  | 'restore_failed'
  | 'internal';
export type ObservedMailState = 'unknown' | 'running' | 'stopped' | 'restoring';
export type ProfileId = string;
export type ServicePolicy = 'require_stopped' | 'manage';
export type JobPhase =
  | 'accepted'
  | 'running'
  | 'cancelling'
  | 'restoring'
  | 'completed'
  | 'partial'
  | 'failed'
  | 'cancelled'
  | 'interrupted';
export type JobStage =
  | 'preflight'
  | 'snapshot'
  | 'selection'
  | 'files'
  | 'database'
  | 'verification'
  | 'directories'
  | 'orphan_database'
  | 'references'
  | 'scan'
  | 'complete'
  | 'cancelled';
export type EnrollmentPhase =
  'preparing' | 'transferring' | 'identity_issued' | 'installing' | 'verifying' | 'ready' | 'failed' | 'unknown';
export type SshCredentials =
  | {
      method: 'password';
      password: string;
      username: string;
    }
  | {
      method: 'key';
      passphrase?: string | null;
      private_key: string;
      username: string;
    };
export type DatabaseTls = 'preferred' | 'required' | 'disabled_local';
export type RetentionRequest =
  | {
      count: number;
      kind: 'days';
    }
  | {
      count: number;
      kind: 'months';
    }
  | {
      count: number;
      kind: 'years';
    }
  | {
      kind: 'before';
      local: string;
    };
export type PasswordPatch =
  | {
      kind: 'keep';
    }
  | {
      kind: 'replace';
      value: string;
    };

/**
 * Explicit browser allowlist. Never derive the internal application state or agent payloads here.
 */
export interface BrowserContracts {
  agents: AgentView[];
  change_password: ChangePasswordInput;
  cleanup_context: CleanupContext;
  confirm_recovery: ConfirmRecoveryInput;
  counter: Counter;
  cutoff: ResolvedCutoff;
  enrollment: EnrollmentView;
  enrollments: EnrollmentView[];
  error: PublicError;
  history: HistoryPage;
  host_key: HostKeyChallenge;
  identity_receipt: IdentityReceipt;
  install: InstallRequest;
  job: JobView;
  job_event: JobEvent;
  login: LoginRequest;
  packages: AgentPackageView[];
  phase: JobPhase;
  prepare_recovery: PrepareRecoveryInput;
  preview_files_page: PreviewFilesPage;
  profile: EditableProfile;
  profile_validation: ProfileValidationResponse;
  profile_version: ProfileVersion;
  recovery_candidate: RecoveryCandidate;
  retention: RetentionRequest;
  save_profile: SaveProfileRequest;
  server_id: ServerId;
  servers: ServerView[];
  session: SessionView;
  session_settings: SessionSettings;
  ssh_target: SshTarget;
  submit: SubmitRequest;
  summary: SummaryDto;
  update: UpdateRequest;
  update_policy: UpdatePolicy;
}
export interface AgentView {
  agent_id: AgentId;
  certificate_expired: boolean;
  certificate_expires_at?: string | null;
  certificate_expiring: boolean;
  health: HealthView;
  name: string;
  revoked: boolean;
  server_id: ServerId;
}
export interface HealthView {
  agent_id: AgentId;
  healthy?: boolean | null;
  observed_at?: string | null;
  online: boolean;
  stale: boolean;
  version?: string | null;
}
export interface ChangePasswordInput {
  confirm_password: string;
  current_password: string;
  new_password: string;
}
export interface CleanupContext {
  busy: boolean;
  configured: boolean;
  latest?: JobView | null;
  profile_version: Counter;
  server: ServerView;
}
export interface JobView {
  dispatch: DispatchState;
  intent: SubmitRequest;
  job_id: JobId;
  revision: Counter;
  slot_held: boolean;
  snapshot?: JobSnapshot | null;
  stream_epoch: string;
}
export interface SubmitRequest {
  confirmed: boolean;
  job_id: JobId;
  limits: RunLimits;
  operation: Operation;
  preview_job_id?: JobId | null;
  profile_version: Counter;
  resolved_cutoff: ResolvedCutoff;
  scan: Scan;
  server_id: ServerId;
  /**
   * Intended cleanup policy; preview itself never stops the service.
   */
  service_policy: 'require_stopped' | 'manage';
}
export interface RunLimits {
  batch_size: number;
  workers: number;
}
export interface ResolvedCutoff {
  file_cutoff_utc: string;
  local: string;
  resolved_at_utc: string;
  utc_offset_seconds: number;
}
export interface JobSnapshot {
  error?: PublicError | null;
  initial_mail_state?: ObservedMailState | null;
  job?: PublicJob | null;
  job_id: JobId;
  mail_state: ObservedMailState;
  observed_at: string;
  phase: JobPhase;
  revision: Counter;
  stage?: JobStage | null;
  stream_epoch: string;
  summary: SummaryDto;
}
export interface PublicError {
  code: ErrorCode;
  correlation_id: string;
  message: string;
}
export interface PublicJob {
  agent_id: AgentId;
  limits: RunLimits;
  operation: Operation;
  profile_id: ProfileId;
  profile_version: Counter;
  resolved_cutoff: ResolvedCutoff;
  scan: Scan;
  service_policy: ServicePolicy;
}
export interface SummaryDto {
  absent_files: Counter;
  deleted_messages: Counter;
  deleted_metadata: Counter;
  deleted_recipients: Counter;
  examined_messages: Counter;
  failed: Counter;
  orphan_metadata: Counter;
  orphan_recipients: Counter;
  remaining_old_files?: Counter | null;
  removed_bytes: Counter;
  removed_directories: Counter;
  removed_files: Counter;
  removed_orphan_files: Counter;
  scanned_files: Counter;
  selected: Counter;
  selected_bytes: Counter;
  selected_files: Counter;
  selected_orphan_files: Counter;
  selected_orphan_metadata: Counter;
  selected_orphan_recipients: Counter;
  total_scan_files?: Counter | null;
  verified_files: Counter;
}
export interface ServerView {
  agent_id: AgentId;
  id: ServerId;
  name: string;
  profile_id: ProfileId;
  slug?: string | null;
}
export interface ConfirmRecoveryInput {
  candidate_id: string;
  recovery_key: string;
}
export interface EnrollmentView {
  agent_id: AgentId;
  expected_sha256: string;
  fingerprint: string;
  id: string;
  package_version: string;
  phase: EnrollmentPhase;
  profile_id: ProfileId;
  server_id: ServerId;
  slot_held: boolean;
}
export interface HistoryPage {
  jobs: JobView[];
  next_cursor?: Counter | null;
}
export interface HostKeyChallenge {
  fingerprint: string;
  target: SshTarget;
}
export interface SshTarget {
  host: string;
  port: number;
}
export interface IdentityReceipt {
  agent_id: AgentId;
  serial: number[];
}
export interface InstallRequest {
  agent_id: AgentId;
  credentials: SshCredentials;
  endpoint: string;
  fingerprint: string;
  legacy?: LegacyService | null;
  name: string;
  operation_id: string;
  package_version: string;
  profile_id: ProfileId;
  server_id: ServerId;
  target: SshTarget;
}
export interface LegacyService {
  env_file: string;
  executable: string;
}
export interface JobEvent {
  job: JobView;
  revision: Counter;
  stream_epoch: string;
}
export interface LoginRequest {
  login: string;
  password: string;
}
export interface AgentPackageView {
  build: string;
  sha256: string;
}
export interface PrepareRecoveryInput {
  current_password: string;
}
export interface PreviewFilesPage {
  complete: boolean;
  job_id: JobId;
  revision: Counter;
  rows: PreviewFile[];
  total: Counter;
}
export interface PreviewFile {
  created_at: string;
  id: string;
  orphan: boolean;
  path: string;
  size_bytes: Counter;
}
export interface EditableProfile {
  agent_id: AgentId;
  configured: boolean;
  data_dir: string;
  hmail_ini: string;
  id: ProfileId;
  mysql_database: string;
  mysql_host: string;
  mysql_port: number;
  mysql_tls: DatabaseTls;
  mysql_username: string;
  password_set: boolean;
  service_name: string;
  version: Counter;
}
export interface ProfileValidationResponse {
  mail_state: ObservedMailState;
}
export interface ProfileVersion {
  id: ProfileId;
  version: Counter;
}
export interface RecoveryCandidate {
  candidate_id: string;
  expires_in_seconds: number;
  recovery_key: string;
}
export interface SaveProfileRequest {
  expected_version: Counter;
  profile: ProfilePatch;
}
export interface ProfilePatch {
  data_dir: string;
  hmail_ini: string;
  mysql_database: string;
  mysql_host: string;
  mysql_port: number;
  mysql_tls: DatabaseTls;
  mysql_username: string;
  password: PasswordPatch;
  service_name: string;
}
export interface SessionView {
  authenticated: boolean;
  generation?: Counter | null;
}
export interface SessionSettings {
  idle_minutes: number;
}
export interface UpdateRequest {
  credentials: SshCredentials;
  fingerprint: string;
  mode?: 'binary' | 'recover_identity';
  operation_id: string;
  package_version: string;
  target: SshTarget;
}
export interface UpdatePolicy {
  check_interval_seconds: number;
  graceful_timeout_minutes: number;
}
