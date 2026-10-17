<script lang="ts">
  import type { BrowserContracts } from "../api";
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
</script>

<div class="server-header">
  <div class="server-selection">
    <label class="field"
      >Сервер<select
        aria-label="Сервер"
        value={selected}
        onchange={(event) => onselect(event.currentTarget.value)}
        {disabled}
        >{#each servers as server (server.id)}<option value={server.id}
            >{server.name}</option
          >{/each}</select
      ></label
    ><span
      class="badge"
      class:online={agent?.health.online && !agent.health.stale}
      ><span class="dot"></span>{agent?.health.online && !agent.health.stale
        ? "Агент на связи"
        : "Нет свежих данных"}</span
    >
  </div>
  <button
    class="button quiet"
    type="button"
    onclick={onrefresh}
    aria-label="Обновить состояние">↻ Обновить</button
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
  .server-selection .field {
    min-width: 230px;
  }
  .badge {
    height: 35px;
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
  .server-header .button {
    padding: 10px;
    font-size: 11px;
  }
  @media (max-width: 540px) {
    .server-header {
      flex-wrap: wrap;
    }
    .server-selection {
      width: 100%;
    }
    .server-selection .field {
      min-width: 0;
      flex: 1;
    }
    .server-header > .button {
      margin-left: auto;
    }
  }
</style>
