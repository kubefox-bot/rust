import {
  CENTER_HEALTH_ARGV,
  CONTAINER_HEALTH_ATTEMPTS,
  CONTAINER_HEALTH_OPTIONS,
  HEALTH_RETRY_MS,
} from "../conts";
import { requireDirectHealthcheck } from "../healthcheck";

export const podmanHealthArguments = (build: string): readonly string[] => [
  "--health-cmd",
  JSON.stringify([...CENTER_HEALTH_ARGV, "--expect-build", build]),
  "--health-interval",
  CONTAINER_HEALTH_OPTIONS.interval,
  "--health-timeout",
  CONTAINER_HEALTH_OPTIONS.timeout,
  "--health-start-period",
  CONTAINER_HEALTH_OPTIONS.startPeriod,
  "--health-retries",
  CONTAINER_HEALTH_OPTIONS.retries,
];

export const containerHealth = ({
  value,
  expectedBuild,
}: Readonly<{ value: unknown; expectedBuild?: string }>):
  "starting" | "healthy" | "unhealthy" => {
  if (!Array.isArray(value) || value.length !== 1)
    throw new Error("Expected one inspected delivery container");
  const containers: readonly unknown[] = value;
  const container = containers[0];
  if (
    typeof container !== "object" ||
    container === null ||
    !("Config" in container) ||
    typeof container.Config !== "object" ||
    container.Config === null ||
    !("Healthcheck" in container.Config)
  )
    throw new Error("Container configuration has no direct healthcheck");
  requireDirectHealthcheck({
    value: container.Config.Healthcheck,
    expectedBuild,
  });
  if (
    !("State" in container) ||
    typeof container.State !== "object" ||
    container.State === null ||
    !("Running" in container.State) ||
    container.State.Running !== true ||
    !("Health" in container.State) ||
    typeof container.State.Health !== "object" ||
    container.State.Health === null ||
    !("Status" in container.State.Health)
  )
    throw new Error("Delivery container is stopped or has no health state");
  const status = container.State.Health.Status;
  if (status !== "starting" && status !== "healthy" && status !== "unhealthy")
    throw new Error("Unknown container health state");
  return status;
};

export const waitForContainerHealth = async ({
  inspect,
  expectedBuild,
}: Readonly<{
  inspect: () => Promise<unknown>;
  expectedBuild?: string;
}>): Promise<void> => {
  for (let attempt = 0; attempt < CONTAINER_HEALTH_ATTEMPTS; attempt++) {
    const status = containerHealth({ value: await inspect(), expectedBuild });
    switch (status) {
      case "healthy":
        return;
      case "unhealthy":
        throw new Error("The engine marked the delivery container unhealthy");
      case "starting":
        if (attempt + 1 < CONTAINER_HEALTH_ATTEMPTS)
          await Bun.sleep(HEALTH_RETRY_MS);
        break;
      default: {
        const unexpected: never = status;
        throw new Error(`Unknown container health state: ${unexpected}`);
      }
    }
  }
  throw new Error(
    "The engine did not mark the delivery container healthy before the deadline",
  );
};
