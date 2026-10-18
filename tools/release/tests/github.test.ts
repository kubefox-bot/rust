import { expect, test } from "bun:test";
import { githubClient } from "../publishing/github";
import type { PublicationConfig } from "../publishing/types";

const config: PublicationConfig = {
  root: "/tmp",
  directory: "/tmp",
  sourceSha: "a".repeat(40),
  sequence: 1,
  token: "fixture-sensitive-token",
  registryAuthFile: "/tmp/auth.json",
  trustedRoot: "/tmp/root.json",
  cosign: "cosign",
  mode: "publish",
};
test("asset redirect sends credentials only to GitHub API and allows only approved HTTPS CDN", async () => {
  const requests: Readonly<{ url: string; headers: Headers }>[] = [];
  const client = githubClient(config, async (input, init) => {
    const url = String(input);
    requests.push({ url, headers: new Headers(init?.headers) });
    return requests.length === 1
      ? new Response(null, {
          status: 302,
          headers: {
            location:
              "https://release-assets.githubusercontent.com/asset?signature=fixture",
          },
        })
      : new Response("verified bytes");
  });
  expect(
    await client.text(
      await client.request({
        path: "/releases/assets/1",
        accept: "application/octet-stream",
      }),
    ),
  ).toBe("verified bytes");
  expect(requests[0].headers.get("Authorization")).toBe(
    "Bearer fixture-sensitive-token",
  );
  expect(requests[1].headers.has("Authorization")).toBe(false);
  for (const location of [
    "http://release-assets.githubusercontent.com/a",
    "https://attacker.example/a",
    "https://user:pass@release-assets.githubusercontent.com/a",
  ]) {
    const denied = githubClient(
      config,
      async () => new Response(null, { status: 302, headers: { location } }),
    );
    await expect(
      denied.request({
        path: "/releases/assets/1",
        accept: "application/octet-stream",
      }),
    ).rejects.toThrow("redirect");
  }
});
test("API failures and excessive metadata do not become missing releases", async () => {
  for (const status of [401, 403, 429, 500]) {
    const client = githubClient(
      config,
      async () => new Response("fixture-sensitive-token", { status }),
    );
    await expect(client.request({ path: "/releases" })).rejects.toThrow(
      `HTTP ${status}`,
    );
  }
  const bounded = githubClient(
    config,
    async () => new Response("x".repeat(2 * 1024 * 1024 + 1)),
  );
  await expect(bounded.json({ path: "/releases" })).rejects.toThrow("limit");
});
