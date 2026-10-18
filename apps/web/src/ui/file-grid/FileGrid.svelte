<script lang="ts">
  import { createTable, tableFeatures } from "@tanstack/svelte-table";
  import { Button } from "../button";
  import { formatBytes } from "../../jobs/display";
  import { formatDateTime } from "../../lib/time";
  import { FILE_COLUMNS } from "./conts";
  import type { FileRow, FileSort, SortDirection } from "./types";
  let {
    rows,
    sort,
    direction,
    onsort,
    loading = false,
  }: {
    rows: FileRow[];
    sort: FileSort;
    direction: SortDirection;
    onsort: (sort: FileSort) => void;
    loading?: boolean;
  } = $props();
  const features = tableFeatures({});
  const table = createTable({
    features,
    columns: FILE_COLUMNS,
    get data() {
      return rows;
    },
    getRowId: (row) => row.id,
  });
</script>

<div
  class="table-scroll"
  role="region"
  aria-label="Выбранные файлы"
  aria-busy={loading}
>
  <table>
    <thead>
      {#each table.getHeaderGroups() as group (group.id)}
        <tr
          >{#each group.headers as header (header.id)}
            <th
              aria-sort={header.id === sort
                ? direction === "asc"
                  ? "ascending"
                  : "descending"
                : undefined}
            >
              {#if header.id === "path" || header.id === "size" || header.id === "created_at"}
                {@const key = header.id}
                <Button size="small" variant="quiet" onclick={() => onsort(key)}
                  >{header.column.columnDef.header}
                  {sort === key
                    ? direction === "asc"
                      ? "↑"
                      : "↓"
                    : "↕"}</Button
                >
              {:else}Связь с базой{/if}
            </th>
          {/each}</tr
        >
      {/each}
    </thead>
    <tbody>
      {#each table.getRowModel().rows as row (row.id)}
        <tr>
          <td class="path">{row.original.path}</td>
          <td class="size" title={`${row.original.size_bytes} Б`}
            >{formatBytes(row.original.size_bytes)}</td
          >
          <td
            ><time datetime={row.original.created_at}
              >{formatDateTime(row.original.created_at)}</time
            ></td
          >
          <td
            ><span class:orphan={row.original.orphan}
              >{row.original.orphan ? "Без записи" : "Связан"}</span
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
