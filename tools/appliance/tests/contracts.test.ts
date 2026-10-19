import { expect, test } from "bun:test";
import { parseLock, parsePayload } from "../contracts";
import pins from "../../../deploy/appliance/inputs.json";
test("appliance contracts reject unsupported schema, duplicate paths and unpinned inputs", () => {
  expect(parseLock(pins).fcos.release).toBe(pins.fcos.release);
  expect(() =>
    parseLock({
      ...pins,
      fcos: { ...pins.fcos, url: "http://example.test/fcos.zip" },
    }),
  ).toThrow();
  const manifest = {
    format_version: 1,
    product: "astra-appliance-payload",
    source_sha: "a".repeat(40),
    build: `git-${"a".repeat(40)}`,
    image: `ghcr.io/kubefox-bot/astra-center@sha256:${"b".repeat(64)}`,
    image_config_digest: `sha256:${"c".repeat(64)}`,
    schema_version: 2,
    files: [{ name: "image", size: 1, sha256: "d".repeat(64) }],
  };
  expect(parsePayload(manifest).schema_version).toBe(2);
  expect(() => parsePayload({ ...manifest, schema_version: 1 })).toThrow();
  expect(() =>
    parsePayload({
      ...manifest,
      files: [...manifest.files, ...manifest.files],
    }),
  ).toThrow();
});
