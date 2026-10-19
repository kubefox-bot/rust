import { screenStatus, TERMINAL_PHASES } from "../../jobs/display";
import type { Job } from "./types";

export const progressStatus = (job: Job | null): string => {
  if (job?.slot_held && job.snapshot && TERMINAL_PHASES[job.snapshot.phase])
    return job.snapshot.phase === "completed"
      ? "Состояние службы не подтверждено"
      : `${screenStatus(job)} — состояние службы не подтверждено`;
  return screenStatus(job);
};
