export const CHECKS = [
  { name: "release", argv: ["mise", "run", "release:test"] },
  { name: "updater", argv: ["mise", "run", "delivery:test"] },
  {
    name: "maintenance",
    argv: [
      "mise",
      "exec",
      "--",
      "cargo",
      "test",
      "-p",
      "hmail-cleaner-center",
      "--test",
      "maintenance",
      "--test",
      "update_settings",
      "--locked",
    ],
  },
] as const;
export const TARGET_PHASES = [
  "GitHub CI and publication",
  "native Linux updater package",
  "FCOS image replacement and rollback",
  "FCOS OS update and reboot",
  "Hyper-V x86_64",
  "Windows Server 2016 agent end-to-end",
] as const;
