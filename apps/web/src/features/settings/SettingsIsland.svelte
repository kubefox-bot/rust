<script lang="ts">
  import { onMount } from "svelte";
  import type { BrowserContracts } from "../../api";
  import { ApiError } from "../../api";
  import { mountAuth } from "../../shell/auth";
  import { mountFooter } from "../../shell/footer";
  import { selectedServer, selectServer } from "../../shell/navigation";
  import ServerHeader from "../../shell/ServerHeader.svelte";
  import { SECTIONS } from "./conts";
  import type { SettingsSection } from "./types";
  import * as api from "./api";
  import Profile from "./Profile.svelte";
  import Agents from "./Agents.svelte";
  import Account from "./Account.svelte";
  let status = $state<"loading" | "authenticated" | "anonymous" | "failed">(
    "loading",
  );
  let available = $state<BrowserContracts["servers"]>([]);
  let agents = $state<BrowserContracts["agents"] | null>(null);
  let selected = $state("");
  let context = $state<BrowserContracts["cleanup_context"] | null>(null);
  let section = $state<SettingsSection>("profile");
  let error = $state("");
  let generation: string | null = null;
  let scope = new AbortController();
  let targetScope = new AbortController();
  let auth: ReturnType<typeof mountAuth> | undefined = undefined;
  let footer: ReturnType<typeof mountFooter> | undefined = undefined;
  const fail = (cause: unknown) => {
    if (cause instanceof DOMException && cause.name === "AbortError") return;
    error = cause instanceof Error ? cause.message : "Операция не выполнена.";
    if (cause instanceof ApiError && cause.status === 401)
      auth?.refresh().catch(() => {
        status = "failed";
      });
  };
  const refresh = async () => {
    if (!selected || status !== "authenticated") return;
    const id = selected;
    const current = generation;
    const signal = targetScope.signal;
    const value = await api.context(id, signal);
    if (!signal.aborted && selected === id && current === generation)
      context = value;
    await footer?.refresh();
  };
  const choose = async (id: string) => {
    if (selected === id && context) return;
    targetScope.abort();
    targetScope = new AbortController();
    selected = id;
    context = null;
    error = "";
    selectServer(id);
    await refresh();
  };
  const navigate = (value: SettingsSection) => {
    section = value;
    window.location.hash = value;
  };
  const readHash = () => {
    section =
      SECTIONS.find(([id]) => `#${id}` === window.location.hash)?.[0] ??
      "profile";
  };
  const reload = async () => {
    const current = generation;
    const signal = scope.signal;
    const list = await api.servers(signal);
    if (signal.aborted || current !== generation) return;
    available = list;
    if (selected && list.some((server) => server.id === selected)) {
      await refresh();
      return;
    }
    const fromUrl = selectedServer();
    const target = fromUrl
      ? list.find((server) => server.id === fromUrl)
      : list[0];
    if (fromUrl && !target)
      throw new Error("Выбранный сервер не найден. Выберите сервер из списка.");
    if (target) await choose(target.id);
  };
  const refreshAll = () => {
    reload().catch(fail);
  };
  onMount(() => {
    readHash();
    const clear = () => {
      generation = null;
      scope.abort();
      targetScope.abort();
      available = [];
      selected = "";
      context = null;
      agents = null;
      footer?.dispose();
      footer = undefined;
      status = "anonymous";
    };
    auth = mountAuth({
      onAuthorized: async (current) => {
        scope = new AbortController();
        targetScope = new AbortController();
        generation = current;
        status = "authenticated";
        error = "";
        footer = mountFooter((values) => {
          agents = values;
        });
        await reload();
      },
      onAnonymous: clear,
      onFailure: (cause) => {
        clear();
        status = "failed";
        fail(cause);
      },
    });
    window.addEventListener("hashchange", readHash);
    return () => {
      clear();
      auth?.dispose();
      window.removeEventListener("hashchange", readHash);
    };
  });
</script>

<div class="private-content">
  {#if status === "loading"}<section class="card empty">
      Проверка сеанса…
    </section>
  {:else if status === "anonymous"}<section class="card empty">
      <p>Войдите для настройки центра.</p>
      <a class="button primary" href="/login/">Войти в центр</a>
    </section>
  {:else if status === "failed"}<section class="card">
      <div class="error" role="alert">{error || "Нет связи с центром."}</div>
      <button class="button" onclick={() => auth?.refresh().catch(fail)}
        >Проверить соединение</button
      >
    </section>
  {:else}
    <div class="page-heading">
      <div>
        <h1>Настройки</h1>
        <p class="subtitle">Серверы, агенты и учётная запись центра.</p>
      </div>
      <button class="button" onclick={() => navigate("agents")}
        >Добавить сервер</button
      >
    </div>
    {#if available.length}<ServerHeader
        servers={available}
        {selected}
        agent={agents?.find((agent) => agent.server_id === selected)}
        onselect={(id) => choose(id).catch(fail)}
        onrefresh={refreshAll}
      />{/if}
    <nav class="settings-tabs" aria-label="Разделы настроек">
      {#each SECTIONS as [id, label]}<button
          class:active={section === id}
          class="button quiet"
          aria-current={section === id ? "page" : undefined}
          onclick={() => navigate(id)}>{label}</button
        >{/each}
    </nav>
    {#if error}<div class="error" role="alert">{error}</div>{/if}
    {#if section === "account"}<Account
        oncommitted={() => window.location.replace("/login/")}
      />
    {:else if section === "agents"}{#key selected}<Agents
          servers={available}
          {selected}
          {agents}
          configured={context?.configured}
          onrefresh={refreshAll}
        />{/key}
    {:else if selected}{#key selected}<Profile
          serverId={selected}
          name={available.find((server) => server.id === selected)?.name ??
            "Сервер"}
          onrefresh={refreshAll}
        />{/key}
    {:else}<section class="card empty">
        Добавьте первый почтовый сервер.
      </section>{/if}
  {/if}
</div>

<style>
  .settings-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    border-bottom: 1px solid var(--line);
    padding-bottom: 15px;
    margin-bottom: 24px;
  }
  .settings-tabs .active {
    background: #eaf0ff;
    border-color: #cbd9fb;
    color: var(--accent);
  }
</style>
