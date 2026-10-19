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
  import { mountVisibility } from "../../../shell/visibility";
  import { fetchFilesPage } from "./api";
  import { createRefreshScheduler } from "./refresh";
  import { createPageLoader } from "./loader";
  import { clampOffset } from "./query";
  import {
    EMPTY_FILTERS,
    MAX_SEARCH_BYTES,
    PAGE_SIZE,
    PAGE_SIZE_OPTIONS,
    SEARCH_DEBOUNCE_MS,
    PROGRESS_REFRESH_MS,
  } from "./conts";
  import type { FileFilters, FilesPage, FilesQuery } from "./types";
  let {
    jobId,
    revision,
    refreshVersion,
    terminal,
    ondismiss,
  }: {
    jobId: string;
    revision: string;
    refreshVersion: number;
    terminal: boolean;
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
  let visible = $state(false);
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
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  });
  const scheduler = createRefreshScheduler({
    refresh: load,
    onerror: (cause) => {
      error = filesError(cause);
      loading = false;
    },
    delayMs: PROGRESS_REFRESH_MS,
  });
  $effect(() => {
    jobId;
    query;
    retry;
    if (!visible) return;
    untrack(() => scheduler.restart());
    return () => {
      scheduler.cancel();
      loader.cancel();
    };
  });
  $effect(() => {
    scheduler.observe({ revision, refreshVersion, terminal, visible });
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
    const unwatch = mountVisibility({
      onVisible: () => {
        visible = true;
      },
      onHidden: () => {
        visible = false;
        scheduler.cancel();
        loader.cancel();
      },
    });
    dialog?.showModal();
    closeButton?.focus();
    return () => {
      unwatch();
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
          <Button
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
        <div class="page-size">
          <Select
            label="Файлов на странице"
            options={PAGE_SIZE_OPTIONS}
            value={query.limit}
            onchange={(limit) => (query = { ...query, limit, offset: 0n })}
          />
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
  header {
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
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    gap: 16px;
    margin-top: 16px;
    align-items: flex-end;
  }
  .actions {
    margin: 0;
    align-items: flex-end;
  }
  footer :global(.button),
  footer :global(.select-trigger) {
    height: 40px;
  }
  .page-size {
    justify-self: end;
    min-width: 120px;
  }
  @media (max-width: 540px) {
    dialog {
      padding: 16px;
    }
    header {
      align-items: start;
    }
    footer {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .actions {
      grid-column: 1 / -1;
      grid-row: 2;
      justify-self: center;
    }
    .page-size {
      grid-column: 2;
      grid-row: 1;
    }
  }
</style>
