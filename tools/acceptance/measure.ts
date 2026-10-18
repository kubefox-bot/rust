import {
  arch,
  cpus,
  freemem,
  loadavg,
  machine,
  platform,
  release,
  totalmem,
} from "node:os";
import { run } from "../lib/process";
import { artifact } from "../release/manifest";
import { nativeArtifact, staticArtifacts } from "./measurement/artifacts";
import {
  MEASUREMENT_OUTPUT,
  readMeasurementConfig,
} from "./measurement/config";
import {
  CENTER_RELATIVE_PATH,
  FIXTURE_RELATIVE_PATH,
} from "./measurement/conts";
import { releaseArtifacts } from "./measurement/release-artifacts";
import { measureWorkload } from "./measurement/workload";
import { validateMeasurementBuild } from "./measurement/build-receipt";
import {
  commitMeasurementReport,
  writeMeasurementFailure,
} from "./measurement/report";

const startedAt = new Date().toISOString();
const unavailable = [
  {
    name: "linux_rootless_podman_quadlet_container_rss_cpu",
    verdict: "NOT RUN",
    reason:
      "This command measures the current native host only; run the immutable release under rootless Podman + systemd/Quadlet on Linux for a deployment baseline",
  },
  {
    name: "windows_server_2016_agent_idle_cleanup_scm_rss_cpu",
    verdict: "NOT RUN",
    reason:
      "No explicitly provisioned native Windows Server 2016 SCM/MySQL/filesystem fixture host is attached; TLS agents here are protocol fixtures",
  },
  {
    name: "thirty_minute_slow_sse_resource_run",
    verdict: "NOT RUN",
    reason:
      "This is a bounded resource sampling run. The separate 30-minute slow-consumer acceptance scenario was not executed; short DOM/counter observations do not establish long-run memory stability",
  },
  {
    name: "real_mail_fast_full_cancel_throughput",
    verdict: "NOT RUN",
    reason:
      "Native center fast/full/cancel windows use two synthetic TLS protocol agents, not mail files or native SCM cleanup",
  },
  {
    name: "browser_total_process_memory",
    verdict: "NOT RUN",
    reason:
      "CDP Runtime.getHeapUsage measures renderer JavaScript heap only; total browser RSS/PSS, GPU memory and renderer process-tree memory are not measured",
  },
] as const;
try {
  await writeMeasurementFailure({
    config: { output: MEASUREMENT_OUTPUT },
    reason: "Measurement run is in progress; previous PASS is invalidated",
  });
  const config = readMeasurementConfig();
  const provenance = await validateMeasurementBuild({ config });
  if (platform() !== "darwin" && platform() !== "linux")
    throw new Error(
      "Native process sampling currently supports macOS and Linux ps counters only",
    );
  await nativeArtifact({ root: config.root, relative: CENTER_RELATIVE_PATH });
  await nativeArtifact({ root: config.root, relative: FIXTURE_RELATIVE_PATH });
  const [center, fixture, files, delivery, rustc, bun] = await Promise.all([
    artifact({ path: config.centerBinary, name: CENTER_RELATIVE_PATH }),
    artifact({ path: config.fixtureBinary, name: FIXTURE_RELATIVE_PATH }),
    staticArtifacts(config.staticDirectory),
    releaseArtifacts(config),
    run({ argv: ["rustc", "--version"], cwd: config.root }),
    run({ argv: ["bun", "--version"], cwd: config.root }),
  ]);
  const before = { freeMemoryBytes: freemem(), loadAverage: loadavg() };
  const measured = await measureWorkload(config);
  if (
    delivery.verdict === "PASS" &&
    delivery.build !== measured.centerBuildVersion
  )
    throw new Error(
      "Release manifest and native/web measurement build identities differ",
    );
  const report = {
    formatVersion: 1,
    verdict: "PASS",
    startedAt,
    finishedAt: new Date().toISOString(),
    scope:
      "Native release center on this host; separate fixture process contains both TLS protocol agents and is excluded from center counters",
    machine: {
      platform: platform(),
      release: release(),
      arch: arch(),
      machine: machine(),
      cpuModel: cpus()[0]?.model,
      logicalCpuCount: cpus().length,
      totalMemoryBytes: totalmem(),
      before,
      after: { freeMemoryBytes: freemem(), loadAverage: loadavg() },
    },
    versions: {
      rustc: rustc.trim(),
      bun: bun.trim(),
      playwright: "1.64.1",
      chromium: measured.browserVersion,
      sourceRevision: provenance.source.revision,
      sourceFingerprint: provenance.source.fingerprint,
      sourceDirty: provenance.source.dirty,
      centerBuild: measured.centerBuildVersion,
      webBuild: measured.webBuildVersion,
    },
    build: {
      requiredCommand:
        "cargo build --release -p hmail-cleaner --no-default-features --features center --locked",
      profile: "release",
      requestedFeatures: ["center"],
      defaultFeatures: false,
      artifact: center,
      fixtureArtifact: fixture,
    },
    methodology: {
      requestedSampleIntervalMs: config.sampleMs,
      minimumWindowMs: config.windowMs,
      process:
        "ps -p <owned center PID> -o rss= -o time=. RSS KiB converted to bytes; CPU is cumulative process time delta divided by monotonic sampled wall time, as percent of one core. Raw samples include command overhead. Peak is the largest sampled RSS, not a guaranteed instantaneous peak. CPU ps precision is host-dependent (normally 10ms).",
      browser:
        "Fresh Chromium context, trust restricted to generated fixture CA SPKI. PerformanceResourceTiming encodedBodySize/decodedBodySize/transferSize are browser-reported response body and transfer byte counts; transfer includes response headers and may be zero for cache hits. CDP Runtime.getHeapUsage is JS heap without forced GC. DOM nodes from Memory.getDOMCounters and elements/rows from selectors.",
      workload:
        "One login KDF, two connected TLS protocol agents, two open cleanup tabs, fast UI preview, full protocol preview with live UI observer, UI cancellation, history with 51 synthetic receipts (50 visible), selected settings. No cleanup files, SQL deletion or SCM effects are simulated as measured native work.",
    },
    native: measured.windows,
    browser: measured.browser,
    lazyPages: measured.lazy,
    workload: {
      counters: measured.counters,
      initialRequests: measured.initialRequests,
    },
    staticAssets: {
      totalBytes: files.reduce((total, file) => total + file.bytes, 0),
      jsBytes: files
        .filter((file) => file.path.endsWith(".js"))
        .reduce((total, file) => total + file.bytes, 0),
      files,
    },
    deliveryArtifacts: delivery,
    notRun: unavailable,
  };
  await commitMeasurementReport({
    config,
    build: provenance,
    report,
    runtime: {
      center: measured.centerBuildVersion,
      web: measured.webBuildVersion,
    },
  });
  console.log(`PASS resource measurements: ${config.output}`);
  for (const check of unavailable)
    console.log(`NOT RUN ${check.name}: ${check.reason}`);
} catch (error) {
  await writeMeasurementFailure({
    config: { output: MEASUREMENT_OUTPUT },
    reason:
      error instanceof Error ? error.message : "Resource measurement failed",
    details: { startedAt, notRun: unavailable },
  });
  throw error;
}
