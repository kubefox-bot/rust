<script lang="ts">
  import { onMount } from "svelte";
  import type { ConfirmedRun } from "./types";
  let {
    target,
    onconfirm,
    ondismiss,
  }: { target: ConfirmedRun; onconfirm: () => void; ondismiss: () => void } =
    $props();
  let dialog: HTMLDialogElement | undefined = undefined;
  let cancelButton: HTMLButtonElement | undefined = undefined;
  let acknowledged = $state(false);
  onMount(() => {
    dialog?.showModal();
    cancelButton?.focus();
    return () => dialog?.close();
  });
</script>

<dialog
  bind:this={dialog}
  oncancel={(event) => {
    event.preventDefault();
    ondismiss();
  }}
  aria-labelledby="confirm-title"
>
  <span class="badge">Подтверждение действия</span>
  <h2 id="confirm-title">Очистить старую почту?</h2>
  <p class="confirm-server">{target.serverName}</p>
  <dl>
    <div>
      <dt>Граница</dt>
      <dd>{target.resolvedCutoff.local.replace("T", " ")} · время сервера</dd>
    </div>
    <div>
      <dt>Режим</dt>
      <dd>{target.scan === "full" ? "Полная сверка файлов" : "По базе"}</dd>
    </div>
    <div>
      <dt>Настройки</dt>
      <dd>Версия {target.profileVersion}</dd>
    </div>
  </dl>
  <div class="notice">
    {target.servicePolicy === "manage"
      ? "Агент остановит почтовую службу на время очистки и восстановит её исходное состояние."
      : "Очистка начнётся только при остановленной почтовой службе."}
  </div>
  <label class="confirm-check"
    ><input type="checkbox" bind:checked={acknowledged} /><span
      >{target.servicePolicy === "manage"
        ? "Подтверждаю удаление и временную остановку почтовой службы."
        : "Подтверждаю удаление выбранной старой почты."}</span
    ></label
  >
  <p class="muted fine">
    Удалённые письма нельзя восстановить из этой панели. Цель и параметры
    зафиксированы по предпросмотру.
  </p>
  <div class="actions">
    <button
      class="button"
      type="button"
      bind:this={cancelButton}
      onclick={ondismiss}>Отмена</button
    ><button
      class="button danger"
      type="button"
      disabled={!acknowledged}
      onclick={onconfirm}>Подтвердить очистку</button
    >
  </div>
</dialog>

<style>
  h2 {
    margin: 18px 0 8px;
    font-size: 21px;
  }
  .confirm-server {
    font-size: 14px;
    font-weight: 600;
    margin: 0 0 18px;
  }
  dl {
    font-size: 11px;
  }
  dl > div {
    display: flex;
    gap: 16px;
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  dt {
    min-width: 80px;
    color: var(--muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .confirm-check {
    display: flex;
    align-items: start;
    gap: 10px;
    line-height: 1.7;
    font-size: 12px;
  }
  .confirm-check input {
    margin-top: 5px;
    accent-color: var(--danger);
  }
  .fine {
    font-size: 10px;
  }
  .actions {
    justify-content: flex-end;
  }
</style>
