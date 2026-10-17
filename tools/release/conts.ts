import type { BaseImages, Engine, Platform } from "./types";

export const RUST_TARGETS = {
  "linux/amd64": {
    triple: "x86_64-unknown-linux-gnu",
    gnuPrefix: "x86_64-linux-gnu",
    packages:
      "gcc-x86-64-linux-gnu=4:12.2.0-3 gcc-12-x86-64-linux-gnu=12.2.0-14cross1 binutils-x86-64-linux-gnu=2.40-2 libc6-dev-amd64-cross=2.36-8cross1",
    elfMachine: 62,
  },
  "linux/arm64": {
    triple: "aarch64-unknown-linux-gnu",
    gnuPrefix: "aarch64-linux-gnu",
    packages:
      "gcc-aarch64-linux-gnu=4:12.2.0-3 gcc-12-aarch64-linux-gnu=12.2.0-14cross1 binutils-aarch64-linux-gnu=2.40-2 libc6-dev-arm64-cross=2.36-8cross1",
    elfMachine: 183,
  },
} as const satisfies Record<
  Platform,
  {
    triple: string;
    gnuPrefix: string;
    packages: string;
    elfMachine: number;
  }
>;
export const ELF_HEADER_BYTES = 64;
export const ELF_MACHINE_OFFSET = 18;
export const ELF_MAGIC = [0x7f, 0x45, 0x4c, 0x46, 2, 1, 1] as const;

export const BASE_IMAGE_ARGUMENTS = {
  BUN_IMAGE: "bun",
  RUST_IMAGE: "rust",
  RUNTIME_IMAGE: "runtime",
} as const satisfies Record<
  "BUN_IMAGE" | "RUST_IMAGE" | "RUNTIME_IMAGE",
  keyof BaseImages
>;

export const AGENT_EXECUTABLE = "hmail-cleaner-agent.exe";
export const AGENT_PRODUCT = "hmail-cleaner-agent";
export const AGENT_TARGET = "x86_64-pc-windows-msvc";
export const PROTOCOL_MIN = 1;
export const PROTOCOL_MAX = 1;
export const BUILD_PATTERN = /^[0-9A-Za-z.+_-]{1,128}$/;
export const REVISION_PATTERN = /^[0-9a-f]{12}$/;
export const PE_DOS_MAGIC = [0x4d, 0x5a] as const;
export const MANIFEST_FILE_MODE = 0o600;
export const MILLISECONDS_PER_SECOND = 1000;
export const IMAGE_TAG_BUILD_LENGTH = 96;
export const IMAGE_TAG_FINGERPRINT_LENGTH = 12;
export const CONTAINER_UID = 10001;
export const CONTAINER_GID = 10001;
export const CENTER_PORT = 8443;
export const DATA_VOLUME = "hmail-center-data";
export const DATA_DIRECTORY = "/data";
export const CENTER_EXECUTABLE = "/usr/local/bin/hmail-cleaner";
export const ELF_FILE = CENTER_EXECUTABLE.slice(1);
export const CENTER_HEALTH_ARGV = [
  CENTER_EXECUTABLE,
  "center",
  "health",
  "--data-dir",
  DATA_DIRECTORY,
] as const;
export const CENTER_HEALTH_TEST = ["CMD", ...CENTER_HEALTH_ARGV] as const;
export const IMAGE_DIGEST_PATTERN = /^sha256:[0-9a-f]{64}$/;
export const IMAGE_REPOSITORY_PATTERN =
  /^(?:[a-z0-9]+(?:[.-][a-z0-9]+)*(?::[0-9]+)?\/)?[a-z0-9]+(?:[._-][a-z0-9]+)*(?:\/[a-z0-9]+(?:[._-][a-z0-9]+)*)*$/;
export const SOURCE_INPUTS = [
  "Cargo.toml",
  "Cargo.lock",
  "package.json",
  "bun.lock",
  "mise.toml",
  ".containerignore",
  "crates",
  "tools",
  "deploy",
  "apps/web/package.json",
  "apps/web/astro.config.mjs",
  "apps/web/tsconfig.json",
  "apps/web/src",
  "apps/web/public",
] as const;
export const EXCLUDED_SOURCE_PARTS = new Set([
  ".git",
  ".superpowers",
  "node_modules",
  "target",
  "dist",
  ".astro",
  ".cache",
]);
export const RELEASE_MANIFEST = "release-manifest.json";
export const OCI_ARCHIVE = "center.oci.tar";
export const RELEASE_FORMAT_VERSION = 1;
export const PRIVATE_DIRECTORY_MODE = 0o700;
export const DELIVERY_TIMEOUT_MS = 120000;
export const HEALTH_RETRY_MS = 1000;
export const HEALTH_ATTEMPTS = 30;
export const CONTAINER_HEALTH_ATTEMPTS = 60;
export const IMAGE_HEALTH_REQUIRED = {
  docker: true,
  podman: false,
} as const satisfies Record<Engine, boolean>;
export const CONTAINER_HEALTH_OPTIONS = {
  interval: "30s",
  timeout: "10s",
  startPeriod: "10s",
  retries: "3",
} as const;
export const MAX_OCI_METADATA_BYTES = 1024 * 1024;
export const MAX_OCI_LAYER_BYTES = 256 * 1024 * 1024;
export const DISALLOWED_RUNTIME_FILES = [
  "usr/local/bin/bun",
  "usr/local/bin/node",
  "usr/bin/node",
  "usr/bin/bun",
  "usr/local/cargo/bin/cargo",
  "usr/local/cargo/bin/rustc",
  "usr/bin/cargo",
  "usr/bin/rustc",
  "bin/sh",
  "bin/bash",
  "usr/bin/bash",
] as const;
export const CONTAINER_INPUTS = [
  "Cargo.toml",
  "Cargo.lock",
  "package.json",
  "bun.lock",
  "crates",
  "apps/web/package.json",
  "apps/web/tsconfig.json",
  "apps/web/astro.config.mjs",
  "apps/web/src",
  "deploy/Containerfile",
] as const;
