<script lang="ts">
  import { Button } from "../ui/button";
  import type { BrowserContracts } from "../api";
  import { Select } from "../ui/select";
  let {
    servers,
    selected,
    agent,
    onselect,
    onrefresh,
    disabled = false,
  }: {
    servers: BrowserContracts["servers"];
    selected: string;
    agent: BrowserContracts["agents"][number] | undefined;
    onselect: (id: string) => void;
    onrefresh: () => void;
    disabled?: boolean;
  } = $props();
  const options = $derived(
    servers.map((server) => ({ value: server.id, label: server.name })),
  );
</script>

<div class="server-header">
  <div class="server-selection">
    <div class="server-picker">
      <Select
        label="Сервер"
        {options}
        value={selected}
        onchange={onselect}
        {disabled}
      />
    </div>
    <span
      class="badge"
      class:online={agent?.health.online && !agent.health.stale}
      ><span class="dot"></span>{agent?.health.online && !agent.health.stale
        ? "Агент на связи"
        : "Нет свежих данных"}</span
    >
  </div>
  <Button
    variant="quiet"
    type="button"
    onclick={onrefresh}
    aria-label="Обновить состояние">↻ Обновить</Button
  >
</div>

<style>
  .server-header {
    display: flex;
    justify-content: space-between;
    align-items: end;
    gap: 16px;
    margin-bottom: 24px;
  }
  .server-selection {
    display: flex;
    align-items: end;
    gap: 12px;
    flex-wrap: wrap;
  }
  .server-picker {
    min-width: 230px;
  }
  .badge {
    height: 40px;
    color: var(--muted);
    background: #fff;
  }
  .badge.online {
    color: var(--success);
    background: #f0faf5;
    border-color: #d2ecdf;
  }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: currentColor;
  }
  @media (max-width: 540px) {
    .server-header {
      flex-wrap: wrap;
    }
    .server-selection {
      width: 100%;
    }
    .server-picker {
      min-width: 0;
      flex: 1;
    }
    .server-header > :global(.button) {
      margin-left: auto;
    }
  }
</style>
