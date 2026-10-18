import { join } from "node:path";
import { DELIVERY_FILE, DELIVERY_BUNDLE } from "../delivery-descriptor/conts";
import {
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_BUNDLE,
  HOST_PACKAGE_MANIFEST,
} from "../updater-package/conts";
import { AGENT_EXECUTABLE } from "../conts";
export const releaseAssets = ({
  directory,
  build,
}: Readonly<{ directory: string; build: string }>): ReadonlyArray<
  Readonly<{ name: string; path: string }>
> => [
  { name: DELIVERY_FILE, path: join(directory, DELIVERY_FILE) },
  { name: DELIVERY_BUNDLE, path: join(directory, DELIVERY_BUNDLE) },
  {
    name: "release-manifest.json",
    path: join(directory, "release-manifest.json"),
  },
  { name: "center.oci.tar", path: join(directory, "center.oci.tar") },
  {
    name: "hmail-center.container",
    path: join(directory, "quadlet/hmail-center.container"),
  },
  {
    name: "hmail-center-data.volume",
    path: join(directory, "quadlet/hmail-center-data.volume"),
  },
  {
    name: AGENT_EXECUTABLE,
    path: join(directory, "agent-packages", build, AGENT_EXECUTABLE),
  },
  {
    name: `${AGENT_EXECUTABLE}.sha256`,
    path: join(
      directory,
      "agent-packages",
      build,
      `${AGENT_EXECUTABLE}.sha256`,
    ),
  },
  {
    name: "agent-manifest.json",
    path: join(directory, "agent-packages", build, "manifest.json"),
  },
  {
    name: HOST_PACKAGE_ARCHIVE,
    path: join(directory, "host-updater", HOST_PACKAGE_ARCHIVE),
  },
  {
    name: HOST_PACKAGE_BUNDLE,
    path: join(directory, "host-updater", HOST_PACKAGE_BUNDLE),
  },
  {
    name: "astra-updater-package-manifest.json",
    path: join(directory, "host-updater", HOST_PACKAGE_MANIFEST),
  },
  { name: `astra-hyperv-${build}.zip`, path: join(directory, "appliance", `astra-hyperv-${build}.zip`) },
  { name: "appliance-manifest.json", path: join(directory, "appliance/appliance-manifest.json") },
  { name: "assembly-report.json", path: join(directory, "appliance/assembly-report.json") },
];
