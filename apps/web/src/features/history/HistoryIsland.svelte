<script lang="ts">
  import { onMount } from "svelte";
  import { ApiError } from "../../api";
  import type { BrowserContracts } from "../../api";
  import { mountAuth } from "../../shell/auth";
  import { mountFooter } from "../../shell/footer";
  import { selectedServer, selectServer } from "../../shell/navigation";
  import ServerHeader from "../../shell/ServerHeader.svelte";
  import * as api from "./api";
  import { FIRST_PAGE } from "./conts";
  import type { HistoryState, LoadedPage, PageRequest } from "./types";
  import HistoryTable from "./HistoryTable.svelte";
  import { observeHistory } from "./live";

  let status = $state<"loading" | "authenticated" | "anonymous" | "failed">(
    "loading",
  );
  let available = $state<BrowserContracts["servers"]>([]);
  let agents = $state<BrowserContracts["agents"] | null>(null);
  let selected = $state("");
  let view = $state<HistoryState | null>(null);
  let sessionError = $state("");
  let generation: string | null = null;
  let scope = new AbortController();
  let request: AbortController | null = null;
  let footer: ReturnType<typeof mountFooter> | undefined = undefined;
  let auth: ReturnType<typeof mountAuth> | undefined = undefined;
  let stopEvents: (() => void) | undefined = undefined;
  const loaded = $derived(
    view?.kind === "ready" ? view.value : (view?.previous ?? null),
  );
  const busy = $derived(view?.kind === "loading");

  const message = (cause: unknown) => {
    const text =
      cause instanceof Error ? cause.message : "Не удалось загрузить историю.";
    return cause instanceof ApiError && cause.correlationId
      ? `${text} Код обращения: ${cause.correlationId}`
      : text;
  };
  const failure = (cause: unknown) => {
    sessionError = message(cause);
  };
  const load = async (page: PageRequest) => {
    if (!selected || !generation || status !== "authenticated") return;
    request?.abort();
    const controller = new AbortController();
    request = controller;
    const serverId = selected;
    const current = generation;
    const previous: LoadedPage | null =
      view?.kind === "ready" ? view.value : (view?.previous ?? null);
    view = { kind: "loading", request: page, previous };
    const active = () =>
      !controller.signal.aborted &&
      generation === current &&
      selected === serverId &&
      status === "authenticated";
    try {
      const data = await api.history({
        serverId,
        cursor: page.cursor,
        signal: controller.signal,
      });
      if (active())
        view = {
          kind: "ready",
          value: { request: page, data, receivedAt: new Date().toISOString() },
        };
    } catch (cause) {
      if (!active()) return;
      view = {
        kind: "failed",
        request: page,
        previous,
        message: message(cause),
      };
      if (cause instanceof ApiError && cause.status === 401)
        await auth?.refresh();
    }
  };
  const choose = async (id: string) => {
    if (selected === id && view) return;
    stopEvents?.();
    stopEvents = undefined;
    request?.abort();
    selected = id;
    view = null;
    sessionError = "";
    selectServer(id);
    const current = generation;
    const active = () =>
      generation === current && selected === id && status === "authenticated";
    stopEvents = observeHistory({
      serverId: id,
      refresh: async () => {
        if (active()) await refreshPage();
      },
      onUnauthorized: async () => {
        if (active()) await auth?.refresh();
      },
      onError: (cause) => {
        if (active()) failure(cause);
      },
    });
    await load(FIRST_PAGE);
  };
  const refreshPage = async () => {
    const page =
      view?.kind === "ready"
        ? view.value.request
        : (view?.request ?? FIRST_PAGE);
    await load(page);
  };
  const refresh = async () => {
    await refreshPage();
    await footer?.refresh();
  };
  const next = async () => {
    if (busy || !loaded?.data.next_cursor) return;
    await load({
      cursor: loaded.data.next_cursor,
      previousCursors: [
        ...loaded.request.previousCursors,
        loaded.request.cursor,
      ],
    });
  };
  const previous = async () => {
    if (busy || !loaded?.request.previousCursors.length) return;
    const cursors = loaded.request.previousCursors;
    const cursor = cursors.at(-1) ?? null;
    await load({ cursor, previousCursors: cursors.slice(0, -1) });
  };
  onMount(() => {
    const clear = () => {
      generation = null;
      scope.abort();
      request?.abort();
      request = null;
      stopEvents?.();
      stopEvents = undefined;
      view = null;
      available = [];
      agents = null;
      selected = "";
      sessionError = "";
      footer?.dispose();
      footer = undefined;
      status = "anonymous";
    };
    auth = mountAuth({
      onAuthorized: async (current) => {
        scope = new AbortController();
        generation = current;
        status = "loading";
        const signal = scope.signal;
        const list = await api.servers(signal);
        if (signal.aborted || generation !== current) return;
        available = list;
        status = "authenticated";
        footer = mountFooter((values) => {
          agents = values;
        });
        const requested = selectedServer();
        const target = requested
          ? list.find((server) => server.id === requested)
          : list[0];
        if (requested && !target)
          throw new Error(
            "Выбранный сервер не найден. Откройте историю со страницы очистки.",
          );
        if (target) await choose(target.id);
      },
      onAnonymous: clear,
      onFailure: (cause) => {
        clear();
        status = "failed";
        failure(cause);
      },
    });
    return () => {
      clear();
      auth?.dispose();
    };
  });
