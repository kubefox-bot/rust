export const HOST_PACKAGE_ARCHIVE = "astra-updater-linux-amd64.tar.gz";
export const HOST_PACKAGE_MANIFEST = "package-manifest.json";
export const HOST_PACKAGE_BUNDLE = "astra-updater-linux-amd64.sigstore.json";
export const HOST_PACKAGE_FILES = [
  "astra-updater",
  "cosign",
  "sigstore-trusted-root.json",
  "install.sh",
  "config.example.json",
  "astra-updater.service",
  "astra-updater.timer",
  "astra-updater-recover.service",
  "astra-reboot-coordinator.service",
] as const;
export const PACKAGE_DOWNLOAD_LIMIT = 256 * 1024 * 1024;
export const PACKAGE_DOWNLOAD_TIMEOUT_MS = 180000;
export const BOOTSTRAP_IDENTITY =
  "keyless@projectsigstore.iam.gserviceaccount.com";
export const BOOTSTRAP_ISSUER = "https://accounts.google.com";
