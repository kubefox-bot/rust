import { BASE_IMAGE_ARGUMENTS } from "./conts";
import type { BaseImages } from "./types";

export const pinContainerfile = ({
  source,
  bases,
}: Readonly<{ source: string; bases: BaseImages }>): string => {
  let rendered = source;
  for (const [argument, image] of Object.entries(BASE_IMAGE_ARGUMENTS)) {
    const declaration = new RegExp(`^ARG ${argument}$`, "gm");
    if (rendered.match(declaration)?.length !== 1)
      throw new Error(`Containerfile must declare exactly one ARG ${argument}`);
    rendered = rendered.replace(declaration, `ARG ${argument}=${bases[image]}`);
  }
  return rendered;
};
