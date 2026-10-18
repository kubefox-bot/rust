import { expect, test } from "bun:test";
import {
  initialState,
  transition,
  progressPercent,
  screenStatus,
} from "../src/features/cleanup/model";
import { eventFixture } from "./fixtures/event";
import { summaryFixture } from "./fixtures/summary";
const epoch = "7a96d933-25ae-4eca-b840-d573a62dc778";
const event = eventFixture(epoch, "2");
const snapshot = {
  job_id: event.job.job_id,
  job: null,
  stream_epoch: epoch,
  revision: "2",
  phase: "completed" as const,
  stage: null,
  summary: summaryFixture,
  mail_state: "running" as const,
  initial_mail_state: "running" as const,
  error: null,
  observed_at: "2026-10-18T00:00:00Z",
};
const preview = { ...event.job, slot_held: false, snapshot };
const context = {
  server: {
    id: event.job.intent.server_id,
    name: "Mail One",
    agent_id: "b68ff8df-8b98-482e-bdb4-91c6bf8a164d",
    profile_id: "b0c49013-f51f-4b25-a70f-fdf113c54ae2",
  },
  profile_version: "2",
  configured: true,
  latest: preview,
  busy: false,
};

test("changed parameters invalidate preview and confirmation", () => {
  const initial = initialState(context);
  expect(initial.preview?.job_id).toBe(preview.job_id);
  const confirmed = transition(initial, { type: "confirm" });
  expect(confirmed.confirmation?.serverId).toBe(context.server.id);
  const changed = transition(confirmed, {
    type: "parameters",
    parameters: { ...initial.parameters, scan: "full" },
  });
  expect(changed.preview).toBeNull();
  expect(changed.confirmation).toBeNull();
});
test("confirmation retains its target, profile and cutoff when another server context arrives", () => {
  const confirmed = transition(initialState(context), { type: "confirm" });
  const target = confirmed.confirmation;
  const other = transition(confirmed, {
    type: "context",
    context: {
      ...context,
      server: { ...context.server, id: "c68482e9-cdaa-4dfb-8fc8-18a4d935a307" },
    },
  });
  expect(target?.serverId).toBe(context.server.id);
  expect(other.confirmation).toBeNull();
  expect(target?.resolvedCutoff).toEqual(event.job.intent.resolved_cutoff);
});
test("restoration remains an active phase and unknown totals have no invented percent", () => {
  expect(
    screenStatus({
      ...preview,
      slot_held: true,
      snapshot: { ...snapshot, phase: "restoring" },
    }),
  ).toBe("Восстановление службы");
  expect(progressPercent(summaryFixture)).toBeNull();
  expect(
    progressPercent({
      ...summaryFixture,
      total_scan_files: "10",
      scanned_files: "3",
    }),
  ).toBe(30);
});
test("fullscreen layout preserves job, preview and bounded journal", () => {
  const initial = initialState(context);
  const expanded = transition(initial, { type: "expanded", expanded: true });
  const collapsed = transition(expanded, { type: "expanded", expanded: false });
  expect(collapsed.job).toEqual(initial.job);
  expect(collapsed.preview).toEqual(initial.preview);
  expect(collapsed.journal).toEqual(initial.journal);
});
test("stale or unrelated jobs cannot replace the selected run", () => {
  const initial = initialState(context);
  const stale = transition(initial, {
    type: "job",
    job: { ...preview, revision: "1" },
  });
  expect(stale.job?.revision).toBe("2");
  const unrelated = transition(initial, {
    type: "job",
    job: { ...preview, job_id: "50209868-1236-4d7b-b65c-9b56d70dcfe7" },
  });
  expect(unrelated.job?.job_id).toBe(preview.job_id);
});

test("submitting clean consumes the local preview and prevents another confirmation", () => {
  const initial = initialState(context);
  const sending = transition(initial, {
    type: "dispatch",
    intent: {
      ...preview.intent,
      operation: "clean",
      confirmed: true,
      preview_job_id: preview.job_id,
      job_id: "50209868-1236-4d7b-b65c-9b56d70dcfe7",
    },
  });
  expect(sending.preview).toBeNull();
  expect(transition(sending, { type: "confirm" }).confirmation).toBeNull();
});

test("journal stays bounded during a long operation", () => {
  const active = {
    ...preview,
    slot_held: true,
    snapshot: { ...snapshot, phase: "running" as const },
  };
  let state = initialState({ ...context, latest: active });
  for (let revision = 3; revision < 1103; revision++) {
    state = transition(state, {
      type: "job",
      job: {
        ...active,
        revision: String(revision),
        snapshot: { ...active.snapshot, revision: String(revision) },
      },
    });
  }
  expect(state.journal).toHaveLength(1000);
  expect(state.journal.at(-1)?.key).toEndWith(":1102");
});

test("authoritative latest replaces a settled run and invalidates its captured confirmation", () => {
  const confirmed = transition(initialState(context), { type: "confirm" });
  const target = confirmed.confirmation;
  const latest = {
    ...preview,
    job_id: "50209868-1236-4d7b-b65c-9b56d70dcfe7",
    slot_held: true,
    snapshot: { ...snapshot, phase: "running" as const },
  };
  const adopted = transition(confirmed, {
    type: "context",
    context: { ...context, latest, busy: true },
  });
  expect(adopted.job?.job_id).toBe(latest.job_id);
  expect(adopted.preview).toBeNull();
  expect(adopted.confirmation).toBeNull();
  expect(target?.previewJobId).toBe(preview.job_id);
  expect(Object.isFrozen(target)).toBe(true);
  const completed = transition(adopted, {
    type: "job",
    job: { ...latest, slot_held: false, revision: "3", snapshot },
  });
  expect(completed.preview?.job_id).toBe(latest.job_id);
});

test("authoritative resync preserves an active or uncertain exact-job binding", () => {
  const unrelated = {
    ...preview,
    job_id: "50209868-1236-4d7b-b65c-9b56d70dcfe7",
  };
  const active = initialState({
    ...context,
    latest: { ...preview, slot_held: true },
  });
  expect(
    transition(active, {
      type: "context",
      context: { ...context, latest: unrelated },
    }).job?.job_id,
  ).toBe(preview.job_id);
  const sending = transition(initialState(context), {
    type: "dispatch",
    intent: preview.intent,
  });
  const unknown = transition(sending, { type: "uncertain" });
  const resynced = transition(unknown, {
    type: "context",
    context: { ...context, latest: unrelated },
  });
  expect(resynced.dispatch.kind).toBe("unknown");
  expect(resynced.job).toBeNull();
  expect(transition(resynced, { type: "job", job: unrelated })).toBe(resynced);
});

test("authoritative context cannot adopt a latest job belonging to another server", () => {
  const confirmed = transition(initialState(context), { type: "confirm" });
  const next = transition(confirmed, {
    type: "context",
    context: {
      ...context,
      latest: {
        ...preview,
        job_id: "50209868-1236-4d7b-b65c-9b56d70dcfe7",
        intent: {
          ...preview.intent,
          server_id: "c68482e9-cdaa-4dfb-8fc8-18a4d935a307",
        },
      },
    },
  });
  expect(next.job).toEqual(confirmed.job);
  expect(next.preview).toEqual(confirmed.preview);
  expect(next.confirmation).toEqual(confirmed.confirmation);
});
