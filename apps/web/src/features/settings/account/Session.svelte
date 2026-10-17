<script lang="ts">
  import { onMount } from "svelte";
  import { sessionSettings, saveSessionSettings } from "./api";
  import { IDLE_INTERVALS } from "./conts";
  import { failureMessage } from "./failure-message";
  import type { SessionSettingsState } from "./types";

  let sessionState = $state<SessionSettingsState>({ kind: "loading" });
  let active = true;
  const load = async (signal: AbortSignal) => {
    sessionState = { kind: "loading" };
    try {
      const result = await sessionSettings(signal);
      if (!active || signal.aborted) return;
      if (
        !IDLE_INTERVALS.some(
          (interval) => interval.minutes === result.idle_minutes,
        )
      ) {
        sessionState = {
          kind: "failed",
          message: "Центр вернул неподдерживаемый таймаут сессии.",
        };
        return;
      }
      sessionState = {
        kind: "ready",
        minutes: result.idle_minutes,
        saving: false,
        saved: false,
        error: "",
      };
    } catch (failure) {
      if (active && !signal.aborted) {
        sessionState = {
          kind: "failed",
          message: failureMessage(
            failure,
            "Не удалось загрузить таймаут сессии.",
          ),
        };
      }
    }
  };
  onMount(() => {
    const controller = new AbortController();
    load(controller.signal);
    return () => {
      active = false;
      controller.abort();
    };
  });
  const select = (event: Event & { currentTarget: HTMLSelectElement }) => {
    if (sessionState.kind !== "ready" || sessionState.saving) return;
    sessionState = {
      ...sessionState,
      minutes: Number(event.currentTarget.value),
      saved: false,
      error: "",
    };
  };
  const save = async (event: SubmitEvent) => {
    event.preventDefault();
    if (sessionState.kind !== "ready" || sessionState.saving) return;
    const selected = sessionState.minutes;
    sessionState = { ...sessionState, saving: true, saved: false, error: "" };
    try {
      const result = await saveSessionSettings({ idle_minutes: selected });
      if (!active) return;
      sessionState = {
        kind: "ready",
        minutes: result.idle_minutes,
        saving: false,
        saved: true,
        error: "",
      };
    } catch (failure) {
      if (!active) return;
      sessionState = {
        kind: "ready",
        minutes: selected,
        saving: false,
        saved: false,
        error: failureMessage(
          failure,
          "Не удалось сохранить таймаут. Проверьте состояние перед повтором.",
        ),
      };
    }
  };
</script>

<section class="card" aria-labelledby="session-settings-heading">
  <h2 id="session-settings-heading">Сессия</h2>
  <p class="subtitle">
    Центр завершит сеанс после выбранного периода бездействия.
  </p>
  {#if sessionState.kind === "loading"}
    <p class="muted" role="status">Загрузка таймаута…</p>
  {:else if sessionState.kind === "failed"}
    <div class="error" role="alert">{sessionState.message}</div>
  {:else}
    <form onsubmit={save}>
      <label class="field" for="account-idle">Выход при бездействии</label>
      <select
        id="account-idle"
        value={sessionState.minutes}
        onchange={select}
        disabled={sessionState.saving}
      >
        {#each IDLE_INTERVALS as interval}
          <option value={interval.minutes}>{interval.label}</option>
        {/each}
      </select>
      {#if sessionState.error}<div class="error" role="alert">
          {sessionState.error}
        </div>{/if}
      {#if sessionState.saved}<p class="saved" role="status">
          Таймаут сохранён
        </p>{/if}
      <div class="actions">
        <button class="button" type="submit" disabled={sessionState.saving}
          >{sessionState.saving ? "Сохранение…" : "Сохранить таймаут"}</button
        >
      </div>
    </form>
  {/if}
</section>

<style>
  form {
    margin-top: 22px;
  }
  select {
    width: 100%;
    max-width: 280px;
    margin-top: 8px;
    border: 1px solid #dce1ea;
    border-radius: 8px;
    background: #fff;
    color: var(--ink);
    padding: 10px 12px;
    font-size: 12px;
  }
  .saved {
    color: var(--success);
    font-size: 12px;
  }
</style>
