<script lang="ts">
  import { onMount } from "svelte";
  import { readPolicy, savePolicy } from "./api";
  import { failureMessage } from "../account/failure-message";
  import type { PolicyState } from "./types";
  let state = $state<PolicyState>({ kind: "loading" });
  const scope = new AbortController();
  const load = async () => {
    state = { kind: "loading" };
    try {
      const policy = await readPolicy(scope.signal);
      if (!scope.signal.aborted)
        state = {
          kind: "ready",
          policy,
          saving: false,
          saved: false,
          error: "",
        };
    } catch (error) {
      if (!scope.signal.aborted)
        state = {
          kind: "failed",
          message: failureMessage(
            error,
            "Не удалось загрузить настройки обновлений.",
          ),
        };
    }
  };
  onMount(() => {
    load();
    return () => scope.abort();
  });
  const edit = (
    field: "check_interval_seconds" | "graceful_timeout_minutes",
    value: number,
  ) => {
    if (state.kind !== "ready" || state.saving) return;
    state = {
      ...state,
      policy: { ...state.policy, [field]: value },
      saved: false,
      error: "",
    };
  };
  const save = async (event: SubmitEvent) => {
    event.preventDefault();
    if (state.kind !== "ready" || state.saving) return;
    const policy = state.policy;
    state = { ...state, saving: true, saved: false, error: "" };
    try {
      const saved = await savePolicy(policy, scope.signal);
      if (!scope.signal.aborted)
        state = {
          kind: "ready",
          policy: saved,
          saving: false,
          saved: true,
          error: "",
        };
    } catch (error) {
      if (!scope.signal.aborted)
        state = {
          kind: "ready",
          policy,
          saving: false,
          saved: false,
          error: failureMessage(
            error,
            "Не удалось сохранить настройки. Проверьте состояние перед повтором.",
          ),
        };
    }
  };
</script>

<section class="card" aria-labelledby="update-settings-heading">
  <h2 id="update-settings-heading">Обновления центра</h2>
  <p class="subtitle">
    Новые версии устанавливаются автоматически. Перед перезапуском центр ожидает
    завершения текущей работы.
  </p>
  {#if state.kind === "loading"}
    <p class="muted" role="status">Загрузка настроек обновлений…</p>
  {:else if state.kind === "failed"}
    <div class="error" role="alert">{state.message}</div>
    <button class="button" onclick={load}>Повторить загрузку</button>
  {:else}
    <form onsubmit={save}>
      <label class="field" for="update-check-interval"
        >Проверять обновления каждые, секунд</label
      >
      <input
        id="update-check-interval"
        type="number"
        min="30"
        max="86400"
        step="1"
        required
        value={state.policy.check_interval_seconds}
        disabled={state.saving}
        oninput={(event) =>
          edit("check_interval_seconds", event.currentTarget.valueAsNumber)}
      />
      <p class="muted">
        От 30 секунд до 24 часов. Изменение применяется при следующем обращении
        службы обновления к центру.
      </p>
      <label class="field" for="update-graceful-timeout"
        >Ожидать завершения работы, минут</label
      >
      <input
        id="update-graceful-timeout"
        type="number"
        min="1"
        max="1440"
        step="1"
        required
        value={state.policy.graceful_timeout_minutes}
        disabled={state.saving}
        oninput={(event) =>
          edit("graceful_timeout_minutes", event.currentTarget.valueAsNumber)}
      />
      <p class="muted">
        От 1 минуты до 24 часов. Если работа не завершена, установка
        откладывается без принудительной остановки очистки.
      </p>
      {#if state.error}<div class="error" role="alert">{state.error}</div>{/if}
      {#if state.saved}<p class="saved" role="status">
          Настройки обновлений сохранены
        </p>{/if}
      <div class="actions">
        <button class="button" type="submit" disabled={state.saving}
          >{state.saving
            ? "Сохранение…"
            : "Сохранить настройки обновлений"}</button
        >
      </div>
    </form>
  {/if}
</section>

<style>
  form {
    margin-top: 22px;
  }
  input {
    display: block;
    width: 100%;
    max-width: 280px;
    margin-top: 8px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: #fff;
    color: var(--ink);
    padding: 10px 12px;
    font-size: 12px;
  }
  form .muted {
    max-width: 640px;
    margin-bottom: 22px;
    font-size: 12px;
  }
  .saved {
    color: var(--success);
    font-size: 12px;
  }
</style>
