import { expect, test } from "bun:test";
import { localImageId } from "../delivery/local-image";

const expected = {
  build: "0.1.0+fixture.1",
  source_revision: "0123456789ab",
  image: {
    digest: `sha256:${"a".repeat(64)}`,
    config_digest: `sha256:${"b".repeat(64)}`,
    reference: `localhost/hmail-center@sha256:${"a".repeat(64)}`,
    local_tag: "localhost/hmail-center:fixture",
    platform: "linux/amd64",
  },
} as const;
const image = {
  Id: expected.image.digest,
  Os: "linux",
  Architecture: "amd64",
  Config: {
    Labels: {
      "org.opencontainers.image.version": "0.1.0+fixture.1",
      "org.opencontainers.image.revision": "0123456789ab",
    },
  },
};

test("Docker image binding accepts both containerd manifest IDs and classic config IDs", () => {
  expect(localImageId({ value: [image], expected })).toBe(
    expected.image.digest,
  );
  expect(
    localImageId({
      value: [{ ...image, Id: expected.image.config_digest }],
      expected,
    }),
  ).toBe(expected.image.config_digest);
});

test("Docker image binding rejects a moved tag, wrong platform and mismatched build labels", () => {
  for (const changed of [
    { ...image, Id: `sha256:${"c".repeat(64)}` },
    { ...image, Architecture: "arm64" },
    { ...image, Os: "windows" },
    {
      ...image,
      Config: {
        Labels: {
          ...image.Config.Labels,
          "org.opencontainers.image.version": "another-build",
        },
      },
    },
    {
      ...image,
      Config: {
        Labels: {
          ...image.Config.Labels,
          "org.opencontainers.image.revision": "another-source",
        },
      },
    },
  ])
    expect(() => localImageId({ value: [changed], expected })).toThrow(
      "verified release",
    );
});

test("Docker image binding rejects missing, multiple and malformed inspect results", () => {
  for (const value of [
    undefined,
    null,
    [],
    [image, image],
    {},
    [null],
    [{ Id: image.Id }],
    [{ ...image, Config: { Labels: null } }],
  ])
    expect(() => localImageId({ value, expected })).toThrow("verified release");
});