</script>

<div class="history-root private-content">
  {#if status === "loading"}
    <section class="card empty">Проверка сеанса…</section>
  {:else if status === "anonymous"}
    <section class="card empty">
      <p>Войдите, чтобы просмотреть историю сервера.</p>
      <a class="button primary" href="/login/">Войти в центр</a>
    </section>
  {:else if status === "failed"}
    <section class="card">
      <div class="error" role="alert">
        {sessionError || "Нет связи с центром."}
      </div>
      <button class="button" onclick={() => auth?.refresh().catch(failure)}
        >Проверить соединение</button
      >
    </section>
  {:else if available.length === 0}
    <section class="card empty">
      <p>Почтовые серверы пока не добавлены.</p>
      <a class="button primary" href="/settings/">Добавить сервер</a>
    </section>
  {:else}
    <ServerHeader
      servers={available}
      {selected}
      agent={agents?.find((agent) => agent.server_id === selected)}
      onselect={(id) => {
        choose(id).catch(failure);
      }}
      onrefresh={() => {
        refresh().catch(failure);
      }}
    />
    {#if sessionError}<div class="error" role="alert">{sessionError}</div>{/if}
    <section
      class="card"
      aria-label="История выбранного сервера"
      aria-busy={busy}
    >
      {#if view?.kind === "failed"}
        <div class="error" role="alert">{view.message}</div>
        {#if loaded}<p class="muted freshness">
            Показаны последние полученные данные от <time
              datetime={loaded.receivedAt}
              >{loaded.receivedAt.replace("T", " ")}</time
            >.
          </p>{/if}
        <button
          class="button"
          onclick={() => {
            if (view?.kind === "failed") load(view.request).catch(failure);
          }}>Повторить загрузку</button
        >
      {/if}
      {#if busy}<p class="muted" role="status">Загрузка истории…</p>{/if}
      {#if loaded}
        {#if loaded.data.jobs.length === 0}<p class="empty">
            Запусков пока нет
          </p>
        {:else}<HistoryTable jobs={loaded.data.jobs} />{/if}
        <nav class="pagination" aria-label="Страницы истории">
          <button
            class="button"
            disabled={busy || loaded.request.previousCursors.length === 0}
            onclick={() => {
              previous().catch(failure);
            }}>Предыдущая страница</button
          >
          <span aria-live="polite"
            >Страница {loaded.request.previousCursors.length + 1}</span
          >
          <button
            class="button"
            disabled={busy || !loaded.data.next_cursor}
            onclick={() => {
              next().catch(failure);
            }}>Следующая страница</button
          >
        </nav>
      {/if}
    </section>
  {/if}
</div>

<style>
  .history-root {
    min-width: 0;
  }
  .pagination {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 14px;
    margin-top: 20px;
  }
  .pagination span,
  .freshness {
    font-size: 11px;
  }
  @media (max-width: 540px) {
    .pagination {
      justify-content: center;
    }
    .pagination .button {
      flex: 1;
    }
  }
</style>
