<script lang="ts">
  import { Button } from "../../ui/button";
  import { Input } from "../../ui/input";
  import { Select } from "../../ui/select";
  import { Checkbox } from "../../ui/checkbox";
  import {
    DEFAULT_RETENTION_DAYS,
    MAX_BATCH_SIZE,
    MAX_WORKERS,
    SCAN_OPTIONS,
    RETENTION_OPTIONS,
  } from "./conts";
  import { displayCutoff, editedCutoff } from "./cutoff-input";
  import { formatServerDateTime } from "../../lib/time";
  import type { Job, RunParameters } from "./types";
  import PreviewResult from "./PreviewResult.svelte";
  let {
    parameters,
    disabled,
    preview,
    onparameters,
    onpreview,
    onclean,
  }: {
    parameters: RunParameters;
    disabled: boolean;
    preview: Job | null;
    onparameters: (value: RunParameters) => void;
    onpreview: () => void;
    onclean: () => void;
  } = $props();
  const retentionKind = (kind: string) => {
    switch (kind) {
      case "before":
        onparameters({
          ...parameters,
          retention: { kind: "before", local: "" },
        });
        break;
      case "days":
      case "months":
      case "years":
        onparameters({
          ...parameters,
          retention: {
            kind,
            count:
              parameters.retention.kind === "before"
                ? DEFAULT_RETENTION_DAYS
                : parameters.retention.count,
          },
        });
        break;
    }
  };
</script>

<section class="card run-config">
  <div class="card-heading">
    <h2>Параметры очистки</h2>
    <span class="badge">Ручной запуск</span>
  </div>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      onpreview();
    }}
  >
    {#if preview}<PreviewResult {preview} />{/if}
    <fieldset {disabled}>
      <div class="field-grid run-fields">
        <div class="field">
          <Select
            label="Режим сканирования"
            aria-describedby="scan-hint"
            options={SCAN_OPTIONS}
            value={parameters.scan}
            {disabled}
            onchange={(scan) =>
              onparameters({
                ...parameters,
                scan,
              })}
          /><small id="scan-hint"
            >Полная сверка требует остановленной почтовой службы.</small
          >
        </div>
        <div class="field">
          <label for="retention-kind">Период хранения</label>
          <div
            class="retention"
            class:exact-date={parameters.retention.kind === "before"}
          >
            <Select
              id="retention-kind"
              aria-label="Период хранения"
              options={RETENTION_OPTIONS}
              value={parameters.retention.kind}
              {disabled}
              onchange={retentionKind}
            />{#if parameters.retention.kind === "before"}<Input
                aria-label="Граница по времени сервера"
                aria-describedby="cutoff-hint"
                type="datetime-local"
                step="any"
                required
                value={displayCutoff(parameters.retention.local)}
                onchange={(event) =>
                  onparameters({
                    ...parameters,
                    retention: {
                      kind: "before",
                      local: editedCutoff(event.currentTarget.value),
                    },
                  })}
              />{:else}<Input
                aria-label="Срок хранения"
                type="number"
                min="1"
                step="1"
                required
                value={parameters.retention.count}
                onchange={(event) => {
                  if (parameters.retention.kind !== "before")
                    onparameters({
                      ...parameters,
                      retention: {
                        kind: parameters.retention.kind,
                        count: Number(event.currentTarget.value),
                      },
                    });
                }}
              />{/if}
          </div>
          {#if parameters.retention.kind === "before" && parameters.retention.local}
            <small
              >Граница: <time datetime={parameters.retention.local}
                >{formatServerDateTime(parameters.retention.local)}</time
              ></small
            >
          {/if}
          <small id="cutoff-hint"
            >Время границы рассчитывается на почтовом сервере.</small
          >
        </div>
      </div>
      <details>
        <summary>
          <svg
            class="chevron"
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            aria-hidden="true"
          >
            <path
              d="m9 6 6 6-6 6"
              stroke="currentColor"
              stroke-width="1.8"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
          Дополнительные параметры
        </summary>
        <div class="service-policy">
          <Checkbox
            label="Останавливать службу на время очистки"
            description="После выполнения агент восстановит исходное состояние службы."
            checked={parameters.servicePolicy === "manage"}
            onchange={(event) =>
              onparameters({
                ...parameters,
                servicePolicy: event.currentTarget.checked
                  ? "manage"
                  : "require_stopped",
              })}
          />
        </div>
        <div class="field-grid advanced">
          <label class="field"
            >Размер пакета<Input
              type="number"
              min="1"
              max={MAX_BATCH_SIZE}
              step="1"
              value={parameters.limits.batch_size}
              onchange={(event) =>
                onparameters({
                  ...parameters,
                  limits: {
                    ...parameters.limits,
                    batch_size: Number(event.currentTarget.value),
                  },
                })}
            /></label
          ><label class="field"
            >Параллельные операции<Input
              type="number"
              min="1"
              max={MAX_WORKERS}
              step="1"
              value={parameters.limits.workers}
              onchange={(event) =>
                onparameters({
                  ...parameters,
                  limits: {
                    ...parameters.limits,
                    workers: Number(event.currentTarget.value),
                  },
                })}
            /></label
          >
        </div>
      </details>
    </fieldset>
    <div class="form-bottom">
      {#if !preview}
        <span class="muted">Предпросмотр покажет объём почты без удаления.</span
        >
      {/if}
      <div class="actions">
        <Button
          type="submit"
          variant={preview ? "secondary" : "primary"}
          {disabled}>Предпросмотр</Button
        ><Button
          variant="danger"
          type="button"
          disabled={disabled || !preview}
          onclick={onclean}>Очистить</Button
        >
      </div>
    </div>
  </form>
</section>

<style>
  .run-config {
    padding: 18px;
  }
  form > :global(.preview-result) {
    margin-bottom: 14px;
  }
  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  .card-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 14px;
  }
  .card-heading h2 {
    margin: 0;
  }
  .retention {
    display: grid;
    grid-template-columns: minmax(0, 43fr) minmax(0, 57fr);
    gap: 8px;
  }
  .retention.exact-date {
    grid-template-columns: minmax(0, 1fr);
  }
  .run-fields {
    grid-template-columns: minmax(0, 1fr);
    gap: 16px;
  }
  .service-policy {
    margin-top: 12px;
  }
  .form-bottom {
    display: grid;
    gap: 12px;
    margin-top: 14px;
    border-top: 1px solid var(--line);
    padding-top: 14px;
  }
  .form-bottom > .muted {
    font-size: 11px;
  }
  .form-bottom .actions {
    margin: 0;
    justify-content: flex-end;
  }
  details {
    margin-top: 12px;
    font-size: 10px;
    color: var(--muted);
  }
  summary {
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 0;
    color: var(--ink);
    font-size: 11px;
    font-weight: 550;
    list-style: none;
  }
  summary::-webkit-details-marker {
    display: none;
  }
  summary:focus-visible {
    outline: 2px solid var(--focus);
    outline-offset: 3px;
    border-radius: 4px;
  }
  .chevron {
    flex-shrink: 0;
    transition: transform 150ms ease;
  }
  details[open] .chevron {
    transform: rotate(90deg);
  }
  .advanced {
    margin-top: 12px;
  }
  @media (prefers-reduced-motion: reduce) {
    .chevron {
      transition: none;
    }
  }
  @media (max-width: 600px) {
    .form-bottom .actions {
      width: 100%;
      display: grid;
      grid-template-columns: 1fr 1fr;
    }
    .retention {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
