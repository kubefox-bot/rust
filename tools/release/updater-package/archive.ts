import { HOST_PACKAGE_FILES, HOST_PACKAGE_MANIFEST } from "./conts";
export const verifyArchiveInventory = async ({
  archive,
  files,
}: Readonly<{ archive: Bun.Archive; files: unknown }>): Promise<void> => {
  if (!Array.isArray(files)) throw new Error("Missing host package inventory");
  const entries = await archive.files();
  const expected = [
    ...HOST_PACKAGE_FILES,
    HOST_PACKAGE_MANIFEST,
    "SHA256SUMS",
  ].sort();
  if (JSON.stringify([...entries.keys()].sort()) !== JSON.stringify(expected))
    throw new Error("Unexpected host package members");
  const names: string[] = [];
  const lines: string[] = [];
  const inventory: readonly unknown[] = files;
  for (const file of inventory) {
    if (
      typeof file !== "object" ||
      file === null ||
      !("name" in file) ||
      typeof file.name !== "string" ||
      !("size" in file) ||
      typeof file.size !== "number" ||
      !Number.isSafeInteger(file.size) ||
      file.size < 1 ||
      !("sha256" in file) ||
      typeof file.sha256 !== "string" ||
      !/^[0-9a-f]{64}$/.test(file.sha256)
    )
      throw new Error("Invalid host package inventory entry");
    const entry = entries.get(file.name);
    if (!entry || entry.size !== file.size)
      throw new Error("Host package member size differs");
    const hash = new Bun.CryptoHasher("sha256");
    for await (const chunk of entry.stream()) hash.update(chunk);
    if (hash.digest("hex") !== file.sha256)
      throw new Error("Host package member hash differs");
    names.push(file.name);
    lines.push(`${file.sha256}  ${file.name}\n`);
  }
  if (
    JSON.stringify(names.sort()) !==
    JSON.stringify([...HOST_PACKAGE_FILES].sort())
  )
    throw new Error("Incomplete or duplicate host package inventory");
  const manifest = entries.get(HOST_PACKAGE_MANIFEST);
  const sums = entries.get("SHA256SUMS");
  if (!manifest || !sums) throw new Error("Missing host package metadata");
  const digest = Bun.SHA256.hash(await manifest.bytes(), "hex");
  if (
    (await sums.text()) !==
    lines.join("") + `${digest}  ${HOST_PACKAGE_MANIFEST}\n`
  )
    throw new Error("Host package checksum index differs");
};
