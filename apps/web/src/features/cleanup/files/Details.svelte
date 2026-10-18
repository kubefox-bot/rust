<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { filesError } from "./error";
  import Filters from "./Filters.svelte";
  import { resolveFilters } from "./filters";
  import { Button } from "../../../ui/button";
  import { Select } from "../../../ui/select";
  import { Input } from "../../../ui/input";
  import { FileGrid } from "../../../ui/file-grid";
  import type { FileSort } from "../../../ui/file-grid";
  import { formatCount } from "../../../jobs/display";
  import { fetchFilesPage } from "./api";
  import { createRefreshScheduler } from "./refresh";
  import { createPageLoader } from "./loader";
  import { clampOffset } from "./query";
  import {
    EMPTY_FILTERS,
    MAX_SEARCH_BYTES,
    PAGE_SIZE,
    PAGE_SIZE_OPTIONS,
    REFRESH_DELAY_MS,
  } from "./conts";
  import type { FileFilters, FilesPage, FilesQuery } from "./types";
  let {
    jobId,
    revision,
    refreshVersion,
    ondismiss,
  }: {
    jobId: string;
    revision: string;
    refreshVersion: number;
    ondismiss: () => void;
  } = $props();
  let dialog: HTMLDialogElement | undefined = undefined;
  let closeButton = $state<HTMLButtonElement | undefined>(undefined);
  let search = $state("");
  let filters = $state<FileFilters>(EMPTY_FILTERS);
  let filterError = $state("");
  let query = $state<FilesQuery>({
    search: "",
    sort: "path",
    direction: "asc",
    offset: 0n,
    limit: PAGE_SIZE,
  });
  let page = $state<FilesPage | null>(null);
  let error = $state("");
  let loading = $state(true);
  let retry = $state(0);
  const loader = createPageLoader((signal) =>
    fetchFilesPage({ jobId, query, signal }),
  );
  const load = async () => {
    if (filterError) return;
    if (new TextEncoder().encode(search).length > MAX_SEARCH_BYTES) {
      page = null;
      loading = false;
      error = "Поисковый запрос слишком длинный (не более 512 байт).";
      return;
    }
    if (search !== query.search) return;
    loading = true;
    error = "";
    try {
      const loaded = await loader.load();
      if (!loaded?.current()) return;
      const result = loaded.page;
      if (result.job_id !== jobId)
        throw new Error("Сервер вернул файлы другого задания.");
      const offset = clampOffset({
        offset: query.offset,
        total: result.total,
        limit: query.limit,
      });
      if (offset !== query.offset) {
        query = { ...query, offset };
        return;
      }
      page = result;
      loading = false;
    } catch (cause) {
      page = null;
      error = filesError(cause);
      loading = false;
    }
  };
  let lastSearch = "";
  $effect(() => {
    const value = search;
    if (value === lastSearch) return;
    lastSearch = value;
    scheduler.cancel();
    loader.cancel();
    const timer = setTimeout(() => {
      if (new TextEncoder().encode(value).length > MAX_SEARCH_BYTES) {
        loader.cancel();
        loading = false;
        error = "Поисковый запрос слишком длинный (не более 512 байт).";
        return;
      }
      query = { ...query, search: value, offset: 0n };
    }, REFRESH_DELAY_MS);
    return () => clearTimeout(timer);
  });
  const scheduler = createRefreshScheduler({
    refresh: load,
    onerror: (cause) => {
      error = filesError(cause);
      loading = false;
    },
    delayMs: REFRESH_DELAY_MS,
  });
  $effect(() => {
    jobId;
    query;
    retry;
    untrack(() => scheduler.restart());
    return () => {
      scheduler.cancel();
      loader.cancel();
    };
  });
  $effect(() => {
    revision;
    refreshVersion;
    scheduler.schedule();
  });
  const changeFilters = (value: FileFilters) => {
    filters = value;
    scheduler.cancel();
    loader.cancel();
    const resolved = resolveFilters(value);
    filterError = resolved.error;
    if (filterError) {
      page = null;
      loading = false;
      return;
    }
    query = {
      ...query,
      createdFrom: undefined,
      createdTo: undefined,
      orphan: undefined,
      ...resolved.filters,
      offset: 0n,
    };
  };
  const resetFilters = () => {
    search = "";
    changeFilters(EMPTY_FILTERS);
    query = { ...query, search: "", offset: 0n };
  };
  const sortBy = (sort: FileSort) => {
    query = {
      ...query,
      sort,
      direction:
        query.sort === sort && query.direction === "asc" ? "desc" : "asc",
      offset: 0n,
    };
  };
  onMount(() => {
    dialog?.showModal();
    closeButton?.focus();
    return () => {
      scheduler.cancel();
      loader.cancel();
      dialog?.close();
    };
  });
