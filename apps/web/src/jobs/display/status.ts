import type { BrowserContracts } from "../../api";
import { PHASE_LABELS } from "./conts";
export const screenStatus = (job: BrowserContracts["job"] | null): string => {
  if (!job) return "Ожидание запуска";
  if (!job.snapshot) return "Проверка доставки задания";
  return PHASE_LABELS[job.snapshot.phase];
};
