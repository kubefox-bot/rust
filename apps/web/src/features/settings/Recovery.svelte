<script lang="ts">
  import { Button } from "../../ui/button";
  import { Input } from "../../ui/input";
  import { onDestroy, tick } from "svelte";
  import { confirmRecovery, prepareRecovery } from "./account/api";
  import {
    MAX_PASSWORD_INPUT_LENGTH,
    MILLISECONDS_PER_SECOND,
    SECONDS_PER_MINUTE,
  } from "./account/conts";
  import { failureMessage } from "./account/failure-message";
  import type { RecoveryState } from "./account/types";

  let { oncommitted }: { oncommitted: () => void } = $props();
  let recoveryState = $state<RecoveryState>({ kind: "password" });
  let currentPassword = $state("");
  let confirmationKey = $state("");
  let error = $state("");
  let confirmationInput = $state<HTMLInputElement | undefined>(undefined);
  let expiryTimer: ReturnType<typeof setTimeout> | undefined = undefined;
  let active = true;

  const clearCandidate = () => {
    clearTimeout(expiryTimer);
    expiryTimer = undefined;
    currentPassword = "";
    confirmationKey = "";
    recoveryState = { kind: "password" };
  };
  onDestroy(() => {
    active = false;
    clearCandidate();
  });
  const prepare = async (event: SubmitEvent) => {
    event.preventDefault();
    if (recoveryState.kind !== "password") return;
    recoveryState = { kind: "preparing" };
    error = "";
    try {
      const operation = prepareRecovery({ current_password: currentPassword });
      currentPassword = "";
      const candidate = await operation;
      if (!active) return;
      recoveryState = { kind: "display", candidate };
      expiryTimer = setTimeout(() => {
        if (!active) return;
        clearCandidate();
        error = "Срок подтверждения ключа истёк. Создайте новый ключ.";
      }, candidate.expires_in_seconds * MILLISECONDS_PER_SECOND);
    } catch (failure) {
      if (!active) return;
      clearCandidate();
      error = failureMessage(
        failure,
        "Не удалось подготовить ключ. Создайте новый ключ после проверки сессии.",
      );
    } finally {
      if (active) currentPassword = "";
    }
  };
  const saved = async () => {
    if (recoveryState.kind !== "display") return;
    recoveryState = {
      kind: "confirm",
      candidateId: recoveryState.candidate.candidate_id,
    };
    await tick();
    if (active) confirmationInput?.focus();
  };
  const confirm = async (event: SubmitEvent) => {
    event.preventDefault();
    if (recoveryState.kind !== "confirm") return;
    const candidateId = recoveryState.candidateId;
    recoveryState = { kind: "confirming", candidateId };
    error = "";
    try {
      const operation = confirmRecovery({
        candidate_id: candidateId,
        recovery_key: confirmationKey,
      });
      confirmationKey = "";
      await operation;
      if (!active) return;
      clearCandidate();
      oncommitted();
    } catch (failure) {
      if (!active || recoveryState.kind !== "confirming") return;
      recoveryState = { kind: "confirm", candidateId };
      error = failureMessage(
        failure,
        "Замена ключа не подтверждена. Проверьте обычный вход перед повтором.",
      );
    } finally {
      if (active) confirmationKey = "";
    }
  };
  const discard = () => {
    clearCandidate();
    error = "";
  };
</script>

<section class="card" aria-labelledby="recovery-heading">
  <h2 id="recovery-heading">Ключ восстановления</h2>
  <p class="subtitle">
    Существующий ключ недоступен для чтения. До подтверждения нового ключа
    действует прежний.
  </p>
  {#if recoveryState.kind === "password" || recoveryState.kind === "preparing"}
    <form onsubmit={prepare} autocomplete="off">
      <label class="field"
        >Текущий пароль для замены ключа<Input
          type="password"
          autocomplete="current-password"
          required
          maxlength={MAX_PASSWORD_INPUT_LENGTH}
          bind:value={currentPassword}
          disabled={recoveryState.kind === "preparing"}
        /></label
      >
      <div class="actions">
        <Button
          loading={recoveryState.kind === "preparing"}
          type="submit"
          disabled={recoveryState.kind === "preparing"}
          >{recoveryState.kind === "preparing"
            ? "Подготовка ключа…"
            : "Создать новый ключ"}</Button
        >
      </div>
    </form>
  {:else if recoveryState.kind === "display"}
    <div class="notice">
      Сохраните ключ вне центра. Он показан один раз; после перехода к
      подтверждению увидеть его снова нельзя. Подтвердите его в течение {Math.ceil(
        recoveryState.candidate.expires_in_seconds / SECONDS_PER_MINUTE,
      )} минут.
    </div>
    <output class="recovery-key" aria-label="Новый ключ восстановления"
      >{recoveryState.candidate.recovery_key}</output
    >
    <div class="actions">
      <Button variant="primary" type="button" onclick={saved}
        >Я сохранил ключ</Button
      ><Button type="button" onclick={discard}>Отменить замену</Button>
    </div>
  {:else}
    <form onsubmit={confirm} autocomplete="off">
      <label class="field"
        >Повторите ключ восстановления<Input
          bind:element={confirmationInput}
          type="password"
          autocomplete="off"
          autocapitalize="off"
          spellcheck={false}
          required
          maxlength={MAX_PASSWORD_INPUT_LENGTH}
          bind:value={confirmationKey}
          disabled={recoveryState.kind === "confirming"}
        /></label
      >
      <p class="subtitle">
        Введите сохранённый ключ. После подтверждения потребуется обычный вход.
      </p>
      <div class="actions">
        <Button
          loading={recoveryState.kind === "confirming"}
          variant="primary"
          type="submit"
          disabled={recoveryState.kind === "confirming"}
          >{recoveryState.kind === "confirming"
            ? "Подтверждение…"
            : "Подтвердить новый ключ"}</Button
        ><Button
          type="button"
          onclick={discard}
          disabled={recoveryState.kind === "confirming"}>Отменить замену</Button
        >
      </div>
    </form>
  {/if}
  {#if error}<div class="error" role="alert">{error}</div>{/if}
</section>

<style>
  form {
    margin-top: 22px;
  }
  .field {
    max-width: 480px;
  }
  form > p {
    margin-top: 12px;
  }
  .recovery-key {
    display: block;
    overflow-wrap: anywhere;
    user-select: all;
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 16px;
    font-family: ui-monospace, monospace;
    font-size: 13px;
    line-height: 1.7;
    background: #f8f9fc;
  }
</style>
