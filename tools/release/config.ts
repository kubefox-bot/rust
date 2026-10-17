import { resolve, join } from "node:path";
import {
  BUILD_PATTERN,
  IMAGE_DIGEST_PATTERN,
  IMAGE_REPOSITORY_PATTERN,
  MILLISECONDS_PER_SECOND,
  IMAGE_TAG_BUILD_LENGTH,
  IMAGE_TAG_FINGERPRINT_LENGTH,
} from "./conts";
import { sourceIdentity } from "./source-identity";
import type {
  BaseImages,
  ReleaseConfig,
  ReleaseEnvironment,
  ReleaseOptions,
  VerificationConfig,
} from "./types";

export const configuredBuildVersion = (
  environment: ReleaseEnvironment = Bun.env,
): string | undefined => {
  const build = environment.HMAIL_BUILD_VERSION;
  if (
    build !== undefined &&
    (!BUILD_PATTERN.test(build) || build === "." || build === "..")
  )
    throw new Error("Invalid HMAIL_BUILD_VERSION");
  return build;
};
export const pinnedImage = (image: string): string => {
  const [name, digest, extra] = image.split("@");
  const repository = name?.replace(/:[^/:]+$/, "");
  if (
    !name ||
    !repository ||
    extra !== undefined ||
    !IMAGE_REPOSITORY_PATTERN.test(repository) ||
    !digest ||
    !IMAGE_DIGEST_PATTERN.test(digest)
  )
    throw new Error("An immutable image@sha256 reference is required");
  return image;
};
export const parseReleaseOptions = (
  environment: ReleaseEnvironment,
): ReleaseOptions => {
  const engine = environment.HMAIL_RELEASE_ENGINE ?? "podman";
  if (engine !== "podman" && engine !== "docker")
    throw new Error("HMAIL_RELEASE_ENGINE must be podman or docker");
  const platform = environment.HMAIL_RELEASE_PLATFORM ?? "linux/amd64";
  if (platform !== "linux/amd64" && platform !== "linux/arm64")
    throw new Error(
      "HMAIL_RELEASE_PLATFORM must be linux/amd64 or linux/arm64",
    );
  const allowDirty = environment.HMAIL_RELEASE_ALLOW_DIRTY ?? "0";
  if (allowDirty !== "0" && allowDirty !== "1")
    throw new Error("HMAIL_RELEASE_ALLOW_DIRTY must be 0 or 1");
  const imageRepository =
    environment.HMAIL_RELEASE_IMAGE ?? "localhost/hmail-center";
  if (!IMAGE_REPOSITORY_PATTERN.test(imageRepository))
    throw new Error("Invalid HMAIL_RELEASE_IMAGE repository");
  const context = environment.HMAIL_RELEASE_DOCKER_CONTEXT;
  if (
    context !== undefined &&
    (engine !== "docker" || !/^[A-Za-z0-9_.-]+$/.test(context))
  )
    throw new Error("A Docker context is valid only for the Docker engine");
  const output = environment.HMAIL_RELEASE_OUTPUT ?? "target/releases";
  if (!output || /[\u0000-\u001f]/.test(output))
    throw new Error("Invalid HMAIL_RELEASE_OUTPUT directory");
  return {
    engine,
    context,
    platform,
    imageRepository,
    output,
    allowDirty: allowDirty === "1",
    build: configuredBuildVersion(environment),
  };
};
const baseImages = async (root: string): Promise<BaseImages> => {
  const value: unknown = await Bun.file(
    join(root, "deploy/base-images.json"),
  ).json();
  if (
    typeof value !== "object" ||
    value === null ||
    !("bun" in value) ||
    typeof value.bun !== "string" ||
    !("rust" in value) ||
    typeof value.rust !== "string" ||
    !("runtime" in value) ||
    typeof value.runtime !== "string" ||
    !("buildkit" in value) ||
    typeof value.buildkit !== "string"
  )
    throw new Error("Invalid base image lock");
  return {
    bun: pinnedImage(value.bun),
    rust: pinnedImage(value.rust),
    runtime: pinnedImage(value.runtime),
    buildkit: pinnedImage(value.buildkit),
  };
};
const packageVersion = async (root: string): Promise<string> => {
  const metadata: unknown = Bun.TOML.parse(
    await Bun.file(join(root, "crates/cli/Cargo.toml")).text(),
  );
  if (
    typeof metadata !== "object" ||
    metadata === null ||
    !("package" in metadata) ||
    typeof metadata.package !== "object" ||
    metadata.package === null ||
    !("version" in metadata.package) ||
    typeof metadata.package.version !== "string" ||
    !BUILD_PATTERN.test(metadata.package.version)
  )
    throw new Error("Missing package version");
  return metadata.package.version;
};
export const readReleaseConfig = async (): Promise<ReleaseConfig> => {
  const environment: ReleaseEnvironment = { ...Bun.env };
  const root = resolve(import.meta.dir, "../..");
  const options = parseReleaseOptions(environment);
  const source = await sourceIdentity(root);
  if (source.dirty && !options.allowDirty)
    throw new Error(
      "Release source is dirty; commit it or explicitly set HMAIL_RELEASE_ALLOW_DIRTY=1 for a local fingerprinted build",
    );
  const build =
    options.build ??
    `${await packageVersion(root)}+${source.revision}.${Math.floor(Date.now() / MILLISECONDS_PER_SECOND)}`;
  if (!BUILD_PATTERN.test(build))
    throw new Error("Invalid common build identity");
  const directory = resolve(root, options.output, build);
  const imageTag = `${options.imageRepository}:build-${build.replaceAll("+", "_").slice(0, IMAGE_TAG_BUILD_LENGTH)}-${Bun.SHA256.hash(build, "hex").slice(0, IMAGE_TAG_FINGERPRINT_LENGTH)}`;
  return {
    root,
    options,
    source,
    build,
    directory,
    imageTag,
    bases: await baseImages(root),
    environment: {
      ...environment,
      HMAIL_BUILD_VERSION: build,
      HMAIL_SOURCE_REVISION: source.revision,
      PUBLIC_BUILD_VERSION: build,
      ...(options.engine === "docker" && options.context
        ? { HMAIL_BUILD_DOCKER_CONTEXT: options.context }
        : {}),
    },
  };
};
export const engineArgv = (
  options: Pick<ReleaseOptions, "engine" | "context">,
): readonly string[] =>
  options.engine === "docker" && options.context
    ? ["docker", "--context", options.context]
    : [options.engine];
export const readVerificationConfig = (): VerificationConfig => {
  const environment: ReleaseEnvironment = { ...Bun.env };
  const arguments_ = Bun.argv.slice(2);
  if (arguments_.length > 1)
    throw new Error("Expected one release-manifest.json path");
  const path = arguments_[0] ?? environment.HMAIL_RELEASE_MANIFEST;
  if (!path)
    throw new Error(
      "Pass a release-manifest.json path or set HMAIL_RELEASE_MANIFEST",
    );
  const root = resolve(import.meta.dir, "../..");
  const manifestPath = resolve(root, path);
  return {
    root,
    options: parseReleaseOptions(environment),
    manifestPath,
    directory: resolve(manifestPath, ".."),
  };
};
