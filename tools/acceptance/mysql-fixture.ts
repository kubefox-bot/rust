import { MYSQL_GUARD_TIMEOUT_MS } from "./conts";
import {
  CHECK_PROCESS_POLICY,
  readProcessOutput,
  type CheckExecutionOptions,
} from "./process-lifecycle";
import type { AcceptanceConfig } from "./types";

export const verifyMysqlFixture = async (
  config: AcceptanceConfig,
  options: CheckExecutionOptions = {},
): Promise<void> => {
  const policy = options.policy ?? {
    ...CHECK_PROCESS_POLICY,
    timeoutMs: MYSQL_GUARD_TIMEOUT_MS,
  };
  const endpoint = (
    await readProcessOutput({
      argv: [
        "docker",
        "context",
        "inspect",
        config.dockerContext,
        "--format",
        "{{.Endpoints.docker.Host}}",
      ],
      cwd: config.root,
      environment: config.environment,
      policy,
      signal: options.signal,
    })
  ).stdout.trim();
  if (!endpoint.startsWith("unix://") && !endpoint.startsWith("npipe://"))
    throw new Error("Database acceptance requires a local Docker context");
  const project = (
    await readProcessOutput({
      argv: [
        "docker",
        "--context",
        config.dockerContext,
        "inspect",
        "--format",
        '{{ index .Config.Labels "com.docker.compose.project" }}',
        "hmail-cleanup-tests-mysql-1",
      ],
      cwd: config.root,
      environment: config.environment,
      policy,
      signal: options.signal,
    })
  ).stdout.trim();
  if (project !== "hmail-cleanup-tests")
    throw new Error(
      "Start only the named disposable MySQL fixture before database acceptance",
    );
  const binding = (
    await readProcessOutput({
      argv: [
        "docker",
        "--context",
        config.dockerContext,
        "inspect",
        "--format",
        '{{with (index .NetworkSettings.Ports "3306/tcp")}}{{(index . 0).HostIp}}:{{(index . 0).HostPort}}{{end}}',
        "hmail-cleanup-tests-mysql-1",
      ],
      cwd: config.root,
      environment: config.environment,
      policy,
      signal: options.signal,
    })
  ).stdout.trim();
  if (binding !== "127.0.0.1:13306")
    throw new Error(
      "Disposable MySQL must use the verified loopback fixture port",
    );
};
