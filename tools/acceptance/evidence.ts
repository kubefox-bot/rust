import { MAX_JOURNAL_ROWS, MAX_QUEUED_EVENTS } from "./conts";
type Evidence = Readonly<{
  confirmedServerId: string;
  cleanedServerIds: readonly string[];
  executions: ReadonlyArray<Readonly<{ jobId: string; count: number }>>;
  maxJournalRows: number;
  maxQueuedEvents: number;
  automaticCleanRequests: number;
}>;
export const assertExecutionEvidence = (evidence: Evidence): void => {
  if (
    !evidence.confirmedServerId ||
    evidence.cleanedServerIds.some((id) => id !== evidence.confirmedServerId)
  )
    throw new Error("Cleanup affected an unconfirmed target");
  const seen = new Set<string>();
  for (const execution of evidence.executions) {
    if (!execution.jobId || execution.count !== 1 || seen.has(execution.jobId))
      throw new Error("Duplicate or invalid job execution");
    seen.add(execution.jobId);
  }
  for (const [observed, limit] of [
    [evidence.maxJournalRows, MAX_JOURNAL_ROWS],
    [evidence.maxQueuedEvents, MAX_QUEUED_EVENTS],
  ]) {
    if (!Number.isSafeInteger(observed) || observed < 0 || observed > limit)
      throw new Error("A bounded buffer exceeded its limit");
  }
  if (evidence.automaticCleanRequests !== 0)
    throw new Error("Automatic cleanup is forbidden in the MVP");
};
