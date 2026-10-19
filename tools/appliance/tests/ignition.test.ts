import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { generateIgnition } from "../ignition";
test("Ignition retains private service home, rootless IDs and an inactive initialization guard", async () => {
  const config = await generateIgnition({
    root: resolve(import.meta.dir, "../../.."),
    payloadSha256: "a".repeat(64),
  });
  const text = JSON.stringify(config);
  expect(text).toContain('"mode":448');
  expect(text).not.toContain('"mode":700');
  expect(text).toContain('"uid":2000');
  expect(text).toContain("Before=zincati.service");
  expect(text).toContain("ConditionPathExists=!");
  expect(text).not.toContain("passwordHash");
});
