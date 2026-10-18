import { expect, test } from "bun:test";
import { selectRoot } from "../disk/layout";
test("image factory selects the labeled XFS root and rejects ambiguity", () => {
  const filesystems = "/dev/sda1: vfat\n/dev/sda3: ext4\n/dev/sda4: xfs\n";
  expect(selectRoot(filesystems, new Map([["/dev/sda4", "root"]]))).toBe(
    "/dev/sda4",
  );
  expect(() => selectRoot(filesystems, new Map())).toThrow("exactly one");
  expect(() =>
    selectRoot(
      filesystems + "/dev/sdb4: xfs\n",
      new Map([
        ["/dev/sda4", "root"],
        ["/dev/sdb4", "root"],
      ]),
    ),
  ).toThrow("exactly one");
});
