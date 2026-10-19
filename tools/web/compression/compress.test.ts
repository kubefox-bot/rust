import { afterEach, expect, test } from "bun:test";
import {
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { brotliDecompressSync, gunzipSync } from "node:zlib";
import { precompressAssets } from "./index";

const roots: string[] = [];
afterEach(async () => {
  await Promise.all(
    roots.splice(0).map((root) => rm(root, { recursive: true, force: true })),
  );
});

const fixture = async () => {
  const root = await mkdtemp(join(tmpdir(), "hmail-web-compression-"));
  roots.push(root);
  await mkdir(join(root, "_astro/nested"), { recursive: true });
  return root;
};

test("emits Brotli and gzip sidecars only for built JavaScript and CSS", async () => {
  const root = await fixture();
  const files = {
    "_astro/app.Abc123.js": "export const label = 'cleanup';\n".repeat(20),
    "_astro/nested/app.Def456.css": ".cleanup { color: red; }\n".repeat(20),
    "_astro/app.Abc123.js.map": '{"version":3}',
    "_astro/icon.Ghi789.svg": "<svg/>",
    "index.html": "<!doctype html><title>Dashboard</title>",
    "public.js": "export const publicFile = true;",
  };
  await Promise.all(
    Object.entries(files).map(([name, content]) =>
      writeFile(join(root, name), content),
    ),
  );
  await precompressAssets({ directory: root });
  for (const name of [
    "_astro/app.Abc123.js",
    "_astro/nested/app.Def456.css",
  ] as const) {
    expect(
      brotliDecompressSync(await readFile(join(root, `${name}.br`))).toString(),
    ).toBe(files[name]);
    expect(
      gunzipSync(await readFile(join(root, `${name}.gz`))).toString(),
    ).toBe(files[name]);
    expect(await readFile(join(root, name), "utf8")).toBe(files[name]);
  }
  expect((await readdir(root)).sort()).toEqual([
    "_astro",
    "index.html",
    "public.js",
  ]);
  expect((await readdir(join(root, "_astro"))).sort()).toEqual([
    "app.Abc123.js",
    "app.Abc123.js.br",
    "app.Abc123.js.gz",
    "app.Abc123.js.map",
    "icon.Ghi789.svg",
    "nested",
  ]);
});

test("does not follow linked input files or directories outside the build", async () => {
  const root = await fixture();
  const outside = await fixture();
  await writeFile(join(outside, "secret.js"), "PRIVATE-DATA-SENTINEL");
  await symlink(join(outside, "secret.js"), join(root, "_astro/linked.js"));
  await symlink(outside, join(root, "_astro/linked"));
  await precompressAssets({ directory: root });
  expect((await readdir(join(root, "_astro"))).sort()).toEqual([
    "linked",
    "linked.js",
    "nested",
  ]);
  expect((await readdir(outside)).sort()).toEqual(["_astro", "secret.js"]);
});

test("propagates a missing build directory rather than silently succeeding", async () => {
  const root = await fixture();
  await expect(
    precompressAssets({ directory: join(root, "missing") }),
  ).rejects.toThrow();
});

test("replaces stale sidecars without following a linked sidecar target", async () => {
  const root = await fixture();
  const outside = await fixture();
  const source = join(root, "_astro/app.Abc123.js");
  const secret = join(outside, "secret.txt");
  await writeFile(source, "export const current=true;");
  await writeFile(`${source}.gz`, "stale compressed bytes");
  await writeFile(secret, "PRIVATE-DATA-SENTINEL");
  await symlink(secret, `${source}.br`);
  await precompressAssets({ directory: root });
  expect(await readFile(secret, "utf8")).toBe("PRIVATE-DATA-SENTINEL");
  expect(brotliDecompressSync(await readFile(`${source}.br`)).toString()).toBe(
    "export const current=true;",
  );
  expect(gunzipSync(await readFile(`${source}.gz`)).toString()).toBe(
    "export const current=true;",
  );
  await precompressAssets({ directory: root });
  expect(brotliDecompressSync(await readFile(`${source}.br`)).toString()).toBe(
    "export const current=true;",
  );
});
