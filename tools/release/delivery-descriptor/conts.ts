import compatibility from "../../../deploy/compatibility.json";
export const MAX_DESCRIPTOR_BYTES = 64 * 1024;
export const SOURCE_SHA_PATTERN = /^[0-9a-f]{40}$/;
export const DELIVERY_PRODUCT = "astra-center";
export const DELIVERY_CHANNEL = "main";
export const DELIVERY_FORMAT = 1;
export const UPDATER_VERSION = 1;
export const SCHEMA_VERSION = compatibility.current_schema_version;
export const DELIVERY_FILE = "delivery.json";
export const DELIVERY_BUNDLE = "delivery.sigstore.json";
export const SIGNER_IDENTITY =
  "https://github.com/kubefox-bot/rust/.github/workflows/center-delivery.yml@refs/heads/main";
export const SIGNER_ISSUER = "https://token.actions.githubusercontent.com";
export const IMAGE_REPOSITORY = "ghcr.io/kubefox-bot/astra-center";
export const GITHUB_REPOSITORY = "kubefox-bot/rust";
