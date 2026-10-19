import { expect, test } from "bun:test";
import { createDraft } from "../publishing/remote";
import { githubClient } from "../publishing/github";
import type { PublicationConfig } from "../publishing/types";
const config: PublicationConfig = {
  root: "/tmp",
  directory: "/tmp",
  sourceSha: "a".repeat(40),
  sequence: 1,
  token: "fixture",
  registryAuthFile: "/tmp/auth.json",
  trustedRoot: "/tmp/root.json",
  cosign: "cosign",
  mode: "publish",
};
test("partial draft with descriptor but missing bundle can resume without becoming published", async () => {
  const client = githubClient(config, async (input) => {
    const path = new URL(input).pathname;
    if (path.endsWith("/releases/tags/center-" + config.sourceSha))
      return Response.json({
        id: 1,
        draft: true,
        target_commitish: config.sourceSha,
      });
    if (path.endsWith("/releases/1/assets"))
      return Response.json([
        { id: 2, name: "delivery.json", size: 16, state: "uploaded" },
      ]);
    if (path.endsWith("/releases/assets/2"))
      return new Response("descriptor bytes");
    throw new Error("Unexpected fixture request");
  });
  expect(await createDraft({ client, config })).toEqual({
    id: 1,
    published: false,
    descriptor: "descriptor bytes",
  });
});
test("API authorization failure cannot create a replacement draft", async () => {
  let writes = 0;
  const client = githubClient(config, async (_input, init) => {
    if (init?.method === "POST") writes++;
    return new Response("denied", { status: 401 });
  });
  await expect(createDraft({ client, config })).rejects.toThrow("401");
  expect(writes).toBe(0);
});
