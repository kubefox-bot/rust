<script lang="ts">
  import { Button } from "../../../ui/button";
  import { Checkbox } from "../../../ui/checkbox";
  import { onMount } from "svelte";
  import type { AgentAction } from "./types";
  let {
    action,
    pending,
    onconfirm,
    onclose,
  }: {
    action: AgentAction;
    pending: boolean;
    onconfirm: () => Promise<void>;
    onclose: () => void;
  } = $props();
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
  aria-labelledby="agent-action-title"
  oncancel={(event) => {
    event.preventDefault();
    if (!pending) onclose();
  }}
>
  <h2 id="agent-action-title">
    {action.kind === "renew"
      ? "Обновить сертификат агента?"
      : "Отозвать доступ агента?"}
  </h2>
  <p>Целевой сервер: <strong>{action.server.name}</strong></p>
  <p class="muted">
    Идентификатор агента: <code>{action.server.agent_id}</code>
  </p>
  <p>
    {action.kind === "renew"
      ? "Центр проверит действующий канал к этому агенту и обновит сертификат. Если доверие утрачено, используйте восстановление по SSH."
      : "Центр перестанет доверять этому агенту. Восстановление сертификата по SSH не возвращает отозванный доступ."}
  </p>
  <Checkbox
    label="Подтверждаю действие для этого сервера"
    bind:checked={acknowledged}
    disabled={pending}
  />
  <div class="actions">
    <Button
      type="button"
      bind:element={cancelButton}
      disabled={pending}
      onclick={onclose}>Отмена</Button
    ><Button
      loading={pending}
      type="button"
      variant={action.kind === "revoke" ? "danger" : "primary"}
      disabled={!acknowledged || pending}
      onclick={onconfirm}>{pending ? "Выполнение…" : "Подтвердить"}</Button
    >
  </div>
</dialog>

<style>
  p {
    font-size: 12px;
  }
  code {
    overflow-wrap: anywhere;
  }
</style>
