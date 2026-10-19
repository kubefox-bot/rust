// Keep validation code with the browser capabilities that consume it.
export const CONTRACT_GROUPS = {
  auth: ["error", "login", "session"],
  values: ["counter", "phase", "server_id", "summary"],
  agents: ["agents", "packages", "servers"],
  cleanup: [
    "cleanup_context",
    "cutoff",
    "history",
    "job",
    "job_event",
    "preview_files_page",
    "retention",
    "submit",
  ],
  profile: ["profile", "profile_validation", "profile_version", "save_profile"],
  enrollment: [
    "enrollment",
    "enrollments",
    "host_key",
    "identity_receipt",
    "install",
    "ssh_target",
    "update",
  ],
  account: [
    "change_password",
    "confirm_recovery",
    "prepare_recovery",
    "recovery_candidate",
    "session_settings",
  ],
  updates: ["update_policy"],
} as const;
