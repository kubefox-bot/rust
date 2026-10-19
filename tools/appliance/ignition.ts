import Ajv from "ajv";
import { join } from "node:path";
import { requireRelativeFile } from "./inventory";
import { HASH_PATTERN } from "./conts";
type Base = Readonly<{
  variant: "fcos";
  version: "1.6.0";
  passwd: {
    groups: readonly { name: string; gid: number }[];
    users: readonly {
      name: string;
      uid?: number;
      primary_group?: string;
      no_user_group?: boolean;
      home_dir?: string;
      shell?: string;
      ssh_authorized_keys_local?: readonly string[];
    }[];
  };
  storage: {
    directories: readonly {
      path: string;
      mode: number;
      user: { name: string };
      group: { name: string };
    }[];
    files: readonly {
      path: string;
      mode?: number;
      contents?: { local: string };
      append?: readonly { inline: string }[];
    }[];
  };
}>;
const named = {
  type: "object",
  additionalProperties: false,
  required: ["name"],
  properties: { name: { type: "string" } },
};
const validate = new Ajv({ strict: true }).compile<Base>({
  type: "object",
  additionalProperties: false,
  required: ["variant", "version", "passwd", "storage"],
  properties: {
    variant: { const: "fcos" },
    version: { const: "1.6.0" },
    passwd: {
      type: "object",
      additionalProperties: false,
      required: ["groups", "users"],
      properties: {
        groups: {
          type: "array",
          items: {
            type: "object",
            additionalProperties: false,
            required: ["name", "gid"],
            properties: { name: { type: "string" }, gid: { type: "integer" } },
          },
        },
        users: {
          type: "array",
          items: {
            type: "object",
            additionalProperties: false,
            required: ["name"],
            properties: {
              name: { type: "string" },
              uid: { type: "integer" },
              primary_group: { type: "string" },
              no_user_group: { type: "boolean" },
              home_dir: { type: "string" },
              shell: { type: "string" },
              ssh_authorized_keys_local: {
                type: "array",
                items: { type: "string" },
              },
            },
          },
        },
      },
    },
    storage: {
      type: "object",
      additionalProperties: false,
      required: ["directories", "files"],
      properties: {
        directories: {
          type: "array",
          items: {
            type: "object",
            additionalProperties: false,
            required: ["path", "mode", "user", "group"],
            properties: {
              path: { type: "string" },
              mode: { type: "integer" },
              user: named,
              group: named,
            },
          },
        },
        files: {
          type: "array",
          items: {
            type: "object",
            additionalProperties: false,
            required: ["path"],
            properties: {
              path: { type: "string" },
              mode: { type: "integer" },
              contents: {
                type: "object",
                additionalProperties: false,
                required: ["local"],
                properties: { local: { type: "string" } },
              },
              append: {
                type: "array",
                items: {
                  type: "object",
                  additionalProperties: false,
                  required: ["inline"],
                  properties: { inline: { type: "string" } },
                },
              },
            },
          },
        },
      },
    },
  },
});
const resource = (value: string) => ({
  source: `data:;base64,${Buffer.from(value).toString("base64")}`,
});
export const generateIgnition = async ({
  root,
  payloadSha256,
}: Readonly<{ root: string; payloadSha256: string }>): Promise<object> => {
  if (!HASH_PATTERN.test(payloadSha256))
    throw new Error("Invalid trusted payload digest");
  const base: unknown = Bun.YAML.parse(
    await Bun.file(join(root, "iac/fcos/config.bu")).text(),
  );
  if (!validate(base))
    throw new Error(
      "Unsupported FCOS base configuration; update appliance translation explicitly",
    );
  const files = await Promise.all(
    base.storage.files.map(async (file) => ({
      path: file.path,
      mode: file.mode ?? 0o644,
      ...(file.contents
        ? {
            contents: resource(
              await Bun.file(
                await requireRelativeFile(
                  join(root, "iac/fcos/files"),
                  file.contents.local,
                ),
              ).text(),
            ),
          }
        : {}),
      ...(file.append
        ? { append: file.append.map((entry) => resource(entry.inline)) }
        : {}),
    })),
  );
  const prepare = await Bun.file(
    join(root, "deploy/appliance/prepare.sh"),
  ).text();
  return {
    ignition: { version: "3.5.0" },
    passwd: {
      groups: base.passwd.groups,
      users: base.passwd.users.map((user) => ({
        name: user.name,
        ...(user.uid === undefined ? {} : { uid: user.uid }),
        ...(user.primary_group === undefined
          ? {}
          : { primaryGroup: user.primary_group }),
        ...(user.no_user_group === undefined
          ? {}
          : { noUserGroup: user.no_user_group }),
        ...(user.home_dir === undefined ? {} : { homeDir: user.home_dir }),
        ...(user.shell === undefined ? {} : { shell: user.shell }),
        ...(user.name === "core" ? { sshAuthorizedKeys: [] } : {}),
      })),
    },
    storage: {
      directories: [
        ...base.storage.directories,
        { path: "/var/lib/astra-appliance-install", mode: 0o755 },
      ],
      files: [
        ...files,
        {
          path: "/etc/astra-appliance/prepare.sh",
          mode: 0o700,
          contents: resource(prepare),
        },
        {
          path: "/var/lib/astra-appliance-install/origin",
          mode: 0o644,
          contents: resource("https://localhost:8443\n"),
        },
      ],
    },
    systemd: {
      units: [
        {
          name: "astra-appliance-prepare.service",
          enabled: true,
          contents: [
            "[Unit]",
            "Description=Verify and prepare offline Astra appliance",
            "Wants=network-online.target",
            "After=network-online.target user@2000.service",
            "Requires=user@2000.service",
            "Before=zincati.service",
            "ConditionPathExists=!/var/lib/astra-appliance-install/prepared",
            "[Service]",
            "Type=oneshot",
            "RemainAfterExit=yes",
            "TimeoutStartSec=15min",
            `Environment=ASTRA_PAYLOAD_SHA256=${payloadSha256}`,
            "ExecStart=/usr/bin/bash /etc/astra-appliance/prepare.sh",
            "[Install]",
            "WantedBy=multi-user.target",
            "",
          ].join("\n"),
        },
      ],
    },
  };
};
