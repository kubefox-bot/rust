export const DEFAULT_SAMPLE_MS = 50;
export const DEFAULT_WINDOW_MS = 3000;
export const MIN_SAMPLE_MS = 20;
export const MAX_SAMPLE_MS = 1000;
export const MIN_WINDOW_MS = 1000;
export const MAX_WINDOW_MS = 60000;
export const FIXTURE_STARTUP_MS = 30000;
export const FIXTURE_SHUTDOWN_MS = 10000;
export const FIXTURE_TERM_GRACE_MS = 1000;
export const FIXTURE_KILL_GRACE_MS = 2000;
export const FIXTURE_HISTORY_ROWS = 51;
export const HISTORY_PAGE_ROWS = 50;
export const UI_TIMEOUT_MS = 10000;
export const AGENT_CONTACT_RETRY_MS = 100;
export const NATIVE_SERVER_START_RETRY_MS = 50;
export const NATIVE_SERVER_CONNECT_TIMEOUT_MS = 1000;
export const PROTOCOL_RETENTION = { kind: "days", count: 30 } as const;
export const PROTOCOL_LIMITS = { batch_size: 1000, workers: 4 } as const;
export const BROWSER_WRITE_HEADERS = {
  "X-HMail-Request": "1",
  "Content-Type": "application/json",
} as const;
export const FIXTURE_PASSWORD = "fixture-center-password";
export const CENTER_RELATIVE_PATH = "target/release/hmail-cleaner";
export const FIXTURE_RELATIVE_PATH = "target/release/examples/resource_fixture";
export const STATIC_RELATIVE_PATH = "apps/web/dist";
export const PREVIEW_STEPS = ["fast", "full"] as const;
export const MAX_BUILD_RECEIPT_BYTES = 1024 * 1024;
export const PRIVATE_REPORT_MODE = 0o600;
const ARTIFACT_SCHEMA = {
  type: "object",
  additionalProperties: false,
  required: ["name", "size", "sha256"],
  properties: {
    name: {
      type: "string",
      pattern:
        "^(?:target/release/(?:examples/)?[a-zA-Z0-9_-]+|apps/web/dist/(?!.*(?:^|/)\\.\\.?(?:/|$))[A-Za-z0-9._+/-]+)$",
    },
    size: { type: "integer", minimum: 1 },
    sha256: { type: "string", pattern: "^[0-9a-f]{64}$" },
  },
} as const;
export const BUILD_RECEIPT_SCHEMA = {
  type: "object",
  additionalProperties: false,
  required: [
    "format_version",
    "build",
    "source",
    "profile",
    "default_features",
    "features",
    "artifacts",
  ],
  properties: {
    format_version: { const: 1 },
    build: { type: "string", pattern: "^[0-9A-Za-z.+_-]{1,128}$" },
    source: {
      type: "object",
      additionalProperties: false,
      required: ["revision", "fingerprint", "dirty"],
      properties: {
        revision: {
          type: "string",
          pattern: "^[0-9a-f]{12}(?:-dirty\\.[0-9a-f]{12})?$",
        },
        fingerprint: { type: "string", pattern: "^[0-9a-f]{64}$" },
        dirty: { type: "boolean" },
      },
    },
    profile: { const: "release" },
    default_features: { const: false },
    features: {
      type: "array",
      minItems: 1,
      maxItems: 1,
      items: { const: "center" },
    },
    artifacts: { type: "array", minItems: 3, items: ARTIFACT_SCHEMA },
  },
} as const;
