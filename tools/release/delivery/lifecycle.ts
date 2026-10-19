import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { engineArgv } from "../config";
import {
  CENTER_EXECUTABLE,
  CENTER_PORT,
  CONTAINER_GID,
  CONTAINER_UID,
  HEALTH_ATTEMPTS,
  HEALTH_RETRY_MS,
  PRIVATE_DIRECTORY_MODE,
} from "../conts";
import { run } from "../../lib/process";
import type { ReleaseManifest, VerificationConfig } from "../types";
import { initialize } from "./initialize";
import { centerHttp } from "./http";
import { podmanHealthArguments, waitForContainerHealth } from "./health";
import { localImageId } from "./local-image";

const assert = (condition: boolean, message: string) => {
  if (!condition) throw new Error(message);
};
const backupIdentity = async (directory: string) => {
  const value: unknown = await Bun.file(
    join(directory, "manifest.json"),
  ).json();
  if (
    typeof value !== "object" ||
    value === null ||
    !("vault_id" in value) ||
    typeof value.vault_id !== "string" ||
    !("schema_version" in value) ||
    typeof value.schema_version !== "number" ||
    !("build_version" in value) ||
    typeof value.build_version !== "string"
  )
    throw new Error("Invalid disposable backup manifest");
  return {
    vaultId: value.vault_id,
    schema: value.schema_version,
    build: value.build_version,
  };
};
export const verifyLifecycle = async ({
  config,
  manifest,
}: Readonly<{
  config: VerificationConfig;
  manifest: ReleaseManifest;
}>): Promise<void> => {
  const engine = engineArgv(config.options);
  const command = (args: readonly string[]) =>
    run({ argv: [...engine, ...args], cwd: config.root });
  const image =
    config.options.engine === "podman"
      ? manifest.image.reference
      : localImageId({
          value: JSON.parse(
            await command(["image", "inspect", manifest.image.local_tag]),
          ),
          expected: manifest,
        });
  const id = crypto.randomUUID();
  const volume = `hmail-delivery-${id}`;
  const names = {
    init: `hmail-delivery-init-${id}`,
    first: `hmail-delivery-first-${id}`,
    backup: `hmail-delivery-backup-${id}`,
    second: `hmail-delivery-second-${id}`,
    backupAfter: `hmail-delivery-backup-after-${id}`,
  };
  const directory = join(config.directory, `delivery-${id}`);
  await mkdir(directory, { mode: PRIVATE_DIRECTORY_MODE });
  const user = [
    "--pull=never",
    "--label",
    `io.hmail.delivery.id=${id}`,
    "--read-only",
    "--user",
    `${CONTAINER_UID}:${CONTAINER_GID}`,
    "--security-opt",
    "no-new-privileges",
    "--cap-drop",
    "ALL",
    ...(config.options.engine === "podman"
      ? ["--userns", `keep-id:uid=${CONTAINER_UID},gid=${CONTAINER_GID}`]
      : []),
    "--volume",
    `${volume}:/data${config.options.engine === "podman" ? ":Z,U" : ""}`,
  ];
  const listener = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch: () => new Response("reserved"),
  });
  const port = listener.port;
  await listener.stop(true);
  if (!port) throw new Error("Cannot reserve the disposable center port");
  const origin = `https://localhost:${port}`;
  const password = `delivery-${crypto.randomUUID()}`;
  const login = "delivery-admin";
  const healthy = async (name: string) => {
    for (let attempt = 0; attempt < HEALTH_ATTEMPTS; attempt++) {
      try {
        await command([
          "exec",
          name,
          CENTER_EXECUTABLE,
          "center",
          "health",
          "--data-dir",
          "/data",
          "--expect-build",
          manifest.build,
        ]);
        return;
      } catch (error) {
        if (attempt + 1 === HEALTH_ATTEMPTS) throw error;
        await Bun.sleep(HEALTH_RETRY_MS);
      }
    }
  };
  const start = async (name: string) => {
    await command([
      "run",
      "--detach",
      "--name",
      name,
      ...user,
      ...(config.options.engine === "podman"
        ? podmanHealthArguments(manifest.build)
        : []),
      "--publish",
      `127.0.0.1:${port}:${CENTER_PORT}`,
      image,
    ]);
    await healthy(name);
    await waitForContainerHealth({
      inspect: async () => {
        const value: unknown = JSON.parse(await command(["inspect", name]));
        return value;
      },
      expectedBuild:
        config.options.engine === "podman" ? manifest.build : undefined,
    });
  };
  const snapshot = async (name: string, suffix: string) => {
    await command([
      "run",
      "--name",
      name,
      ...user,
      image,
      "center",
      "backup",
      "--data-dir",
      "/data",
      "--destination",
      `/data/delivery-snapshot-${suffix}`,
    ]);
    const destination = join(directory, suffix);
    await mkdir(destination, { mode: PRIVATE_DIRECTORY_MODE });
    for (const filename of ["manifest.json", "ca-cert.pem"])
      await command([
        "cp",
        `${name}:/data/delivery-snapshot-${suffix}/${filename}`,
        join(destination, filename),
      ]);
    return backupIdentity(destination);
  };
  await command([
    "volume",
    "create",
    "--label",
    "app=hmail-delivery-test",
    volume,
  ]);
  let originalFailure: unknown = undefined;
  try {
    await initialize({
      argv: [
        ...engine,
        "run",
        "--name",
        names.init,
        "-it",
        ...user,
        image,
        "center",
        "init",
        "--data-dir",
        "/data",
        "--origin",
        origin,
        "--listen",
        `0.0.0.0:${CENTER_PORT}`,
      ],
      cwd: config.root,
      login,
      password,
    });
    await start(names.first);
    await command([
      "cp",
      `${names.first}:/data/ca-cert.pem`,
      join(directory, "ca-cert.pem"),
    ]);
    const http = centerHttp({
      origin,
      ca: await Bun.file(join(directory, "ca-cert.pem")).text(),
    });
    const health = await http({ path: "/healthz" });
    assert(health.ok, "Health must succeed before login");
    for (const path of ["/", "/history/", "/settings/", "/login/"]) {
      const response = await http({ path });
      assert(
        response.ok &&
          response.headers.get("content-type")?.includes("text/html") === true,
        "Production static route failed",
      );
      assert(
        (await response.text()).includes(manifest.build),
        "Frontend build identity does not match the binary and manifest",
      );
    }
    const missing = await http({ path: "/api/v1/missing" });
    assert(
      missing.status === 404 &&
        missing.headers.get("content-type")?.includes("text/html") !== true,
      "API 404 must not return HTML",
    );
    const response = await http({
      path: "/api/v1/auth/login",
      method: "POST",
      body: { login, password },
    });
    assert(response.ok, "Disposable login failed");
    const cookie = response.headers
      .getSetCookie()
      .map((value) => value.split(";")[0])
      .filter(Boolean)
      .join("; ");
    assert(cookie.length > 0, "Disposable login did not establish a cookie");
    await command(["stop", "--time", "30", names.first]);
    assert(
      (
        await command([
          "inspect",
          "--format",
          "{{.State.ExitCode}}",
          names.first,
        ])
      ).trim() === "0",
      "SIGTERM did not complete gracefully",
    );
    const before = await snapshot(names.backup, "before");
    assert(before.build === manifest.build, "Backup build identity mismatch");
    await start(names.second);
    const revoked: unknown = await (
      await http({ path: "/api/v1/auth/session", cookie })
    ).json();
    assert(
      typeof revoked === "object" &&
        revoked !== null &&
        "authenticated" in revoked &&
        revoked.authenticated === false,
      "Restart retained an authenticated session",
    );
    await command(["stop", "--time", "30", names.second]);
    const after = await snapshot(names.backupAfter, "after");
    assert(
      after.vaultId === before.vaultId &&
        after.schema === before.schema &&
        after.build === before.build,
      "Recreation changed the vault or schema identity",
    );
    const firstCa = Bun.SHA256.hash(
      await Bun.file(join(directory, "before/ca-cert.pem")).bytes(),
      "hex",
    );
    const secondCa = Bun.SHA256.hash(
      await Bun.file(join(directory, "after/ca-cert.pem")).bytes(),
      "hex",
    );
    assert(firstCa === secondCa, "Recreation changed the public CA identity");
    await Bun.write(
      join(directory, "delivery-report.json"),
      `${JSON.stringify({ build: manifest.build, engine: config.options.engine, platform: manifest.image.platform, image_digest: manifest.image.digest, checks: { prelogin_https_health: "PASS", direct_container_healthcheck: "PASS", engine_health_state_before_login: "PASS", static_routes: "PASS", api_404: "PASS", credential_login: "PASS", graceful_sigterm: "PASS", stopped_backup: "PASS", recreate_vault_and_ca: "PASS", restart_revokes_session: "PASS", native_quadlet_logout_reboot: "NOT RUN", active_remote_agent_job: "NOT RUN" } }, null, 2)}\n`,
    );
    console.log(`Delivery lifecycle: ${directory}`);
  } catch (error) {
    originalFailure = error;
    throw error;
  } finally {
    const failures: unknown[] = [];
    try {
      const owned = (
        await command([
          "ps",
          "--all",
          "--filter",
          `label=io.hmail.delivery.id=${id}`,
          "--format",
          "{{.Names}}",
        ])
      )
        .trim()
        .split("\n")
        .filter(Boolean);
      for (const name of owned) await command(["rm", "--force", name]);
    } catch (error) {
      failures.push(error);
    }
    try {
      await command(["volume", "rm", volume]);
    } catch (error) {
      failures.push(error);
    }
    if (failures.length)
      throw new AggregateError(
        originalFailure === undefined
          ? failures
          : [originalFailure, ...failures],
        "Disposable delivery cleanup failed",
      );
  }
};
