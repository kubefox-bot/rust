<script lang="ts">
  import { Button } from "../../../ui/button";
  import { onMount } from "svelte";
  import { read, reconcile, verifiedOperation, message } from "./api";
  import { ENROLLMENT_PHASES, OPERATION_REFRESH_MS } from "./conts";
  import type { EnrollmentAttempt, Enrollment } from "./types";
  let {
    attempt,
    initial,
    configured = false,
    onrefresh,
    onlock,
  }: {
    attempt: EnrollmentAttempt;
    initial: Enrollment | null;
    configured?: boolean;
    onrefresh: () => void;
    onlock: (locked: boolean) => void;
  } = $props();
  let view = $state<Enrollment | null>(null);
  let pending = $state(false);
  let error = $state("");
  const controller = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined = undefined;
  const active = $derived(
    view !== null &&
      view.slot_held &&
      view.phase !== "unknown" &&
      view.phase !== "failed" &&
      view.phase !== "ready",
  );
  const accept = (next: Enrollment) => {
    view = verifiedOperation({ attempt, view: next });
    const settled =
      !view.slot_held && (view.phase === "ready" || view.phase === "failed");
    onlock(!settled);
    if (settled) onrefresh();
  };
  const refresh = async (operation: "read" | "reconcile") => {
    if (pending || controller.signal.aborted) return;
    clearTimeout(timer);
    pending = true;
    error = "";
    try {
      const next = await (operation === "read" ? read : reconcile)({
        operationId: attempt.operationId,
        signal: controller.signal,
      });
      if (!controller.signal.aborted) accept(next);
    } catch (failure) {
      if (!controller.signal.aborted) error = message(failure);
    } finally {
      pending = false;
      if (!controller.signal.aborted && active && !error)
        timer = setTimeout(() => {
          refresh("read");
        }, OPERATION_REFRESH_MS);
    }
  };
  onMount(() => {
    if (initial) accept(initial);
    else onlock(true);
    if (active)
      timer = setTimeout(() => {
        refresh("read");
      }, OPERATION_REFRESH_MS);
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  });
</script>

<section aria-label="Состояние подключения" aria-live="polite">
  <h3>
    {view ? ENROLLMENT_PHASES[view.phase] : "Результат установки неизвестен"}
  </h3>
  <p>Целевой сервер: <strong>{attempt.name}</strong></p>
  <p class="muted">Операция <code>{attempt.operationId}</code></p>
  {#if !view || view.phase === "unknown" || error}
    <div class="notice">
      Запрос мог быть принят. Проверьте и согласуйте эту операцию по её
      идентификатору перед новым действием.
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if view?.phase === "ready"}
    <p class="badge">
      {configured ? "Профиль настроен" : "Требуется настройка"}
    </p>
    {#if !configured}<p class="muted">
        Откройте почтовый профиль этого сервера и проверьте его настройки перед
        очисткой.
      </p>{/if}
  {:else}
    <p class="muted">Пакет {attempt.packageVersion}</p>
    <div class="actions">
      <Button type="button" disabled={pending} onclick={() => refresh("read")}
        >Обновить состояние</Button
      >
      <Button
        type="button"
        variant="primary"
        disabled={pending}
        onclick={() => refresh("reconcile")}>Согласовать состояние</Button
      >
    </div>
  {/if}
</section>

<style>
  p {
    font-size: 12px;
  }
  code {
    overflow-wrap: anywhere;
  }
</style>
