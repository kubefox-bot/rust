import { expect, test } from "bun:test";
import {
  createDeliveryDescriptor,
  parseDeliveryDescriptor,
} from "../delivery-descriptor";
import { releaseFixture, sourceSha } from "./delivery-fixture";

const input = () => ({
  manifest: releaseFixture(),
  sourceSha,
  sequence: 7,
  releaseManifestSha256: "1".repeat(64),
});
test("descriptor binds clean common build, immutable image and full source SHA", () => {
  const result = createDeliveryDescriptor(input());
  expect(result).toMatchObject({
    format_version: 1,
    product: "astra-center",
    channel: "main",
    build: `git-${sourceSha}`,
    source_sha: sourceSha,
    sequence: 7,
    updater_min: 1,
    updater_max: 1,
    schema_version: 2,
  });
  expect(parseDeliveryDescriptor(JSON.stringify(result))).toEqual(result);
});
test("descriptor rejects dirty, wrong build, SHA, architecture and malformed bounds", () => {
  for (const mutation of [
    "dirty",
    "build",
    "sha",
    "platform",
    "sequence",
    "hash",
  ]) {
    const value = input();
    switch (mutation) {
      case "dirty":
        value.manifest = { ...value.manifest, dirty: true };
        break;
      case "build":
        value.manifest = { ...value.manifest, build: "other" };
        break;
      case "sha":
        value.sourceSha = "b".repeat(40);
        break;
      case "platform":
        value.manifest = {
          ...value.manifest,
          image: { ...value.manifest.image, platform: "linux/arm64" },
        };
        break;
      case "sequence":
        value.sequence = 0;
        break;
      case "hash":
        value.releaseManifestSha256 = "bad";
        break;
    }
    expect(() => createDeliveryDescriptor(value)).toThrow();
  }
});
test("parser rejects unknown fields, inconsistent digest, wrong product and oversized data", () => {
  const descriptor = createDeliveryDescriptor(input());
  for (const value of [
    { ...descriptor, extra: true },
    { ...descriptor, product: "other" },
    {
      ...descriptor,
      image: { ...descriptor.image, digest: `sha256:${"9".repeat(64)}` },
    },
    { ...descriptor, updater_max: 0 },
  ])
    expect(() => parseDeliveryDescriptor(JSON.stringify(value))).toThrow();
  expect(() => parseDeliveryDescriptor(" ".repeat(65537))).toThrow();
});
