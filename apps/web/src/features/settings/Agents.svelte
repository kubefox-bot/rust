<script lang="ts">
  import AgentStatus from "../../shell/AgentStatus.svelte";
  import { Button } from "../../ui/button";
  import { onMount } from "svelte";
  import { formatDateTime } from "../../lib/time";
  import type { BrowserContracts } from "../../api";
  import Enrollment from "./Enrollment.svelte";
  import OperationStatus from "./agents/OperationStatus.svelte";
  import ActionConfirmation from "./agents/ActionConfirmation.svelte";
  import { recent, renew, revoke, message } from "./agents/api";
  import { isNewerAgentBuild } from "../../agents";
  import { ENROLLMENT_PHASES } from "./agents/conts";
  import type {
    AgentAction,
    EnrollmentAttempt,
    EnrollmentMode,
    Enrollment as EnrollmentView,
  } from "./agents/types";
  let {
    servers,
    selected,
    agents,
    configured,
    onrefresh,
    readPackages,
  }: {
    servers: BrowserContracts["servers"];
    selected: string;
    agents: BrowserContracts["agents"] | null;
    configured: boolean | undefined;
    onrefresh: () => void;
    readPackages: () => Promise<BrowserContracts["packages"]>;
  } = $props();
  type Resources =
    | Readonly<{ kind: "loading" }>
    | Readonly<{
        kind: "ready";
        packages: BrowserContracts["packages"];
        operations: BrowserContracts["enrollments"];
      }>
    | Readonly<{ kind: "error"; message: string }>;
  type CurrentView =
    | Readonly<{ kind: "wizard"; mode: EnrollmentMode; revision: number }>
    | Readonly<{
        kind: "operation";
        attempt: EnrollmentAttempt;
        view: EnrollmentView;
      }>;
  let resources = $state<Resources>({ kind: "loading" });
  let current = $state<CurrentView>({
    kind: "wizard",
    mode: { kind: "install" },
    revision: 0,
  });
  let locked = $state(false);
  let action = $state<AgentAction | null>(null);
  let actionPending = $state(false);
  let error = $state("");
  let notice = $state("");
  const controller = new AbortController();
  const server = $derived(
    servers.find((candidate) => candidate.id === selected),
  );
  const agent = $derived(
    agents?.find(
      (candidate) =>
        candidate.server_id === selected &&
        candidate.agent_id === server?.agent_id,
    ),
  );
  const updatePackages = $derived(
    resources.kind === "ready" && agent?.health.version
      ? resources.packages.filter((candidate) =>
          isNewerAgentBuild({
            current: agent.health.version ?? "",
            available: candidate.build,
          }),
        )
      : [],
  );
  const load = async () => {
    resources = { kind: "loading" };
    try {
      const [available, operations] = await Promise.all([
        readPackages(),
        recent(controller.signal),
      ]);
      if (!controller.signal.aborted) {
        resources = { kind: "ready", packages: available, operations };
        const unresolved = operations.find(
          (operation) =>
            (operation.server_id === selected ||
              !servers.some(
                (candidate) => candidate.id === operation.server_id,
              )) &&
            (operation.slot_held || operation.phase === "unknown"),
        );
        if (unresolved) inspect(unresolved);
      }
    } catch (failure) {
      if (!controller.signal.aborted)
        resources = { kind: "error", message: message(failure) };
    }
  };
  onMount(() => {
    load();
    return () => controller.abort();
  });
  const chooseMode = (mode: EnrollmentMode) => {
    if (locked || actionPending) return;
    current = {
      kind: "wizard",
      mode,
      revision: current.kind === "wizard" ? current.revision + 1 : 0,
    };
    error = "";
    notice = "";
  };
  $effect(() => {
    if (
      resources.kind !== "ready" ||
      !server ||
      !agent ||
      current.kind !== "wizard" ||
      locked ||
      actionPending
    )
      return;
    const url = new URL(window.location.href);
    if (url.searchParams.get("action") !== "update-agent") return;
    url.searchParams.delete("action");
    window.history.replaceState(null, "", url);
    if (!agent.revoked && !agent.certificate_expired && updatePackages.length)
      chooseMode({ kind: "binary", server });
  });
  const inspect = (view: EnrollmentView) => {
    if (locked || actionPending) return;
    const target = servers.find(
      (candidate) =>
        candidate.id === view.server_id && candidate.agent_id === view.agent_id,
    );
    current = {
      kind: "operation",
      view,
      attempt: {
        operationId: view.id,
        agentId: view.agent_id,
        serverId: view.server_id,
        profileId: view.profile_id,
        name: target?.name ?? `Сервер ${view.server_id}`,
        fingerprint: view.fingerprint,
        packageVersion: view.package_version,
        expectedSha256: view.expected_sha256,
      },
    };
    error = "";
    notice = "";
  };
  const perform = async (confirmed: AgentAction) => {
    if (
      actionPending ||
      locked ||
      selected !== confirmed.server.id ||
      server?.agent_id !== confirmed.server.agent_id ||
      controller.signal.aborted
    )
      return;
    actionPending = true;
    error = "";
    notice = "";
    try {
      if (confirmed.kind === "renew") {
        const receipt = await renew({
          agentId: confirmed.server.agent_id,
          signal: controller.signal,
        });
        if (receipt.agent_id !== confirmed.server.agent_id)
          throw new Error("Agent receipt target mismatch");
      } else {
        await revoke({
          agentId: confirmed.server.agent_id,
          signal: controller.signal,
        });
      }
      if (!controller.signal.aborted && selected === confirmed.server.id) {
        notice =
          confirmed.kind === "renew"
            ? "Сертификат агента обновлён."
            : "Доступ агента отозван.";
        onrefresh();
      }
    } catch (failure) {
      if (!controller.signal.aborted) error = message(failure);
    } finally {
      actionPending = false;
      action = null;
    }
  };
