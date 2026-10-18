<script lang="ts">
  import type { BrowserContracts } from "../../api";
  import {
    formatBytes,
    formatCount,
    screenStatus,
    TERMINAL_PHASES,
  } from "../../jobs/display";
  import { OPERATION_LABELS, SCAN_LABELS } from "./conts";
  import { formatDateTime, formatServerDateTime } from "../../lib/time";

  let { jobs }: { jobs: ReadonlyArray<BrowserContracts["job"]> } = $props();
</script>

<div class="table-scroll" role="region" aria-label="Запуски очистки">
  <table>
    <caption class="sr-only">Сохранённые запуски выбранного сервера</caption>
    <thead>
      <tr>
        <th scope="col">Последнее изменение / задание</th>
        <th scope="col">Действие</th>
        <th scope="col">Выбрано</th>
        <th scope="col">Удалено</th>
        <th scope="col">Состояние</th>
      </tr>
    </thead>
    <tbody>
      {#each jobs as job (job.job_id)}
        <tr>
          <td>
            {#if job.snapshot}
              <time datetime={job.snapshot.observed_at}
                >{formatDateTime(job.snapshot.observed_at)}</time
              >
            {:else}<span class="muted">Снимок ещё не получен</span>{/if}
            <code>{job.job_id}</code>
          </td>
          <td>
            <strong>{OPERATION_LABELS[job.intent.operation]}</strong>
            <small>{SCAN_LABELS[job.intent.scan]}</small>
            <small
              >До {formatServerDateTime(job.intent.resolved_cutoff.local)} · время
              сервера</small
            >
          </td>
          <td>
            <strong>{formatCount(job.snapshot?.summary.selected_files)}</strong>
            <small>{formatBytes(job.snapshot?.summary.selected_bytes)}</small>
          </td>
          <td>
            <strong>{formatCount(job.snapshot?.summary.removed_files)}</strong>
            <small>{formatBytes(job.snapshot?.summary.removed_bytes)}</small>
          </td>
          <td>
            <strong>{screenStatus(job)}</strong>
            {#if job.slot_held && job.snapshot && TERMINAL_PHASES[job.snapshot.phase]}
              <small class="attention" role="status"
                >Состояние службы не подтверждено</small
              >
            {/if}
            <small>Ошибок: {formatCount(job.snapshot?.summary.failed)}</small>
            <small
              >Осталось старых: {formatCount(
                job.snapshot?.summary.remaining_old_files,
              )}</small
            >
            {#if job.snapshot?.error}<small class="failed"
                >Код обращения: {job.snapshot.error.correlation_id}</small
              >{/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .table-scroll {
    max-width: 100%;
    overflow-x: auto;
  }
  table {
    min-width: 790px;
  }
  td {
    vertical-align: top;
  }
  strong,
  small,
  code,
  time {
    display: block;
  }
  strong {
    font-size: 12px;
    font-weight: 550;
  }
  small,
  time {
    font-size: 10px;
    line-height: 1.6;
  }
  small,
  code {
    color: var(--muted);
    margin-top: 5px;
  }
  code {
    font-size: 9px;
    max-width: 240px;
  }
  .failed {
    color: var(--danger);
  }
  .attention {
    color: var(--ink);
    font-weight: 550;
  }
</style>
