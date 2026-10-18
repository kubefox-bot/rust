<script lang="ts">
  import type { BrowserContracts } from "../api";
  let { agent }: { agent: BrowserContracts["agents"][number] | undefined } =
    $props();
  const status = $derived(
    agent?.revoked
      ? { tone: "danger", label: "Доступ отозван" }
      : agent?.health.stale
        ? { tone: "muted", label: "Данные устарели" }
        : agent?.health.online
          ? { tone: "online", label: "Агент на связи" }
          : { tone: "muted", label: "Нет свежих данных" },
  );
</script>

<span class="badge agent-status" data-tone={status.tone}>
  <span class="dot" aria-hidden="true"></span>{status.label}
</span>

<style>
  .agent-status {
    height: 40px;
    color: var(--muted);
    background: #fff;
  }
  .agent-status[data-tone="online"] {
    color: var(--success);
    background: #f0faf5;
    border-color: #d2ecdf;
  }
  .agent-status[data-tone="danger"] {
    color: var(--danger);
    background: #fff1f2;
    border-color: #fecdd3;
  }
  .dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: currentColor;
  }
</style>
