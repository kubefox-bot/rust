import { join } from "node:path";
import {
  BUILD_PATTERN,
  DATA_DIRECTORY,
  DATA_VOLUME,
  CONTAINER_GID,
  CONTAINER_UID,
} from "./conts";
import { pinnedImage } from "./config";
import type { Deployment } from "./types";

export const deployment = async ({
  image,
  build,
}: Readonly<{ image: string; build: string }>): Promise<Deployment> => {
  pinnedImage(image);
  if (!BUILD_PATTERN.test(build))
    throw new Error("Invalid deployment build identity");
  const directory = join(import.meta.dir, "../../deploy/quadlet");
  const template = await Bun.file(
    join(directory, "hmail-center.container"),
  ).text();
  const container = template
    .replaceAll("@HMAIL_IMAGE@", image)
    .replaceAll("@HMAIL_BUILD@", build);
  if (/@HMAIL_/.test(container))
    throw new Error("Unresolved Quadlet deployment template");
  return {
    container,
    volume: await Bun.file(join(directory, "hmail-center-data.volume")).text(),
    init: [
      "podman",
      "run",
      "--rm",
      "-it",
      "--read-only",
      "--user",
      `${CONTAINER_UID}:${CONTAINER_GID}`,
      "--userns",
      `keep-id:uid=${CONTAINER_UID},gid=${CONTAINER_GID}`,
      "--volume",
      `${DATA_VOLUME}:${DATA_DIRECTORY}:Z,U`,
      image,
      "center",
      "init",
      "--data-dir",
      DATA_DIRECTORY,
    ],
  };
};
