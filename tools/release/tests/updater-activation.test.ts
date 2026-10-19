import { expect, test } from "bun:test";
import { resolve } from "node:path";

const installer = resolve(
  import.meta.dir,
  "../../../deploy/updater/install.sh",
);
const configure = async (mode: string) => {
  const process = Bun.spawn(
    [
      "bash",
      "-c",
      'source "$1"; systemctl() { printf "%s\\n" "$*"; }; configure_services "$2"',
      "fixture",
      installer,
      mode,
    ],
    { stdout: "pipe", stderr: "pipe" },
  );
  return {
    code: await process.exited,
    output: await new Response(process.stdout).text(),
  };
};
test("preparing updater reloads units without activating discovery or reboot", async () => {
  const result = await configure("prepare");
  expect(result.code).toBe(0);
  expect(result.output.trim()).toBe("--user daemon-reload");
});
test("explicit activation enables the existing coordinator and timer", async () => {
  const result = await configure("activate");
  expect(result.code).toBe(0);
  expect(result.output).toContain("--user start astra-updater-recover.service");
  expect(result.output).toContain(
    "--user enable --now astra-reboot-coordinator.service astra-updater.timer",
  );
});
