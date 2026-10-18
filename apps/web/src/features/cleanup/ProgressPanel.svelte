<script lang="ts">
  import { Button } from "../../ui/button";
  import type { Dispatch, Job } from "./types";
  import { progressPercent } from "./model";
  import { progressStatus } from "./progress-status";
  import {
    formatBytes,
    formatCount,
    MAIL_LABELS,
    STAGE_LABELS,
    TERMINAL_PHASES,
  } from "../../jobs/display";
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
  const completed = $derived(!active && snapshot?.phase === "completed");
  const checkingService = $derived(
    Boolean(job?.slot_held && snapshot && TERMINAL_PHASES[snapshot.phase]),
  );
  const failed = $derived(
    snapshot?.phase === "failed" ||
      snapshot?.phase === "partial" ||
      snapshot?.phase === "interrupted",
  );
  const attention = $derived(
    !failed && (checkingService || dispatch.kind === "unknown"),
  );
  const operation = $derived(
    dispatch.kind === "settled"
      ? job?.intent.operation
      : dispatch.intent.operation,
  );
  const canCancel = $derived(
    job?.slot_held &&
      snapshot?.phase !== "restoring" &&
      snapshot?.phase !== "cancelling",
  );
</script>

<section class="card progress-panel" aria-label="Состояние задания">
  <div class="progress-heading">
    <div>
      <span class="eyebrow">
        {#if operation === "preview"}ПРЕДПРОСМОТР{:else if operation === "clean"}ОЧИСТКА{:else}ТЕКУЩЕЕ
          ЗАДАНИЕ{/if}
      </span>
      <h2 aria-live="polite" class:failed class:attention>
        <span
          class="status-dot"
          class:active
          class:completed
          class:failed
          class:attention
          aria-hidden="true"
        ></span>
        {dispatch.kind === "sending"
          ? "Отправка задания…"
          : dispatch.kind === "unknown"
            ? "Доставка неизвестна — требуется проверка"
            : progressStatus(job)}
      </h2>
    </div>
  </div>
  {#if active && !checkingService && snapshot?.stage && snapshot.stage !== "complete" && snapshot.stage !== "cancelled"}<p
      class="stage"
    >
      {STAGE_LABELS[snapshot.stage]}
    </p>{/if}{#if !active && !job}<div
      class="idle-line"
      aria-hidden="true"
    ></div>{:else if !active}<div
      class="settled-line"
      class:completed
      class:failed
      aria-hidden="true"
    ></div>{:else}<progress
      aria-label="Прогресс текущего этапа"
      max="100"
      value={percent ?? undefined}
    ></progress>{/if}
  <div class="progress-meta">
    {#if active && !checkingService}<span
        >{percent === null
          ? "Общий объём пока неизвестен"
          : `${percent}% сканирования`}</span
      >{/if}<span class="mail-state"
      >{job
        ? `Служба: ${MAIL_LABELS[snapshot?.mail_state ?? "unknown"]}`
        : "Ожидание команды"}</span
    >
  </div>
  {#if snapshot}
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
  {/if}
  {#if job || dispatch.kind === "unknown"}<div class="progress-bottom">
      <code title="Идентификатор задания"
        >{job?.job_id ??
          (dispatch.kind === "unknown" ? dispatch.intent.job_id : "")}</code
      >
      <div class="actions">
        <Button
          size="small"
          variant="quiet"
          type="button"
          onclick={onreconcile}
          disabled={busy}>Проверить состояние</Button
        >{#if canCancel}<Button
            size="small"
            type="button"
            onclick={oncancel}
            disabled={busy}>Отменить задание</Button
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
  .progress-panel {
    padding: 18px;
  }
  .idle-line {
    height: 5px;
    border-radius: 9px;
    background: #eef1f6;
    margin: 14px 0 8px;
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
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 16px;
    margin: 6px 0 0;
  }
  .status-dot {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--control-line);
  }
  .status-dot.active {
    background: var(--accent);
  }
  .status-dot.completed {
    background: var(--success);
  }
  .status-dot.failed {
    background: var(--danger);
  }
  h2.failed {
    color: var(--danger);
  }
  .status-dot.attention {
    background: #94621a;
  }
  h2.attention {
    color: #94621a;
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
    margin: 14px 0 8px;
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
  .settled-line {
    height: 5px;
    border-radius: 9px;
    background: var(--line);
    margin: 14px 0 8px;
  }
  .settled-line.completed {
    background: var(--success);
  }
  .settled-line.failed {
    background: var(--danger);
  }
  .progress-meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: 10px;
    color: var(--muted);
  }
  .mail-state {
    margin-left: auto;
  }
  .metrics {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px solid var(--line);
  }
  .metrics > div {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 14px;
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
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.6px;
  }
  .metrics strong.failed {
    color: var(--danger);
  }
  .progress-bottom {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    border-top: 1px solid var(--line);
    margin-top: 14px;
    padding-top: 12px;
  }
  .progress-bottom code {
    font-size: 9px;
    color: var(--muted);
  }
  .progress-bottom .actions {
    margin: 0;
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
