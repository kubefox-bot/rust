import type { BrowserContracts } from "../../api";
import {
  DEFAULT_PARAMETERS,
  JOURNAL_LIMIT,
  PHASE_LABELS,
  STAGE_LABELS,
  PUBLIC_ERROR_LABELS,
} from "./conts";
import type {
  CleanupState,
  CleanupEvent,
  Context,
  Job,
  JournalEntry,
  RunParameters,
} from "./types";
const eligible = (job: Job | null, version: string): job is Job =>
  Boolean(
    job &&
    !job.slot_held &&
    job.intent.operation === "preview" &&
    job.intent.profile_version === version &&
    job.snapshot?.phase === "completed" &&
    job.snapshot.summary.failed === "0",
  );
const parametersFromJob = (job: Job): RunParameters => ({
  scan: job.intent.scan,
  servicePolicy: job.intent.service_policy,
  retention: { kind: "before", local: job.intent.resolved_cutoff.local },
  limits: { ...job.intent.limits },
});
export const initialState = (context: Context): CleanupState => {
  const { latest, ...details } = context;
  return {
    context: details,
    parameters: latest ? parametersFromJob(latest) : DEFAULT_PARAMETERS,
    job: latest ?? null,
    preview: eligible(latest ?? null, context.profile_version)
      ? (latest ?? null)
      : null,
    previewCandidateId:
      latest?.intent.operation === "preview" ? latest.job_id : null,
    confirmation: null,
    dispatch: { kind: "settled" },
    journal: [],
    expanded: false,
  };
};
export const screenStatus = (job: Job | null): string => {
  if (!job) return "Ожидание запуска";
  if (!job.snapshot) return "Проверка доставки задания";
  return PHASE_LABELS[job.snapshot.phase];
};
export const progressPercent = (
  summary: BrowserContracts["summary"],
): number | null => {
  if (
    summary.total_scan_files === null ||
    summary.total_scan_files === undefined
  )
    return null;
  const total = BigInt(summary.total_scan_files);
  if (total === 0n) return 0;
  const percent = (BigInt(summary.scanned_files) * 100n) / total;
  return Number(percent > 100n ? 100n : percent);
};
const journalEntry = (job: Job): JournalEntry | null => {
  const snapshot = job.snapshot;
  if (!snapshot) return null;
  return {
    key: `${job.stream_epoch}:${job.job_id}:${job.revision}`,
    at: snapshot.observed_at,
    failed: snapshot.error !== null && snapshot.error !== undefined,
    text: `${PHASE_LABELS[snapshot.phase]}${snapshot.stage ? ` · ${STAGE_LABELS[snapshot.stage]}` : ""} · выбрано ${snapshot.summary.selected_files}, удалено ${snapshot.summary.removed_files}${snapshot.error ? ` · ${PUBLIC_ERROR_LABELS[snapshot.error.code]} (${snapshot.error.correlation_id})` : ""}`,
  };
};
const observe = (state: CleanupState, job: Job): CleanupState => {
  if (job.intent.server_id !== state.context.server.id) return state;
  const expected =
    state.dispatch.kind === "settled"
      ? state.job?.job_id
      : state.dispatch.intent.job_id;
  if (expected && job.job_id !== expected) return state;
  if (
    state.job?.job_id === job.job_id &&
    state.job.stream_epoch === job.stream_epoch &&
    BigInt(job.revision) < BigInt(state.job.revision)
  )
    return state;
  const entry = journalEntry(job);
  const journal =
    entry && !state.journal.some((value) => value.key === entry.key)
      ? [...state.journal, entry].slice(-JOURNAL_LIMIT)
      : state.journal;
  const preview =
    job.job_id === state.previewCandidateId &&
    eligible(job, state.context.profile_version)
      ? job
      : state.preview;
  return { ...state, job, preview, journal, dispatch: { kind: "settled" } };
};
export const transition = (
  state: CleanupState,
  event: CleanupEvent,
): CleanupState => {
  switch (event.type) {
    case "parameters":
      return {
        ...state,
        parameters: event.parameters,
        preview: null,
        previewCandidateId: null,
        confirmation: null,
      };
    case "expanded":
      return { ...state, expanded: event.expanded };
    case "dismiss":
      return { ...state, confirmation: null };
    case "dispatch":
      return {
        ...state,
        job: null,
        preview: null,
        dispatch: { kind: "sending", intent: event.intent },
        previewCandidateId:
          event.intent.operation === "preview" ? event.intent.job_id : null,
        confirmation: null,
      };
    case "uncertain":
      return state.dispatch.kind === "sending"
        ? { ...state, dispatch: { ...state.dispatch, kind: "unknown" } }
        : state;
    case "job":
      return observe(state, event.job);
    case "context": {
      if (event.context.server.id !== state.context.server.id)
        return initialState(event.context);
      const { latest, ...context } = event.context;
      const changed = context.profile_version !== state.context.profile_version;
      const next = {
        ...state,
        context,
        preview: changed ? null : state.preview,
        previewCandidateId: changed ? null : state.previewCandidateId,
        confirmation: changed ? null : state.confirmation,
      };
      return latest && (!state.job || latest.job_id === state.job.job_id)
        ? observe(next, latest)
        : next;
    }
    case "confirm": {
      const preview = state.preview;
      if (
        !eligible(preview, state.context.profile_version) ||
        state.context.busy ||
        state.job?.slot_held ||
        state.dispatch.kind !== "settled"
      )
        return state;
      return {
        ...state,
        confirmation: Object.freeze({
          serverId: state.context.server.id,
          serverName: state.context.server.name,
          profileVersion: state.context.profile_version,
          previewJobId: preview.job_id,
          resolvedCutoff: Object.freeze({ ...preview.intent.resolved_cutoff }),
          scan: preview.intent.scan,
          servicePolicy: preview.intent.service_policy,
          limits: Object.freeze({ ...preview.intent.limits }),
        }),
      };
    }
  }
};