</script>

<dialog
  bind:this={dialog}
  aria-labelledby="file-details-title"
  oncancel={(event) => {
    event.preventDefault();
    ondismiss();
  }}
>
  <header>
    <div>
      <span class="badge">Предпросмотр</span>
      <h2 id="file-details-title">Выбранные файлы</h2>
    </div>
    <Button bind:element={closeButton} onclick={ondismiss}>Закрыть</Button>
  </header>
  <p class="muted">
    Список содержит только выбранные физические файлы. Время создания показано в
    часовом поясе браузера.
  </p>
  <label for="file-search">Поиск по пути</label>
  <Input
    id="file-search"
    type="search"
    bind:value={search}
    placeholder="Часть пути или имени файла"
  />
  <Filters value={filters} onchange={changeFilters} onreset={resetFilters} />
  {#if filterError}<p role="alert" class="error">{filterError}</p>
  {:else if error}<div role="alert" class="error">{error}</div>
    <Button onclick={() => retry++}>Повторить</Button>
  {:else}
    <p class="state" role="status">
      {#if loading}Загрузка файлов…{:else if page?.complete}Сбор завершён{:else}Список
        пополняется по мере сканирования…{/if}
      {#if page}Найдено: {formatCount(page.total)}{/if}
    </p>
    {#if page}
      {#if page.rows.length > 0}<FileGrid
          rows={page.rows}
          sort={query.sort}
          direction={query.direction}
          onsort={sortBy}
          {loading}
        />
      {:else}<div class="empty">
          {page.complete
            ? "Подходящих файлов нет."
            : "Подходящие файлы пока не найдены. Сканирование продолжается."}
        </div>{/if}
      <footer>
        <span
          >{#if page.total !== "0"}{formatCount(
              String(query.offset + 1n),
            )}–{formatCount(String(query.offset + BigInt(page.rows.length)))} из {formatCount(
              page.total,
            )}{:else}0 файлов{/if}</span
        >
        <div class="actions">
          <Select
            label="Файлов на странице"
            options={PAGE_SIZE_OPTIONS}
            value={query.limit}
            onchange={(limit) => (query = { ...query, limit, offset: 0n })}
          /><Button
            disabled={loading || query.offset === 0n}
            onclick={() =>
              (query = {
                ...query,
                offset: query.offset - BigInt(query.limit),
              })}>Назад</Button
          ><Button
            disabled={loading ||
              query.offset + BigInt(query.limit) >= BigInt(page.total)}
            onclick={() =>
              (query = {
                ...query,
                offset: query.offset + BigInt(query.limit),
              })}>Далее</Button
          >
        </div>
      </footer>
    {/if}
  {/if}
</dialog>

<style>
  dialog {
    width: min(1200px, calc(100vw - 32px));
    max-width: none;
    max-height: calc(100dvh - 32px);
    padding: 24px;
  }
  header,
  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  h2 {
    margin: 12px 0;
    font-size: 21px;
  }
  p,
  label,
  footer {
    font-size: 12px;
  }
  label {
    display: block;
    margin-bottom: 8px;
  }
  .state {
    color: var(--muted);
    min-height: 18px;
  }
  footer {
    margin-top: 16px;
    flex-wrap: wrap;
    align-items: flex-end;
  }
  .actions {
    margin: 0;
    align-items: flex-end;
  }
  .actions :global(.button),
  .actions :global(.select-trigger) {
    height: 40px;
  }
  @media (max-width: 540px) {
    dialog {
      padding: 16px;
    }
    header {
      align-items: start;
    }
  }
</style>
