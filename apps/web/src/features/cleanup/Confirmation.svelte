<script lang="ts">
  import { Button } from "../../ui/button";
  import { Checkbox } from "../../ui/checkbox";
  import { onMount } from "svelte";
  import type { ConfirmedRun } from "./types";
  import { formatServerDateTime } from "../../lib/time";
  let {
    target,
    onconfirm,
    ondismiss,
  }: { target: ConfirmedRun; onconfirm: () => void; ondismiss: () => void } =
    $props();
  let dialog: HTMLDialogElement | undefined = undefined;
  let cancelButton = $state<HTMLButtonElement | undefined>(undefined);
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
      <dd>
        <time datetime={target.resolvedCutoff.local}
          >{formatServerDateTime(target.resolvedCutoff.local)}</time
        > · время сервера
      </dd>
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
  <Checkbox
    tone="danger"
    bind:checked={acknowledged}
    label={target.servicePolicy === "manage"
      ? "Подтверждаю удаление и временную остановку почтовой службы."
      : "Подтверждаю удаление выбранной старой почты."}
  />
  <p class="muted fine">
    Удалённые письма нельзя восстановить из этой панели. Цель и параметры
    зафиксированы по предпросмотру.
  </p>
  <div class="actions">
    <Button type="button" bind:element={cancelButton} onclick={ondismiss}
      >Отмена</Button
    ><Button
      variant="danger"
      type="button"
      disabled={!acknowledged}
      onclick={onconfirm}>Подтвердить очистку</Button
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
  .fine {
    font-size: 10px;
  }
  .actions {
    justify-content: flex-end;
  }
</style>
