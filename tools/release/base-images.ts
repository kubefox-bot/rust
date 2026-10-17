import { engineArgv } from "./config";
import { run } from "../lib/process";
import type { ReleaseConfig } from "./types";

export const validateBaseImages = async (config: ReleaseConfig) => {
  for (const image of Object.values(config.bases)) {
    const argv =
      config.options.engine === "podman"
        ? [...engineArgv(config.options), "manifest", "inspect", image]
        : [
            ...engineArgv(config.options),
            "buildx",
            "imagetools",
            "inspect",
            image,
          ];
    await run({ argv, cwd: config.root });
  }
};
