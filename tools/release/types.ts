export interface AgentManifestInput {
  readonly directory: string;
  readonly build: string;
}
export type ReleaseEnvironment = Readonly<Record<string, string | undefined>>;
export type Engine = "podman" | "docker";
export type Platform = "linux/amd64" | "linux/arm64";
export type ReleaseOptions = Readonly<{
  engine: Engine;
  context?: string;
  platform: Platform;
  imageRepository: string;
  output: string;
  allowDirty: boolean;
  agentPackageDirectory?: string;
  build?: string;
}>;
export type SourceIdentity = Readonly<{
  revision: string;
  fingerprint: string;
  dirty: boolean;
}>;
export type BaseImages = Readonly<{
  bun: string;
  rust: string;
  runtime: string;
  buildkit: string;
}>;
export type ReleaseConfig = Readonly<{
  root: string;
  options: ReleaseOptions;
  source: SourceIdentity;
  build: string;
  directory: string;
  imageTag: string;
  bases: BaseImages;
  environment: ReleaseEnvironment;
}>;
export type Artifact = Readonly<{ name: string; size: number; sha256: string }>;
export type AgentManifest = Readonly<{
  product: "hmail-cleaner-agent";
  build: string;
  target: "x86_64-pc-windows-msvc";
  protocol_min: number;
  protocol_max: number;
  sha256: string;
}>;
export type ReleaseManifest = Readonly<{
  format_version: 1;
  build: string;
  source_revision: string;
  source_fingerprint: string;
  dirty: boolean;
  engine: Engine;
  image: Readonly<{
    reference: string;
    digest: string;
    config_digest: string;
    local_tag: string;
    platform: Platform;
  }>;
  base_images: BaseImages;
  agent: AgentManifest;
  artifacts: ReadonlyArray<Artifact>;
}>;
export type Deployment = Readonly<{
  container: string;
  volume: string;
  init: readonly string[];
}>;
export type VerificationConfig = Readonly<{
  root: string;
  options: ReleaseOptions;
  manifestPath: string;
  directory: string;
}>;
