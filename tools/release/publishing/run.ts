import { join } from "node:path";
import { publishRelease } from "../publish";
import { readPublicationConfig } from "./config";
import { githubClient } from "./github";
import { copyVerifiedImage } from "./image";
import { readVerifiedRelease } from "./gate";
import { createDraft, uploadAssets, verifyAssets } from "./remote";
import { signHostPackage } from "./host-package";
import { releaseAssets } from "./release";
import { signDelivery } from "./signing";
import { publishedDescriptors } from "./channel";
import { promoteRelease } from "./promote";
import { DELIVERY_FILE } from "../delivery-descriptor/conts";
import { parseDeliveryDescriptor } from "../delivery-descriptor";
import type { PublishedRelease } from "./types";

const config = readPublicationConfig();
const client = githubClient(config);
switch (config.mode) {
  case "publish": {
    const { manifest } = await readVerifiedRelease(config);
    const release = await publishRelease({
      ...config,
      ports: {
        copyImage: () => copyVerifiedImage({ config, manifest }),
        createDraft: () => createDraft({ client, config }),
        sign: async (path) => {
          await signHostPackage(config);
          return signDelivery({ config, path });
        },
        upload: (releaseId, files) =>
          uploadAssets({ client, releaseId, files }),
        verifyAssets: (releaseId, names) =>
          verifyAssets({ client, config, releaseId, names }),
      },
    });
    console.log(JSON.stringify(release));
    break;
  }
  case "promote": {
    await readVerifiedRelease(config);
    const descriptor = parseDeliveryDescriptor(
      await Bun.file(join(config.directory, DELIVERY_FILE)).text(),
    );
    const value: unknown = await Bun.file(
      join(config.directory, "publication.json"),
    ).json();
    if (
      typeof value !== "object" ||
      value === null ||
      !("releaseId" in value) ||
      typeof value.releaseId !== "number"
    )
      throw new Error("Missing completed publication receipt");
    const release: PublishedRelease = {
      sourceSha: config.sourceSha,
      sequence: config.sequence,
      imageDigest: descriptor.image.digest,
      releaseId: value.releaseId,
    };
    console.log(
      await promoteRelease({
        release,
        descriptor,
        ports: {
          currentHead: async () => {
            const value = await client.json({ path: "/git/ref/heads/main" });
            if (
              typeof value !== "object" ||
              value === null ||
              !("object" in value) ||
              typeof value.object !== "object" ||
              value.object === null ||
              !("sha" in value.object) ||
              typeof value.object.sha !== "string"
            )
              throw new Error("Missing main HEAD");
            return value.object.sha;
          },
          publishedDescriptors: () => publishedDescriptors({ client, config }),
          publishDraft: async (releaseId) => {
            await verifyAssets({
              client,
              config,
              releaseId,
              names: releaseAssets({
                directory: config.directory,
                build: descriptor.build,
              }).map((file) => file.name),
            });
            await client.json({
              path: `/releases/${releaseId}`,
              method: "PATCH",
              body: { draft: false, make_latest: "false" },
            });
          },
        },
      }),
    );
    break;
  }
  default: {
    const unexpected: never = config.mode;
    throw new Error(`Unexpected publication mode ${unexpected}`);
  }
}
