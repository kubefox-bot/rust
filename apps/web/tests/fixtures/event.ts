import type { BrowserContracts } from "../../src/api";
export const eventFixture = (
  epoch: string,
  revision: string,
): BrowserContracts["job_event"] => ({
  stream_epoch: epoch,
  revision,
  job: {
    job_id: "97f8b70f-cdbc-4201-a72c-5151391e7af1",
    dispatch: "acknowledged",
    slot_held: true,
    stream_epoch: epoch,
    revision,
    snapshot: null,
    intent: {
      job_id: "97f8b70f-cdbc-4201-a72c-5151391e7af1",
      server_id: "75e684ad-64d2-4f24-a587-e02b172d3ac0",
      profile_version: "2",
      operation: "preview",
      scan: "fast",
      resolved_cutoff: {
        local: "2026-01-01T00:00:00",
        file_cutoff_utc: "2026-01-01T00:00:00Z",
        utc_offset_seconds: 0,
        resolved_at_utc: "2026-10-18T00:00:00Z",
      },
      service_policy: "require_stopped",
      limits: { batch_size: 1000, workers: 2 },
      preview_job_id: null,
      confirmed: false,
    },
  },
});
