<script lang="ts">
  import "./agent-status.css";
  import { agentBadge } from "./agent-status";
  import type { BrowserContracts } from "../api";
  let { agent }: { agent: BrowserContracts["agents"][number] | undefined } =
    $props();
  const status = $derived(agentBadge(agent));
</script>

<span class="badge agent-status" data-status={status.state}>
  <span class="dot" aria-hidden="true"></span>{status.label}
</span>
