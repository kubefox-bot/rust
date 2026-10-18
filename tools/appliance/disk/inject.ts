import { copyFile, mkdir, rm } from "node:fs/promises";
import { join } from "node:path";
import { run } from "../../lib/process";
import { DISK_RESERVE_BYTES } from "../conts";
import { artifact } from "../../release/manifest";
import { withGuest } from "./session";
import { mountRoot } from "./layout";
import { createPayloadArchive } from "./archive";
import { requireMetadata } from "./metadata";
import type { Artifact } from "../types";
const requireVhdx = async (path: string): Promise<void> => {
  const info: unknown = JSON.parse(
    await run({ argv: ["qemu-img", "info", "--output=json", path], cwd: "/" }),
  );
  if (
    typeof info !== "object" ||
    info === null ||
    !("format" in info) ||
    info.format !== "vhdx" ||
    "backing-filename" in info ||
    !("virtual-size" in info) ||
    typeof info["virtual-size"] !== "number"
  )
    throw new Error("Expected standalone VHDX without backing files");
};
export const injectPayload = async ({
  upstream,
  output,
  payload,
  scratch,
  files,
}: Readonly<{
  upstream: string;
  output: string;
  payload: string;
  scratch: string;
  files: readonly Artifact[];
}>): Promise<Readonly<{ layout: string; tools: readonly string[] }>> => {
  await requireVhdx(upstream);
  await copyFile(upstream, output);
  const tar = join(scratch, "payload.tar");
  await createPayloadArchive({
    payload,
    tar,
    scratch,
    names: files.map((file) => file.name),
  });
  const identity = await withGuest({
    path: output,
    readonly: false,
    operation: async (guest) => {
      const layout = await mountRoot(guest, "upstream");
      const stat = await guest.command("statvfs", "/");
      const fields = new Map(
        stat
          .trim()
          .split("\n")
          .map((line) => {
            const [name, value] = line.split(": ");
            return [name, value] as const;
          }),
      );
      const free = Number(fields.get("bavail")) * Number(fields.get("frsize"));
      if (
        !Number.isSafeInteger(free) ||
        free <
          files.reduce((size, file) => size + file.size, 0) + DISK_RESERVE_BYTES
      )
        throw new Error(
          "Insufficient root filesystem space for appliance payload",
        );
      const destination = `${layout.varPath}/lib/astra-appliance`;
      await guest.command("mkdir-p", destination);
      await guest.command("tar-in", tar, destination);
      await guest.command("chmod", "0755", destination);
      await guest.command("sync");
      await guest.command("umount-all");
      return layout.identity;
    },
  });
  // Reopen the final bytes read-only and verify every payload file independently.
  await withGuest({
    path: output,
    readonly: true,
    operation: async (guest) => {
      const layout = await mountRoot(guest, "final");
      if (layout.identity !== identity)
        throw new Error("OS layout or first-boot identity changed");
      const destination = `${layout.varPath}/lib/astra-appliance`;
      const names = (await guest.command("find", destination))
        .trim()
        .split("\n")
        .map((name) => name.replace(/^\/+/, ""))
        .filter(Boolean);
      const actualFiles: string[] = [];
      requireMetadata(await guest.command("lstatns", destination), "directory");
      for (const name of names) {
        if (
          (
            await guest.command("is-symlink", `${destination}/${name}`)
          ).trim() === "true"
        )
          throw new Error("Unexpected payload symlink");
        if (
          (await guest.command("is-file", `${destination}/${name}`)).trim() ===
          "true"
        ) {
          requireMetadata(
            await guest.command("lstatns", `${destination}/${name}`),
            "file",
          );
          actualFiles.push(name);
        } else {
          requireMetadata(
            await guest.command("lstatns", `${destination}/${name}`),
            "directory",
          );
        }
      }
      if (
        JSON.stringify(actualFiles.sort()) !==
        JSON.stringify(files.map((file) => file.name).sort())
      )
        throw new Error("Final payload contains unexpected files");
      const readback = join(scratch, "readback");
      await mkdir(readback);
      for (const file of files) {
        const target = join(readback, "file");
        await guest.command("download", `${destination}/${file.name}`, target);
        const actual = await artifact({ path: target, name: file.name });
        if (actual.sha256 !== file.sha256 || actual.size !== file.size)
          throw new Error(`Injected bytes differ: ${file.name}`);
      }
      await guest.command("umount-all");
    },
  });
  await rm(tar);
  return {
    layout: identity,
    tools: await Promise.all([
      run({ argv: ["guestfish", "--version"], cwd: "/" }),
      run({ argv: ["qemu-img", "--version"], cwd: "/" }),
    ]),
  };
};
