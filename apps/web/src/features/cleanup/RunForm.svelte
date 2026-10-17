<script lang="ts">
  import { DEFAULT_RETENTION_DAYS, MAX_BATCH_SIZE, MAX_WORKERS } from "./conts";
  import type { RunParameters } from "./types";
  let {
    parameters,
    disabled,
    canClean,
    cutoff,
    onparameters,
    onpreview,
    onclean,
  }: {
    parameters: RunParameters;
    disabled: boolean;
    canClean: boolean;
    cutoff: string | null;
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
    <fieldset {disabled}>
      <div class="field-grid">
        <label class="field"
          >Режим сканирования<select
            aria-label="Режим сканирования"
            aria-describedby="scan-hint"
            value={parameters.scan}
            onchange={(event) =>
              onparameters({
                ...parameters,
                scan: event.currentTarget.value === "full" ? "full" : "fast",
              })}
            ><option value="fast">По базе — быстро</option><option value="full"
              >Полная сверка файлов</option
            ></select
          ><small id="scan-hint"
            >Полная сверка требует остановленной почтовой службы.</small
          ></label
        >
        <div class="field">
          <label for="retention-kind">Период хранения</label>
          <div class="retention">
            <select
              id="retention-kind"
              aria-label="Период хранения"
              value={parameters.retention.kind}
              onchange={(event) => retentionKind(event.currentTarget.value)}
              ><option value="days">Дней</option><option value="months"
                >Месяцев</option
              ><option value="years">Лет</option><option value="before"
                >До даты</option
              ></select
            >{#if parameters.retention.kind === "before"}<input
                aria-label="Граница по времени сервера"
                type="datetime-local"
                step="1"
                required
                value={parameters.retention.local}
                onchange={(event) =>
                  onparameters({
                    ...parameters,
                    retention: {
                      kind: "before",
                      local: event.currentTarget.value,
                    },
                  })}
              />{:else}<input
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
          <small>Время границы рассчитывается на почтовом сервере.</small>
        </div>
      </div>
      <label class="manage"
        ><input
          type="checkbox"
          checked={parameters.servicePolicy === "manage"}
          onchange={(event) =>
            onparameters({
              ...parameters,
              servicePolicy: event.currentTarget.checked
                ? "manage"
                : "require_stopped",
            })}
        /><span
          >Останавливать службу на время очистки<small
            >После выполнения агент восстановит исходное состояние службы.</small
          ></span
        ></label
      >
      <details>
        <summary>Дополнительные параметры</summary>
        <div class="field-grid advanced">
          <label class="field"
            >Размер пакета<input
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
            >Параллельные операции<input
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
    {#if cutoff}<div class="cutoff">
        Граница последнего предпросмотра: <strong>{cutoff}</strong> · время сервера
      </div>{/if}
    <div class="form-bottom">
      <span class="muted">Сначала проверьте объём в предпросмотре.</span>
      <div class="actions">
        <button class="button" type="submit" {disabled}>Предпросмотр</button
        ><button
          class="button primary"
          type="button"
          disabled={disabled || !canClean}
          onclick={onclean}>Очистить</button
        >
      </div>
    </div>
  </form>
</section>

<style>
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
    margin-bottom: 22px;
  }
  .card-heading h2 {
    margin: 0;
  }
  .retention {
    display: flex;
    gap: 8px;
  }
  .retention select {
    width: 43%;
  }
  .retention input {
    flex: 1;
    width: 57%;
    min-width: 0;
  }
  .manage {
    display: flex;
    align-items: start;
    gap: 10px;
    margin-top: 22px;
    font-size: 12px;
    line-height: 1.5;
  }
  .manage input {
    accent-color: var(--accent);
    margin-top: 3px;
  }
  .manage small {
    display: block;
    color: var(--muted);
    font-size: 10px;
    margin-top: 4px;
  }
  .form-bottom {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: center;
    margin-top: 24px;
    border-top: 1px solid var(--line);
    padding-top: 20px;
  }
  .form-bottom > .muted {
    font-size: 11px;
  }
  .form-bottom .actions {
    margin: 0;
  }
  .cutoff {
    font-size: 11px;
    color: var(--muted);
    margin-top: 18px;
    line-height: 1.7;
  }
  details {
    margin-top: 18px;
    font-size: 10px;
    color: var(--muted);
  }
  summary {
    cursor: pointer;
    display: inline-flex;
    gap: 8px;
  }
  .advanced {
    margin-top: 16px;
  }
  @media (max-width: 600px) {
    .form-bottom {
      flex-wrap: wrap;
    }
    .form-bottom .actions {
      width: 100%;
      display: grid;
      grid-template-columns: 1fr 1fr;
    }
    .retention {
      flex-wrap: wrap;
    }
    .retention select,
    .retention input {
      width: 100%;
    }
  }
</style>