</script>

<section class="card" aria-label="Агенты серверов">
  <div class="heading">
    <h2>Агенты серверов</h2>
    <Button
      type="button"
      disabled={locked || actionPending}
      onclick={() => chooseMode({ kind: "install" })}>Новое подключение</Button
    >
  </div>
  {#if server}
    <p class="target">Выбранный сервер: <strong>{server.name}</strong></p>
    {#if agent}
      <div class="badges">
        <AgentStatus {agent} />
        {#if configured === false}<span class="badge">Требуется настройка</span
          >{/if}
        {#if agent.certificate_expired}<span class="badge"
            >Сертификат истёк</span
          >{:else if agent.certificate_expiring}<span class="badge"
            >Срок сертификата истекает</span
          >{/if}
      </div>
      <div class="version-row">
        <span class="muted"
          >Версия агента: {agent.health.version ?? "неизвестна"}</span
        >
        {#if updatePackages.length > 0}
          <Button
            type="button"
            disabled={locked ||
              actionPending ||
              agent.revoked ||
              agent.certificate_expired}
            onclick={() => chooseMode({ kind: "binary", server })}
            variant="primary">Обновить агента</Button
          >
        {/if}
      </div>
      <p class="muted">
        Состояние получено:
        {#if agent.health.observed_at}<time datetime={agent.health.observed_at}
            >{formatDateTime(agent.health.observed_at)}</time
          >{:else}ещё нет данных{/if}
      </p>
      <details class="connection-security">
        <summary>Действия с агентом</summary>
        {#if agent.certificate_expires_at}<p class="muted">
            Сертификат действует до <time
              datetime={agent.certificate_expires_at}
              >{formatDateTime(agent.certificate_expires_at)}</time
            >
          </p>{/if}
        <div class="actions">
          <Button
            type="button"
            disabled={locked || actionPending || agent.revoked}
            onclick={() => chooseMode({ kind: "recover_identity", server })}
            variant="warning">Восстановить доверие</Button
          >
          <Button
            type="button"
            disabled={locked ||
              actionPending ||
              agent.revoked ||
              agent.certificate_expired}
            onclick={() => {
              action = { kind: "renew", server };
            }}
            variant="success">Обновить сертификат</Button
          >
          <Button
            type="button"
            disabled={locked || actionPending || agent.revoked}
            onclick={() => {
              action = { kind: "revoke", server };
            }}
            variant="danger">Отозвать доступ</Button
          >
        </div>
      </details>
    {:else if agents === null}<p class="notice">
        Состояние агентов недоступно. Обновите настройки, чтобы повторить
        запрос.
      </p>{:else}<p class="notice">
        Агент выбранного сервера отсутствует в ответе центра.
      </p>{/if}
  {:else}<p class="muted">
      Добавьте первый сервер через проверенное SSH-подключение.
    </p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}
</section>

{#if resources.kind === "loading"}<p role="status">
    Загрузка пакетов и подключений…
  </p>
{:else if resources.kind === "error"}<section class="card">
    <p class="error" role="alert">{resources.message}</p>
    <Button type="button" onclick={load}>Повторить загрузку подключений</Button>
  </section>
{:else}
  {#if current.kind === "wizard"}
    {#key current.revision}<Enrollment
        mode={current.mode}
        packages={current.mode.kind === "binary"
          ? updatePackages
          : resources.packages}
        configured={current.mode.kind !== "install" && configured === true}
        {onrefresh}
        onlock={(value) => {
          locked = value;
        }}
      />{/key}
  {:else}
    {#key current.attempt.operationId}<section class="card">
        <OperationStatus
          attempt={current.attempt}
          initial={current.view}
          configured={current.view.server_id === selected &&
            configured === true}
          {onrefresh}
          onlock={(value) => {
            locked = value;
          }}
        />
      </section>{/key}
  {/if}
  {#if resources.operations.length > 0}
    <section class="card" aria-label="Недавние подключения">
      <h2>Недавние подключения</h2>
      <ul>
        {#each resources.operations as operation (operation.id)}<li>
            <span
              ><strong
                >{servers.find(
                  (candidate) => candidate.id === operation.server_id,
                )?.name ?? `Сервер ${operation.server_id}`}</strong
              >
              · {ENROLLMENT_PHASES[operation.phase]}</span
            ><Button
              type="button"
              disabled={locked || actionPending}
              onclick={() => inspect(operation)}>Проверить операцию</Button
            >
          </li>{/each}
      </ul>
    </section>
  {/if}
{/if}
{#if action}
  {@const confirmed = action}
  <ActionConfirmation
    action={confirmed}
    pending={actionPending}
    onconfirm={() => perform(confirmed)}
    onclose={() => {
      if (!actionPending) action = null;
    }}
  />
{/if}

<style>
  .version-row {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    margin-top: 16px;
    font-size: 12px;
  }
  .connection-security {
    border-top: 1px solid var(--line);
    margin-top: 20px;
    padding-top: 16px;
  }
  .connection-security summary {
    cursor: pointer;
    font-size: 12px;
    font-weight: 600;
  }
  .connection-security .actions {
    margin-top: 16px;
  }
  .heading {
    display: flex;
    gap: 16px;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
  }
  .heading h2 {
    margin: 0;
  }
  .target,
  p {
    font-size: 12px;
  }
  .badges {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  @media (max-width: 600px) {
    li {
      align-items: flex-start;
      flex-direction: column;
    }
  }
</style>
