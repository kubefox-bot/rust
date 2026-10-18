<script lang="ts">
  import { Button, ButtonLink } from "../../ui/button";
  import { onMount } from "svelte";
  import { ApiError, subscribe } from "../../api";
  import type { BrowserContracts } from "../../api";
  import { mountAuth } from "../../shell/auth";
  import { mountFooter } from "../../shell/footer";
  import { selectedServer, selectServer } from "../../shell/navigation";
  import * as api from "./api";
  import { initialState, transition } from "./model";
  import { createContextLoader } from "./context-loader";
  import { REFRESH_INTERVAL_MS } from "./conts";
  import type { CleanupState, ConfirmedRun, Job } from "./types";
  import ServerHeader from "../../shell/ServerHeader.svelte";
  import RunForm from "./RunForm.svelte";
  import ProgressPanel from "./ProgressPanel.svelte";
  import Journal from "./Journal.svelte";
  import Confirmation from "./Confirmation.svelte";
  let status = $state<"loading" | "authenticated" | "anonymous" | "failed">(
    "loading",
  );
  let available = $state<BrowserContracts["servers"]>([]);
  let agents = $state<BrowserContracts["agents"] | null>(null);
  let selected = $state("");
  let view = $state<CleanupState | null>(null);
  let busy = $state(false);
  let detailsJobId = $state<string | null>(null);
  let detailsImport = $state<Promise<
    typeof import("./files/Details.svelte")
  > | null>(null);
  let filesRefreshVersion = $state(0);
  const detailsTarget = $derived(
    view?.job?.intent.operation === "preview"
      ? view.job
      : (view?.preview ?? null),
  );
  const openDetails = () => {
    if (!detailsTarget) return;
    detailsJobId = detailsTarget.job_id;
    detailsImport = import("./files/Details.svelte");
  };
  let error = $state("");
  let generation: string | null = null;
  let scope = new AbortController();
  let operation: AbortController | null = null;
  let operationId = 0;
  const contexts = createContextLoader(api.context);
  let subscription: (() => void) | undefined = undefined;
  let footer: ReturnType<typeof mountFooter> | undefined = undefined;
  let auth: ReturnType<typeof mountAuth> | undefined = undefined;
  const active = (id: string, current: string | null) =>
    selected === id &&
    generation === current &&
    status === "authenticated" &&
    !scope.signal.aborted;
  const failure = (cause: unknown) => {
    if (cause instanceof DOMException && cause.name === "AbortError") return;
    error = cause instanceof Error ? cause.message : "Операция не выполнена.";
    if (cause instanceof ApiError && cause.correlationId)
      error += ` Код обращения: ${cause.correlationId}`;
    if (cause instanceof ApiError && cause.status === 401)
      auth?.refresh().catch(() => {
        status = "failed";
      });
  };
  const observeJob = (job: Job): boolean => {
    if (!view) return false;
    const previous = view;
    const next = transition(previous, { type: "job", job });
    const advanced = Boolean(
      next.job &&
      (next.job.job_id !== previous.job?.job_id ||
        next.job.stream_epoch !== previous.job?.stream_epoch ||
        next.job.revision !== previous.job?.revision ||
        next.job.slot_held !== previous.job?.slot_held),
    );
    if (advanced) contexts.invalidate();
    view = next;
    return advanced;
  };
  const refresh = async (options?: Readonly<{ owner: AbortController }>) => {
    if (
      (busy && options?.owner !== operation) ||
      !view ||
      !selected ||
      !generation ||
      status !== "authenticated"
    )
      return;
    const id = selected;
    const current = generation;
    const signal = options?.owner.signal ?? scope.signal;
    const loaded = await contexts.load({ serverId: id, signal });
    if (loaded?.current() && active(id, current))
      view = view
        ? transition(view, { type: "context", context: loaded.context })
        : initialState(loaded.context);
  };
  const choose = async (id: string) => {
    if (selected === id && view?.context.server.id === id) return;
    contexts.invalidate();
    subscription?.();
    subscription = undefined;
    operation?.abort();
    operationId++;
    busy = false;
    scope.abort();
    scope = new AbortController();
    selected = id;
    view = null;
    detailsJobId = null;
    error = "";
    selectServer({ id, servers: available });
    const current = generation;
    const loaded = await contexts.load({ serverId: id, signal: scope.signal });
    if (!loaded?.current() || !active(id, current)) return;
    view = initialState(loaded.context);
    subscription = subscribe({
      serverId: id,
      onEvent: (event) => {
        if (active(id, current) && view) {
          const needsLatest =
            view.dispatch.kind === "settled" &&
            !view.job?.slot_held &&
            view.job?.job_id !== event.job.job_id;
          const advanced = observeJob(event.job);
          if (needsLatest || (advanced && !event.job.slot_held))
            refresh().catch(failure);
        }
      },
      onResync: () => {
        if (active(id, current)) {
          filesRefreshVersion++;
          refresh().catch(failure);
        }
      },
      onUnauthorized: () => {
        auth?.refresh().catch(failure);
      },
    });
  };
  const beginOperation = () => {
    contexts.invalidate();
    operation?.abort();
    const controller = new AbortController();
    operation = controller;
    const id = ++operationId;
    busy = true;
    error = "";
    return { controller, id };
  };
  const send = async (
    input: BrowserContracts["submit"],
    signal: AbortSignal,
  ) => {
    const current = generation;
    if (!view || !active(input.server_id, current)) return;
    contexts.invalidate();
    view = transition(view, { type: "dispatch", intent: input });
    try {
      const job = await api.submit(input, signal);
      if (active(input.server_id, current) && view) observeJob(job);
    } catch (cause) {
      if (active(input.server_id, current) && view)
        view = transition(view, { type: "uncertain" });
      throw cause;
    }
  };
  const preview = async () => {
    if (!view || busy) return;
    const state = view;
    const id = state.context.server.id;
    const current = generation;
    const task = beginOperation();
    try {
      const loaded = await contexts.load({
        serverId: id,
        signal: task.controller.signal,
      });
      if (!active(id, current) || task.controller.signal.aborted) return;
      if (!loaded?.current())
        throw new Error(
          "Состояние сервера изменилось. Проверьте его и повторите предпросмотр.",
        );
      const context = loaded.context;
      view = transition(view ?? state, { type: "context", context });
      if (context.busy)
        throw new Error(
          "Сейчас выполняется другая операция. Дождитесь её завершения.",
        );
      if (!context.configured)
        throw new Error("Сначала настройте подключение к базе и пути сервера.");
      const validation = await api.validate(id, task.controller.signal);
      if (!active(id, current)) return;
      if (
        state.parameters.scan === "full" &&
        validation.mail_state !== "stopped"
      )
        throw new Error(
          "Для полного предпросмотра сначала остановите почтовую службу. Предпросмотр не останавливает её автоматически.",
        );
      const cutoff = await api.resolve(
        id,
        state.parameters.retention,
        task.controller.signal,
      );
      if (!active(id, current)) return;
      await send(
        {
          job_id: crypto.randomUUID(),
          server_id: id,
          profile_version: context.profile_version,
          operation: "preview",
          scan: state.parameters.scan,
          resolved_cutoff: cutoff,
          service_policy: state.parameters.servicePolicy,
          limits: { ...state.parameters.limits },
          preview_job_id: null,
          confirmed: false,
        },
        task.controller.signal,
      );
    } finally {
      if (task.id === operationId) busy = false;
    }
  };
  const clean = async (target: ConfirmedRun) => {
    if (busy || !active(target.serverId, generation)) return;
    const task = beginOperation();
    try {
      await send(
        {
          job_id: crypto.randomUUID(),
          server_id: target.serverId,
          profile_version: target.profileVersion,
          operation: "clean",
          scan: target.scan,
          resolved_cutoff: { ...target.resolvedCutoff },
          service_policy: target.servicePolicy,
          limits: { ...target.limits },
          preview_job_id: target.previewJobId,
          confirmed: true,
        },
        task.controller.signal,
      );
    } finally {
      if (task.id === operationId) busy = false;
    }
  };
  const jobAction = async (kind: "cancel" | "reconcile") => {
    if (!view || busy) return;
    const id =
      view.job?.job_id ??
      (view.dispatch.kind === "settled" ? null : view.dispatch.intent.job_id);
    if (!id) return;
    const server = selected;
    const current = generation;
    const task = beginOperation();
    try {
      const job = await api[kind](id, task.controller.signal);
      if (active(server, current) && view) observeJob(job);
      await refresh({ owner: task.controller });
    } finally {
      if (task.id === operationId) busy = false;
    }
  };
  onMount(() => {
    const clear = () => {
      contexts.invalidate();
      generation = null;
      subscription?.();
      subscription = undefined;
      operation?.abort();
      scope.abort();
      operationId++;
      busy = false;
      view = null;
      detailsJobId = null;
      available = [];
      agents = null;
      footer?.dispose();
      footer = undefined;
      document.body.classList.remove("journal-expanded");
      status = "anonymous";
    };
    auth = mountAuth({
      onAuthorized: async (current) => {
        scope = new AbortController();
        generation = current;
        status = "loading";
        error = "";
        const signal = scope.signal;
        const list = await api.servers(signal);
        if (generation !== current || signal.aborted) return;
        available = list;
        status = "authenticated";
        footer = mountFooter((values) => {
          agents = values;
        });
        const selectedId = selectedServer(list);
        const target = selectedId
          ? list.find((server) => server.id === selectedId)
          : list[0];
        if (selectedId && !target)
          throw new Error(
            "Выбранный сервер не найден. Откройте страницу очистки заново.",
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
    const timer = window.setInterval(() => {
      if (!busy) refresh().catch(failure);
    }, REFRESH_INTERVAL_MS);
    return () => {
      clear();
      auth?.dispose();
      clearInterval(timer);
    };
  });
  $effect(() => {
    document.body.classList.toggle("journal-expanded", view?.expanded ?? false);
  });
</script>

<div class="cleanup-root private-content" class:expanded={view?.expanded}>
  {#if status === "loading"}<section class="card empty">
      Проверка сеанса…
    </section>
  {:else if status === "anonymous"}<section class="card empty">
      <p>Войдите, чтобы выбрать сервер и выполнить очистку.</p>
      <ButtonLink variant="primary" href="/login/">Войти в центр</ButtonLink>
    </section>
  {:else if status === "failed"}<section class="card">
      <div class="error" role="alert">{error || "Нет связи с центром."}</div>
      <Button onclick={() => auth?.refresh().catch(failure)}
        >Проверить соединение</Button
      >
    </section>
  {:else}
    {#if available.length === 0}<section class="card empty">
        <p>Почтовые серверы пока не добавлены.</p>
        <ButtonLink variant="primary" href="/settings/"
          >Добавить сервер</ButtonLink
        >
      </section>
    {:else}<ServerHeader
        servers={available}
        {selected}
        agent={agents?.find((agent) => agent.server_id === selected)}
        onselect={(id) => {
          choose(id).catch(failure);
        }}
        onrefresh={() => {
          refresh().catch(failure);
          footer?.refresh().catch(failure);
        }}
        disabled={busy ||
          (view?.confirmation !== null && view?.confirmation !== undefined)}
      />{/if}
    {#if error}<div class="error" role="alert">{error}</div>{/if}
    {#if view}
      {#if !view.context.configured}<div class="notice notice-warning">
          Агент добавлен. <a href={`/settings/?server=${selected}`}
            >Настройте MySQL и пути</a
          >, чтобы выполнить предпросмотр.
        </div>{/if}
      <div class="cleanup-workspace">
        <ProgressPanel
          job={view.job}
          preview={view.preview}
          dispatch={view.dispatch}
          {busy}
          canShowDetails={detailsTarget !== null}
          ondetails={openDetails}
          oncancel={() => {
            jobAction("cancel").catch(failure);
          }}
          onreconcile={() => {
            jobAction("reconcile").catch(failure);
          }}
        />
        <RunForm
          parameters={view.parameters}
          disabled={busy ||
            view.context.busy ||
            Boolean(view.job?.slot_held) ||
            view.dispatch.kind !== "settled" ||
            !view.context.configured}
          preview={view.preview}
          onparameters={(parameters) => {
            if (view)
              view = transition(view, { type: "parameters", parameters });
          }}
          onpreview={() => {
            preview().catch(failure);
          }}
          onclean={() => {
            if (view) view = transition(view, { type: "confirm" });
          }}
        />
      </div>
      <Journal
        entries={view.journal}
        expanded={view.expanded}
        onexpanded={(expanded) => {
          if (view) view = transition(view, { type: "expanded", expanded });
        }}
      />
      {#if detailsJobId && detailsImport && detailsTarget?.job_id === detailsJobId}
        {#await detailsImport}<p role="status">Загрузка подробностей…</p>
        {:then module}
          {#key detailsJobId}<module.default
              jobId={detailsJobId}
              revision={detailsTarget.revision}
              refreshVersion={filesRefreshVersion}
              ondismiss={() => (detailsJobId = null)}
            />{/key}
        {:catch}<div role="alert" class="error">
            Не удалось открыть подробности. <Button onclick={openDetails}
              >Повторить</Button
            >
          </div>{/await}
      {/if}
      {#if view.confirmation}<Confirmation
          target={view.confirmation}
          ondismiss={() => {
            if (view) view = transition(view, { type: "dismiss" });
          }}
          onconfirm={() => {
            const target = view?.confirmation;
            if (target) clean(target).catch(failure);
          }}
        />{/if}
    {:else if available.length > 0}<section class="card empty">
        Загрузка состояния сервера…
      </section>{/if}
  {/if}
</div>

<style>
  .cleanup-root {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .cleanup-workspace {
    display: grid;
    gap: 16px;
    align-items: start;
    min-width: 0;
  }
  .cleanup-workspace > :global(.card) {
    margin: 0;
    min-width: 0;
  }
  .cleanup-root > :global(.journal) {
    margin-top: 16px;
  }
  @media (min-width: 1180px) {
    .cleanup-workspace {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    }
    .cleanup-workspace > :global(.run-config) {
      grid-column: 1;
      grid-row: 1;
    }
    .cleanup-workspace > :global(.progress-panel) {
      grid-column: 2;
      grid-row: 1;
    }
  }
  :global(body.journal-expanded .sidebar),
  :global(body.journal-expanded .topbar),
  :global(body.journal-expanded .page-heading),
  :global(body.journal-expanded .run-config) {
    display: none;
  }
  :global(body.journal-expanded .app-shell) {
    grid-template-columns: 1fr;
  }
  :global(body.journal-expanded .workspace) {
    grid-column: 1;
    height: 100dvh;
    min-height: 0;
  }
  :global(body.journal-expanded .main) {
    padding: 16px 24px;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  :global(body.journal-expanded .footer) {
    flex-shrink: 0;
  }
  .cleanup-root.expanded {
    flex: 1;
    min-height: 0;
  }
  .cleanup-root.expanded .cleanup-workspace {
    display: block;
    flex-shrink: 0;
  }
  :global(body.journal-expanded .progress-panel) {
    padding: 16px;
    margin-top: 0;
  }
  :global(body.journal-expanded .preview-summary),
  :global(body.journal-expanded .preview-meta),
  :global(body.journal-expanded .metrics) {
    display: none;
  }
  :global(body.journal-expanded .journal) {
    flex: 1;
    min-height: 0;
    margin-top: 12px;
  }
  :global(body.journal-expanded .journal .log) {
    flex: 1;
    min-height: 0;
    max-height: none;
  }
  :global(body.journal-expanded .server-header) {
    margin-bottom: 12px;
    flex-shrink: 0;
  }
  :global(body.journal-expanded .progress-panel) {
    flex-shrink: 0;
  }
  @media (max-width: 540px) {
    :global(body.journal-expanded .main) {
      padding: 12px;
    }
  }
</style>
