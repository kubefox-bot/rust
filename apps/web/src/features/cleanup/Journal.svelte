<script lang="ts">
  import { tick } from "svelte";
  import type { JournalEntry } from "./types";
  import { JOURNAL_FOLLOW_DISTANCE_PX, JOURNAL_LIMIT } from "./conts";
  let {
    entries,
    expanded,
    onexpanded,
  }: {
    entries: ReadonlyArray<JournalEntry>;
    expanded: boolean;
    onexpanded: (value: boolean) => void;
  } = $props();
  let log: HTMLDivElement | undefined = undefined;
  let toggleButton: HTMLButtonElement | undefined = undefined;
  let follow = true;
  const resize = (value: boolean) => {
    onexpanded(value);
    if (!value)
      tick()
        .then(() => toggleButton?.focus())
        .catch(() => {
          follow = false;
        });
  };
  $effect(() => {
    entries.at(-1)?.key;
    if (follow)
      tick()
        .then(() => {
          if (log) log.scrollTop = log.scrollHeight;
        })
        .catch(() => {
          follow = false;
        });
  });
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape" && expanded) {
      event.preventDefault();
      resize(false);
    }
  }}
/>
<section class="card journal" class:expanded>
  <div class="journal-header">
    <div>
      <h2>Журнал событий</h2>
      <span>Время UTC · последние {JOURNAL_LIMIT} событий</span>
    </div>
    <button
      class="button quiet"
      type="button"
      bind:this={toggleButton}
      onclick={() => resize(!expanded)}
      aria-expanded={expanded}
      >{expanded ? "Свернуть журнал" : "Развернуть журнал"}</button
    >
  </div>
  <div
    class="log"
    bind:this={log}
    role="log"
    aria-label="Журнал событий задания"
    aria-live="polite"
    onscroll={() => {
      if (log)
        follow =
          log.scrollHeight - log.scrollTop - log.clientHeight <
          JOURNAL_FOLLOW_DISTANCE_PX;
    }}
  >
    {#if entries.length === 0}<div class="journal-empty">
        <span class="log-cursor"></span>Здесь появятся события выбранного
        задания.
      </div>{:else}{#each entries as entry (entry.key)}<div
          class="log-entry"
          class:failed={entry.failed}
        >
          <time datetime={entry.at}>{entry.at.slice(11, 19)}</time><span
            >{entry.text}</span
          >
        </div>{/each}{/if}
  </div>
</section>

<style>
  .journal {
    padding: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    min-height: 250px;
  }
  .journal-header {
    padding: 18px 22px;
    border-bottom: 1px solid var(--line);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .journal-header h2 {
    margin: 0 0 5px;
    font-size: 13px;
  }
  .journal-header span {
    font-size: 9px;
    color: var(--muted);
  }
  .journal-header button {
    font-size: 10px;
    padding: 8px 10px;
  }
  .log {
    padding: 15px 22px;
    max-height: 300px;
    min-height: 160px;
    overflow: auto;
    background: #fcfdff;
    font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
    font-size: 10px;
    line-height: 1.9;
  }
  .log-entry {
    display: grid;
    grid-template-columns: 65px minmax(0, 1fr);
    gap: 12px;
    padding: 3px 0;
    color: #536178;
    overflow-wrap: anywhere;
  }
  .log-entry time {
    color: #9aa4b2;
  }
  .log-entry.failed {
    color: var(--danger);
  }
  .journal-empty {
    display: flex;
    align-items: center;
    gap: 10px;
    color: #98a1b0;
    padding: 6px 0;
  }
  .log-cursor {
    width: 5px;
    height: 10px;
    background: #c2cbd8;
    flex-shrink: 0;
  }
  .journal.expanded {
    flex: 1;
    min-height: 300px;
  }
  .expanded .log {
    flex: 1;
    max-height: calc(100dvh - 420px);
  }
  @media (max-width: 540px) {
    .journal-header {
      padding: 16px;
    }
    .log {
      padding: 14px 16px;
    }
    .log-entry {
      grid-template-columns: 55px minmax(0, 1fr);
      gap: 8px;
    }
    .journal-header {
      align-items: start;
    }
    .journal-header button {
      font-size: 9px;
    }
    .expanded .log {
      max-height: calc(100dvh - 480px);
    }
  }
</style>
