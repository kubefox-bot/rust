<script lang="ts">
  import type { Dispatch, Job } from "./types";
  import { screenStatus, progressPercent } from "./model";
  import { formatBytes, formatCount } from "./format";
  import { MAIL_LABELS, STAGE_LABELS } from "./conts";
  let {
    job,
    dispatch,
    busy,
    oncancel,
    onreconcile,
  }: {
    job: Job | null;
    dispatch: Dispatch;
    busy: boolean;
    oncancel: () => void;
    onreconcile: () => void;
  } = $props();
  const snapshot = $derived(job?.snapshot);
  const percent = $derived(snapshot ? progressPercent(snapshot.summary) : null);
  const active = $derived(job?.slot_held || dispatch.kind !== "settled");
  const canCancel = $derived(
    job?.slot_held &&
      snapshot?.phase !== "restoring" &&
      snapshot?.phase !== "cancelling",
  );
</script>

<section class="card progress-panel" aria-label="Состояние задания">
  <div class="progress-heading">
    <div>
      <span class="eyebrow">ТЕКУЩЕЕ ЗАДАНИЕ</span>
      <h2 aria-live="polite">
        {dispatch.kind === "sending"
          ? "Отправка задания…"
          : dispatch.kind === "unknown"
            ? "Доставка неизвестна — требуется проверка"
            : screenStatus(job)}
      </h2>
    </div>
    <span class="badge" class:active
      >{active ? "В работе" : job ? "Завершено" : "Нет задания"}</span
    >
  </div>
  {#if snapshot?.stage}<p class="stage">
      {STAGE_LABELS[snapshot.stage]}
    </p>{/if}{#if !active && !job}<div
      class="idle-line"
      aria-label="Ожидание запуска"
    ></div>{:else if snapshot && !active && percent === null}<div
      class="completed-line"
      aria-label="Выполнение завершено"
    ></div>{:else}<progress
      aria-label="Прогресс текущего этапа"
      max="100"
      value={percent ?? undefined}
    ></progress>{/if}
  <div class="progress-meta">
    <span
      >{percent === null
        ? "Общий объём не определён"
        : `${percent}% сканирования`}</span
    ><span
      >{job
        ? `Служба: ${MAIL_LABELS[snapshot?.mail_state ?? "unknown"]}`
        : "Ожидание команды"}</span
    >
  </div>
  <div class="metrics">
    <div>
      <span>Выбрано</span><strong
        >{formatCount(snapshot?.summary.selected_files ?? "0")}</strong
      ><small>{formatBytes(snapshot?.summary.selected_bytes ?? "0")}</small>
    </div>
    <div>
      <span>Удалено файлов</span><strong
        >{formatCount(snapshot?.summary.removed_files ?? "0")}</strong
      ><small>{formatBytes(snapshot?.summary.removed_bytes ?? "0")}</small>
    </div>
    <div>
      <span>Ошибок</span><strong
        class:failed={snapshot?.summary.failed !== "0" &&
          snapshot?.summary.failed !== undefined}
        >{formatCount(snapshot?.summary.failed ?? "0")}</strong
      ><small
        >Осталось старых: {formatCount(
          snapshot?.summary.remaining_old_files,
        )}</small
      >
    </div>
  </div>
  {#if job || dispatch.kind === "unknown"}<div class="progress-bottom">
      <code title="Идентификатор задания"
        >{job?.job_id ??
          (dispatch.kind === "unknown" ? dispatch.intent.job_id : "")}</code
      >
      <div class="actions">
        <button
          class="button quiet"
          type="button"
          onclick={onreconcile}
          disabled={busy}>Проверить состояние</button
        >{#if canCancel}<button
            class="button"
            type="button"
            onclick={oncancel}
            disabled={busy}>Отменить задание</button
          >{/if}
      </div>
    </div>{/if}{#if job && !snapshot}<div class="notice">
      Центр сохраняет исходный идентификатор. Повторная очистка не отправляется
      автоматически.
    </div>{/if}{#if snapshot && snapshot.phase === "interrupted"}<div
      class="notice"
    >
      Проверьте состояние службы и агента. Прерванное задание автоматически не
      повторяется.
    </div>{/if}
</section>

<style>
  .idle-line {
    height: 5px;
    border-radius: 9px;
    background: #eef1f6;
    margin: 22px 0 10px;
  }
  .eyebrow {
    font-size: 9px;
    letter-spacing: 1.2px;
    color: var(--muted);
    font-weight: 600;
  }
  .progress-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .progress-heading h2 {
    font-size: 16px;
    margin: 8px 0 0;
  }
  .badge.active {
    background: var(--accent-soft);
    color: var(--accent);
    border-color: #dce5ff;
  }
  .stage {
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 0;
  }
  progress {
    width: 100%;
    height: 5px;
    border: 0;
    display: block;
    margin: 22px 0 10px;
    border-radius: 9px;
    accent-color: var(--accent);
    overflow: hidden;
  }
  progress::-webkit-progress-bar {
    background: #eef1f6;
  }
  progress::-webkit-progress-value {
    background: var(--accent);
    border-radius: 9px;
  }
  .completed-line {
    height: 5px;
    border-radius: 9px;
    background: var(--success);
    margin: 22px 0 10px;
  }
  .progress-meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 10px;
    color: var(--muted);
  }
  .metrics {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin-top: 24px;
    padding-top: 20px;
    border-top: 1px solid var(--line);
  }
  .metrics > div {
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 0 20px;
    border-left: 1px solid var(--line);
  }
  .metrics > div:first-child {
    padding-left: 0;
    border: 0;
  }
  .metrics span,
  .metrics small {
    color: var(--muted);
    font-size: 10px;
    line-height: 1.5;
  }
  .metrics strong {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.6px;
  }
  .metrics strong.failed {
    color: var(--danger);
  }
  .progress-bottom {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-top: 1px solid var(--line);
    margin-top: 20px;
    padding-top: 16px;
  }
  .progress-bottom code {
    font-size: 9px;
    color: var(--muted);
  }
  .progress-bottom .actions {
    margin: 0;
  }
  .progress-bottom button {
    font-size: 10px;
    padding: 8px 10px;
  }
  @media (max-width: 600px) {
    .progress-bottom {
      flex-wrap: wrap;
    }
    .progress-meta {
      flex-wrap: wrap;
    }
    .metrics > div {
      padding: 0 10px;
    }
    .metrics strong {
      font-size: 21px;
    }
    .progress-heading h2 {
      font-size: 14px;
    }
    .progress-heading {
      align-items: start;
    }
  }
</style>
