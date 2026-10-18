<script lang="ts">
  import type { Readable } from "svelte/store";
  import { ButtonLink } from "../../ui/button";
  import { isNewerAgentBuild } from "../../agents";
  import { formatDateTime } from "../../lib/time";
  import AgentStatus from "../AgentStatus.svelte";
  import { agentUpdateUrl } from "../navigation";
  import type { AgentListView } from "./types";
  let { entries }: { entries: Readable<AgentListView> } = $props();
</script>

{#each $entries.agents as agent (agent.agent_id)}
  <li data-server-id={agent.server_id}>
    <strong>{agent.name}</strong>
    <AgentStatus {agent} />
    <div class="agent-version">
      <span
        >{agent.health.version
          ? `Версия: ${agent.health.version}`
          : "Версия неизвестна"}</span
      >
      {#if !agent.revoked && !agent.certificate_expired && agent.health.version && $entries.packages?.some( (candidate) => isNewerAgentBuild( { current: agent.health.version ?? "", available: candidate.build } ) )}
        <ButtonLink
          variant="link"
          size="small"
          href={agentUpdateUrl({ id: agent.server_id })}
          >Обновить агента</ButtonLink
        >
      {/if}
    </div>
    <span class="agent-contact">
      {#if agent.health.observed_at}
        Последний контакт: <time datetime={agent.health.observed_at}
          >{formatDateTime(agent.health.observed_at)}</time
        >
      {:else}Контакт ещё не получен{/if}
    </span>
  </li>
{/each}
