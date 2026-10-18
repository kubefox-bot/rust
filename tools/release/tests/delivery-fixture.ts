import type { ReleaseManifest } from "../types";
export const sourceSha = "a".repeat(40);
export const releaseFixture = (): ReleaseManifest => ({
  format_version: 1,
  build: `git-${sourceSha}`,
  source_revision: sourceSha.slice(0, 12),
  source_fingerprint: "b".repeat(64),
  dirty: false,
  engine: "podman",
  image: {
    reference: `ghcr.io/kubefox-bot/astra-center@sha256:${"c".repeat(64)}`,
    digest: `sha256:${"c".repeat(64)}`,
    config_digest: `sha256:${"d".repeat(64)}`,
    local_tag: "ghcr.io/kubefox-bot/astra-center:test",
    platform: "linux/amd64",
  },
  base_images: {
    bun: `docker.io/oven/bun@sha256:${"e".repeat(64)}`,
    rust: `docker.io/library/rust@sha256:${"e".repeat(64)}`,
    runtime: `gcr.io/distroless/cc@sha256:${"e".repeat(64)}`,
    buildkit: `docker.io/moby/buildkit@sha256:${"e".repeat(64)}`,
  },
  agent: {
    product: "hmail-cleaner-agent",
    build: `git-${sourceSha}`,
    target: "x86_64-pc-windows-msvc",
    protocol_min: 1,
    protocol_max: 1,
    sha256: "f".repeat(64),
  },
  artifacts: [{ name: "center.oci.tar", size: 1, sha256: "f".repeat(64) }],
});
