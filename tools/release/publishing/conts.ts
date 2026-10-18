export const MAX_API_BYTES = 2 * 1024 * 1024;
export const API_TIMEOUT_MS = 30000;
export const CI_RECEIPT_FILE = "verification.json";
export const PROMOTION_FILE = "publication.json";
export const REQUIRED_CHECKS = [
  "prelogin_https_health",
  "direct_container_healthcheck",
  "engine_health_state_before_login",
  "static_routes",
  "api_404",
  "credential_login",
  "graceful_sigterm",
  "stopped_backup",
  "recreate_vault_and_ca",
  "restart_revokes_session",
] as const;
export const SOURCE_LABEL = "io.astra.source-sha";
export const MAX_RELEASE_PAGES = 20;

export const UPLOAD_TIMEOUT_MS = 300000;
export const MAX_ASSET_PAGES = 20;
export const GITHUB_PAGE_SIZE = 100;
export const GITHUB_API_VERSION = "2022-11-28";
export const GITHUB_ASSET_HOSTS = [
  "release-assets.githubusercontent.com",
  "objects.githubusercontent.com",
] as const;
