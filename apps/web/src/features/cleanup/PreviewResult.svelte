<script lang="ts">
  import type { Job } from "./types";
  import { formatBytes, formatCount } from "../../jobs/display";
  import { formatServerDateTime } from "../../lib/time";

  let { preview }: { preview: Job } = $props();
  const summary = $derived(preview.snapshot?.summary);
  const cutoff = $derived(preview.intent.resolved_cutoff.local);
</script>

<section class="preview-result" aria-label="Результат предпросмотра">
  <h3>Результат предпросмотра</h3>
  <dl>
    <div>
      <dt>Файлы</dt>
      <dd>{formatCount(summary?.selected_files)}</dd>
    </div>
    <div>
      <dt>Объём</dt>
      <dd>{formatBytes(summary?.selected_bytes)}</dd>
    </div>
  </dl>
  <p>
    До <time datetime={cutoff}>{formatServerDateTime(cutoff)}</time> · время сервера
  </p>
</section>

<style>
  .preview-result {
    padding: 12px 14px;
    border: 1px solid var(--accent-line);
    border-radius: 10px;
    background: var(--accent-soft);
  }
  h3 {
    margin: 0 0 10px;
    font-size: 11px;
    color: var(--accent);
  }
  dl {
    display: flex;
    flex-wrap: wrap;
    gap: 12px 28px;
    margin: 0;
  }
  dl > div {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  dt,
  p {
    font-size: 10px;
    color: var(--muted);
  }
  dd {
    margin: 0;
    font-size: 20px;
    font-weight: 650;
    font-variant-numeric: tabular-nums;
    overflow-wrap: anywhere;
  }
  p {
    margin: 8px 0 0;
    overflow-wrap: anywhere;
  }
</style>
