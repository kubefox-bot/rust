import { defineConfig } from "astro/config";
import svelte from "@astrojs/svelte";
import { fileURLToPath } from "node:url";
import { precompressAssets } from "../../tools/web/compression/index.ts";
export default defineConfig({
  output: "static",
  trailingSlash: "always",
  integrations: [
    svelte(),
    {
      name: "precompressed-assets",
      hooks: {
        "astro:build:done": async ({ dir }) => {
          await precompressAssets({ directory: fileURLToPath(dir) });
        },
      },
    },
  ],
  vite: {
    css: { transformer: "lightningcss" },
    build: { cssMinify: "lightningcss" },
  },
  prefetch: false,
  build: { format: "directory" },
});
