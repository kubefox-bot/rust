import { join } from "node:path";
import { MILLISECONDS_PER_SECOND, REVISION_PATTERN } from "./conts";

export const readBuildVersion = async (root: string): Promise<string> => {
  const metadata: unknown = Bun.TOML.parse(await Bun.file(join(root, "crates/cli/Cargo.toml")).text());
  if (typeof metadata !== "object" || metadata === null || !("package" in metadata)) {
    throw new Error("Missing package metadata");
  }
  const pkg = metadata.package;
  if (typeof pkg !== "object" || pkg === null || !("version" in pkg) || typeof pkg.version !== "string") {
    throw new Error("Missing package version");
  }
  const revision = Bun.spawn(["git", "rev-parse", "--short=12", "HEAD"], { cwd: root, stdout: "pipe", stderr: "pipe" });
  const hash = (await new Response(revision.stdout).text()).trim();
  if (await revision.exited !== 0 || !REVISION_PATTERN.test(hash)) {
    throw new Error("Cannot resolve the source revision");
  }
  const dirty = Bun.spawn(["git", "diff", "--quiet", "HEAD"], { cwd: root, stdout: "ignore", stderr: "pipe" });
  const status = await dirty.exited;
  if (status !== 0 && status !== 1) throw new Error("Cannot inspect source state");
  const suffix = status === 1 ? "-dirty" : "";
  const timestamp = Math.floor(Date.now() / MILLISECONDS_PER_SECOND);
  return `${pkg.version}+${hash}${suffix}.${timestamp}`;
};
