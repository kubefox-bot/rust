import { rm } from "node:fs/promises";
import { join } from "node:path";
import { engineArgv } from "./config";
import { OCI_ARCHIVE, RUST_TARGETS } from "./conts";
import { run } from "../lib/process";
import { stageBuildContext } from "./build-context";
import type { ReleaseConfig } from "./types";

export const buildImage = async (
  config: ReleaseConfig,
  agentDirectory: string,
): Promise<string> => {
  const archive = join(config.directory, OCI_ARCHIVE);
  const target = RUST_TARGETS[config.options.platform];
  const context = await stageBuildContext({
    root: config.root,
    directory: config.directory,
    bases: config.bases,
  });
  const arguments_ = [
    "--platform",
    config.options.platform,
    "--file",
    join(context, "deploy/Containerfile"),
    "--tag",
    config.imageTag,
    "--build-context",
    `agent-package=${agentDirectory}`,
    "--build-arg",
    `HMAIL_RUST_TARGET=${target.triple}`,
    "--build-arg",
    `HMAIL_GNU_PREFIX=${target.gnuPrefix}`,
    "--build-arg",
    `HMAIL_GNU_PACKAGES=${target.packages}`,
    "--build-arg",
    `HMAIL_ELF_MACHINE=${target.elfMachine}`,
    "--build-arg",
    `HMAIL_BUILD_VERSION=${config.build}`,
    "--build-arg",
    `HMAIL_SOURCE_REVISION=${config.source.revision}`,
    "--build-arg",
    `HMAIL_SOURCE_SHA=${config.environment.HMAIL_SOURCE_SHA ?? config.source.revision}`,
  ];
  switch (config.options.engine) {
    case "podman":
      await run({
        argv: [
          ...engineArgv(config.options),
          "build",
          "--format",
          "oci",
          ...arguments_,
          context,
        ],
        cwd: config.root,
        inherit: true,
      });
      await run({
        argv: [
          ...engineArgv(config.options),
          "save",
          "--format",
          "oci-archive",
          "--output",
          archive,
          config.imageTag,
        ],
        cwd: config.root,
        inherit: true,
      });
      break;
    case "docker": {
      const builder = `hmail-release-${crypto.randomUUID()}`;
      await run({
        argv: [
          ...engineArgv(config.options),
          "buildx",
          "create",
          "--name",
          builder,
          "--driver",
          "docker-container",
          "--driver-opt",
          `image=${config.bases.buildkit}`,
        ],
        cwd: config.root,
      });
      let originalFailure: unknown = undefined;
      try {
        await run({
          argv: [
            ...engineArgv(config.options),
            "buildx",
            "build",
            "--builder",
            builder,
            "--provenance=false",
            "--output",
            `type=oci,dest=${archive}`,
            "--output",
            `type=docker,name=${config.imageTag}`,
            ...arguments_,
            context,
          ],
          cwd: config.root,
          inherit: true,
        });
      } catch (error) {
        originalFailure = error;
        throw error;
      } finally {
        try {
          await run({
            argv: [
              ...engineArgv(config.options),
              "buildx",
              "rm",
              "--force",
              builder,
            ],
            cwd: config.root,
          });
        } catch (error) {
          throw new AggregateError(
            originalFailure === undefined ? [error] : [originalFailure, error],
            "Task-owned BuildKit cleanup failed",
          );
        }
      }
      break;
    }
    default: {
      const unexpected: never = config.options.engine;
      throw new Error(`Unsupported release engine: ${unexpected}`);
    }
  }
  await rm(context, { recursive: true });
  return archive;
};
