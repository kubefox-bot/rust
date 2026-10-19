import { expect, test } from "bun:test";
import { chromium } from "playwright";
import { resolve, sep } from "node:path";
const BUILD_DIRECTORY = resolve(import.meta.dir, "../dist");

test("static cleanup shell defers history and settings code and requests", async () => {
  const requests: string[] = [];
  const server = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch: async (request) => {
      const path = new URL(request.url).pathname;
      requests.push(path);
      if (path === "/api/v1/auth/session")
        return Response.json({ authenticated: false, generation: null });
      const file = resolve(
        BUILD_DIRECTORY,
        `.${path}`,
        path.endsWith("/") ? "index.html" : "",
      );
      if (!file.startsWith(`${BUILD_DIRECTORY}${sep}`))
        return new Response(null, { status: 404 });
      const asset = Bun.file(file);
      return (await asset.exists())
        ? new Response(asset)
        : new Response(null, { status: 404 });
    },
  });
  try {
    const browser = await chromium.launch({ headless: true });
    try {
      const page = await browser.newPage();
      const failures: string[] = [];
      page.on("pageerror", (error) => failures.push(error.message));
      await page.goto(`http://127.0.0.1:${server.port}/`);
      await page
        .getByRole("link", { name: "Войти в центр", exact: true })
        .waitFor({ timeout: 5000 })
        .catch((error) => {
          throw new AggregateError(
            [error, ...failures],
            "Svelte shell hydration failed",
          );
        });
      expect(requests).toContain("/api/v1/auth/session");
      expect(
        requests.filter((path) =>
          /\/api\/v1\/(history|settings)|(?:History|Settings).*\.js/.test(path),
        ),
      ).toHaveLength(0);
      expect(failures).toEqual([]);
    } finally {
      await browser.close();
    }
  } finally {
    await server.stop(true);
  }
}, 30000);
