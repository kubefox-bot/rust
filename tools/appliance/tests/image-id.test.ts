import { expect, test } from "bun:test";
import { resolve } from "node:path";
test("Podman bare and prefixed config IDs match only the exact expected SHA256", async () => {
  const script = resolve(
    import.meta.dir,
    "../../../deploy/appliance/prepare-user.sh",
  );
  const digest = "a".repeat(64);
  const verify = async (id: string) => {
    const child = Bun.spawn(
      [
        "bash",
        "-c",
        'source "$1"; fixture_image_id=$2; podman() { printf "%s\\n" "$fixture_image_id"; }; verify_image fixture "$3"',
        "fixture",
        script,
        id,
        `sha256:${digest}`,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    const [code] = await Promise.all([
      child.exited,
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
    ]);
    return code;
  };
  expect(await verify(digest)).toBe(0);
  expect(await verify(`sha256:${digest}`)).toBe(0);
  expect(await verify("b".repeat(64))).not.toBe(0);
  expect(await verify(`sha256:sha256:${digest}`)).not.toBe(0);
});
