<script lang="ts">
  import { Button } from "../button";
  import { formatBytes } from "../../jobs/display";
  import { formatDateTime } from "../../lib/time";
  import { SORTABLE_COLUMNS } from "./conts";
  import type { FileRow, FileSort, SortDirection } from "./types";
  let {
    rows,
    sort,
    direction,
    onsort,
    loading = false,
  }: {
    rows: ReadonlyArray<FileRow>;
    sort: FileSort;
    direction: SortDirection;
    onsort: (sort: FileSort) => void;
    loading?: boolean;
  } = $props();
</script>

<div
  class="table-scroll"
  role="region"
  aria-label="Выбранные файлы"
  aria-busy={loading}
>
  <table>
    <thead>
      <tr
        >{#each SORTABLE_COLUMNS as column (column.id)}
          <th
            scope="col"
            aria-sort={column.id === sort
              ? direction === "asc"
                ? "ascending"
                : "descending"
              : undefined}
          >
            <Button
              size="small"
              variant="quiet"
              onclick={() => onsort(column.id)}
              >{column.header}
              {sort === column.id
                ? direction === "asc"
                  ? "↑"
                  : "↓"
                : "↕"}</Button
            >
          </th>
        {/each}<th scope="col">Связь с базой</th></tr
      >
    </thead>
    <tbody>
      {#each rows as row (row.id)}
        <tr>
          <td class="path">{row.path}</td>
          <td class="size" title={`${row.size_bytes} Б`}
            >{formatBytes(row.size_bytes)}</td
          >
          <td
            ><time datetime={row.created_at}
              >{formatDateTime(row.created_at)}</time
            ></td
          >
          <td
            ><span class:orphan={row.orphan}
              >{row.orphan ? "Без записи" : "Связан"}</span
            ></td
          >
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .table-scroll {
    overflow: auto;
    min-height: 160px;
    max-height: 55dvh;
    border: 1px solid var(--line);
    border-radius: 10px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    text-align: left;
    font-size: 11px;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
    z-index: 1;
  }
  th,
  td {
    padding: 10px 12px;
    border-bottom: 1px solid var(--line);
  }
  .path {
    min-width: 240px;
    max-width: 620px;
    overflow-wrap: anywhere;
  }
  .size,
  time {
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  td span {
    color: var(--success);
    white-space: nowrap;
  }
  td span.orphan {
    color: var(--muted);
  }
</style>
