import type { Guest } from "./session";
export const selectRoot = (
  filesystems: string,
  labels: ReadonlyMap<string, string>,
): string => {
  const roots = filesystems
    .trim()
    .split("\n")
    .filter((line) => line.endsWith(": xfs"))
    .map((line) => line.slice(0, -5))
    .filter((device) => labels.get(device) === "root");
  if (roots.length !== 1 || !/^\/dev\/[A-Za-z0-9]+$/.test(roots[0]))
    throw new Error("Expected exactly one labeled FCOS XFS root");
  return roots[0];
};
export const mountRoot = async (
  guest: Guest,
  stage: "upstream" | "final",
): Promise<Readonly<{ root: string; varPath: string; identity: string }>> => {
  const filesystems = await guest.command("list-filesystems");
  const labels = new Map<string, string>();
  for (const line of filesystems.trim().split("\n")) {
    if (!line.endsWith(": xfs")) continue;
    const device = line.slice(0, -5);
    if (!/^\/dev\/[A-Za-z0-9]+$/.test(device))
      throw new Error("Unexpected guest device");
    labels.set(device, (await guest.command("vfs-label", device)).trim());
  }
  const root = selectRoot(filesystems, labels);
  await guest.command(stage === "final" ? "mount-ro" : "mount", root, "/");
  const stateroots = (await guest.command("ls", "/ostree/deploy"))
    .trim()
    .split("\n");
  if (stateroots.length !== 1 || !/^[a-z0-9_-]+$/.test(stateroots[0]))
    throw new Error("Ambiguous OSTree stateroot");
  const stateroot = `/ostree/deploy/${stateroots[0]}`;
  const deployments = (await guest.command("ls", `${stateroot}/deploy`))
    .trim()
    .split("\n")
    .filter((entry) => !entry.endsWith(".origin"));
  if (deployments.length !== 1 || !/^[0-9a-f]{64}\.0$/.test(deployments[0]))
    throw new Error("Expected an unbooted single OSTree deployment");
  const etc = `${stateroot}/deploy/${deployments[0]}/etc`;
  const machineId =
    (await guest.command("exists", `${etc}/machine-id`)).trim() === "true"
      ? (await guest.command("cat", `${etc}/machine-id`)).trim()
      : "";
  if (machineId !== "" && machineId !== "uninitialized")
    throw new Error("Template has an existing machine identity");
  const ssh =
    (await guest.command("is-dir", `${etc}/ssh`)).trim() === "true"
      ? await guest.command("find", `${etc}/ssh`)
      : "";
  if (ssh.split("\n").some((name) => /ssh_host_.*_key(?:\.pub)?$/.test(name)))
    throw new Error("Template contains SSH host identities");
  const varPath = `${stateroot}/var`;
  for (const name of [
    "lib/containers/storage",
    "home/hmail-center",
    "lib/astra-appliance-install",
    ...(stage === "upstream" ? ["lib/astra-appliance"] : []),
  ])
    if ((await guest.command("exists", `${varPath}/${name}`)).trim() === "true")
      throw new Error(`Template already contains instance state: ${name}`);
  const bootDigests: string[] = [];
  for (const device of ["/dev/sda1", "/dev/sda2", "/dev/sda3"])
    bootDigests.push(
      (await guest.command("checksum-device", "sha256", device)).trim(),
    );
  return {
    root,
    varPath,
    identity: JSON.stringify({
      filesystems,
      stateroot,
      deployments,
      machineId,
      bootDigests,
      uuid: (await guest.command("vfs-uuid", root)).trim(),
      partitions: await guest.command("part-list", "/dev/sda"),
    }),
  };
};
