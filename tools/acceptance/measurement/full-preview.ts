import type { Page } from "playwright";
import { decode } from "../../../apps/web/src/api";
import {
  BROWSER_WRITE_HEADERS,
  PROTOCOL_LIMITS,
  PROTOCOL_RETENTION,
} from "./conts";
export const fullProtocolPreview = async (
  input: Readonly<{ page: Page; serverId: string }>,
): Promise<void> => {
  const context = decode(
    "cleanup_context",
    await input.page.evaluate(async (serverId) => {
      const response = await fetch(
        `/api/v1/servers/${serverId}/cleanup-context`,
      );
      if (!response.ok) throw new Error("Fixture cleanup context unavailable");
      return response.json();
    }, input.serverId),
  );
  const cutoff = decode(
    "cutoff",
    await input.page.evaluate(
      async ({ serverId, headers, retention }) => {
        const response = await fetch(`/api/v1/servers/${serverId}/resolve`, {
          method: "POST",
          headers,
          body: JSON.stringify(retention),
        });
        if (!response.ok) throw new Error("Fixture cutoff unavailable");
        return response.json();
      },
      {
        serverId: input.serverId,
        headers: BROWSER_WRITE_HEADERS,
        retention: PROTOCOL_RETENTION,
      },
    ),
  );
  const body = decode("submit", {
    job_id: crypto.randomUUID(),
    server_id: input.serverId,
    profile_version: context.profile_version,
    operation: "preview",
    scan: "full",
    resolved_cutoff: cutoff,
    service_policy: "require_stopped",
    limits: PROTOCOL_LIMITS,
    preview_job_id: null,
    confirmed: false,
  });
  decode(
    "job",
    await input.page.evaluate(
      async ({ body, headers }) => {
        const response = await fetch(`/api/v1/servers/${body.server_id}/jobs`, {
          method: "POST",
          headers,
          body: JSON.stringify(body),
        });
        if (!response.ok)
          throw new Error("Fixture full preview dispatch failed");
        return response.json();
      },
      { body, headers: BROWSER_WRITE_HEADERS },
    ),
  );
  await input.page.reload();
};
