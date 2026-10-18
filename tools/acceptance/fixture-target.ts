import { lstat, realpath } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, isAbsolute, join, relative, sep } from "node:path";
import {
  FIXTURE_KIND,
  FIXTURE_MARKER,
  FIXTURE_ROOT_PREFIX,
  FIXTURE_SERVICE_PREFIX,
  MAX_MARKER_BYTES,
} from "./conts";
type FixtureTarget = Readonly<{
  fixture_id: string;
  root: string;
  data: string;
  service: string;
}>;
const inside = (root: string, path: string) => {
  const child = relative(root, path);
  return (
    child.length > 0 &&
    !child.startsWith(`..${sep}`) &&
    child !== ".." &&
    !isAbsolute(child)
  );
};
export const validateFixtureTarget = async (
  input: unknown,
): Promise<FixtureTarget> => {
  if (
    typeof input !== "object" ||
    input === null ||
    !("fixture_id" in input) ||
    typeof input.fixture_id !== "string" ||
    !("root" in input) ||
    typeof input.root !== "string" ||
    !("data" in input) ||
    typeof input.data !== "string" ||
    !("service" in input) ||
    typeof input.service !== "string"
  )
    throw new Error("Invalid fixture target");
  const value = {
    fixture_id: input.fixture_id,
    root: input.root,
    data: input.data,
    service: input.service,
  };
  if (
    !value.fixture_id ||
    !value.service.startsWith(FIXTURE_SERVICE_PREFIX) ||
    !basename(value.root).startsWith(FIXTURE_ROOT_PREFIX)
  )
    throw new Error("A dedicated synthetic fixture target is required");
  const root = await realpath(value.root);
  if (!inside(await realpath(tmpdir()), root))
    throw new Error("Fixture root must be temporary and isolated");
  const markerPath = join(root, FIXTURE_MARKER);
  const marker = await lstat(markerPath).catch(() => {
    throw new Error("A fixture marker is required");
  });
  if (!marker.isFile() || marker.size > MAX_MARKER_BYTES)
    throw new Error("Invalid fixture marker");
  const content: unknown = await Bun.file(markerPath).json();
  if (
    typeof content !== "object" ||
    content === null ||
    !("kind" in content) ||
    content.kind !== FIXTURE_KIND ||
    !("fixture_id" in content) ||
    content.fixture_id !== value.fixture_id
  )
    throw new Error("Fixture marker identity mismatch");
  const data = await realpath(value.data);
  if (!inside(root, data) || !(await lstat(value.data)).isDirectory())
    throw new Error("Fixture data escapes its synthetic root");
  return { ...value, root, data };
};
