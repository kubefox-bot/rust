<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    install,
    update,
    probe,
    verifiedOperation,
    message,
  } from "./agents/api";
  import { agentEndpoint, sshTarget } from "./agents/validation";
  import { resolveSubmissionFailure } from "./agents/admission";
  import { SSH_PORT, AGENT_PORT, MAX_PORT, MODE_LABELS } from "./agents/conts";
  import type {
    EnrollmentMode,
    EnrollmentAttempt,
    WizardState,
  } from "./agents/types";
  import type { BrowserContracts } from "../../api";
  import Credentials from "./agents/Credentials.svelte";
  import OperationStatus from "./agents/OperationStatus.svelte";
  let {
    mode,
    packages,
    configured = false,
    onrefresh,
    onlock,
  }: {
    mode: EnrollmentMode;
    packages: BrowserContracts["packages"];
    configured?: boolean;
    onrefresh: () => void;
    onlock: (locked: boolean) => void;
  } = $props();
  let wizard = $state<WizardState>({ kind: "target" });
  let host = $state("");
  let port = $state<number | undefined>(SSH_PORT);
  let name = $state("Новый сервер");
  let endpoint = $state("");
  let packageVersion = $state("");
  let acknowledged = $state(false);
  let probing = $state(false);
  let error = $state("");
  const controller = new AbortController();
  const title = $derived(MODE_LABELS[mode.kind]);
  $effect(() => {
    if (!packageVersion && packages[0]) packageVersion = packages[0].build;
  });
  onDestroy(() => {
    controller.abort();
  });
  const reset = () => {
    wizard = { kind: "target" };
    acknowledged = false;
    error = "";
  };
  const checkHost = async (event: SubmitEvent) => {
    event.preventDefault();
    if (probing || wizard.kind !== "target") return;
    error = "";
    probing = true;
    acknowledged = false;
    try {
      const target = sshTarget({ host, port });
      const challenge = await probe({ target, signal: controller.signal });
      if (controller.signal.aborted) return;
      if (
        challenge.target.host !== target.host ||
        challenge.target.port !== target.port
      ) {
        error =
          "Ответ относится к другому SSH-серверу. Проверьте адрес и повторите проверку ключа.";
        return;
      }
      if (!endpoint)
        endpoint = `https://${host.includes(":") ? `[${host}]` : host}:${AGENT_PORT}`;
      wizard = { kind: "trust", challenge };
    } catch (failure) {
      if (!controller.signal.aborted) error = message(failure);
    } finally {
      probing = false;
    }
  };
  const continueToCredentials = () => {
    if (wizard.kind !== "trust" || !acknowledged) return;
    if (mode.kind === "install") {
      if (
        !name.trim() ||
        name.length > 128 ||
        /[\u0000-\u001f\u007f]/.test(name)
      ) {
        error =
          "Укажите название сервера длиной до 128 символов. Вернитесь к адресу сервера.";
        return;
      }
      try {
        endpoint = agentEndpoint({
          value: endpoint,
          host: wizard.challenge.target.host,
        });
      } catch {
        error =
          "Адрес API должен использовать HTTPS и тот же хост, что SSH, без пути и реквизитов. Вернитесь к адресу сервера.";
        return;
      }
    }
    error = "";
    wizard = { kind: "credentials", challenge: wizard.challenge };
  };
  const submit = async (
    credentials: BrowserContracts["install"]["credentials"],
  ) => {
    if (wizard.kind !== "credentials" || controller.signal.aborted) return;
    const challenge = wizard.challenge;
    const selectedPackage = packages.find(
      (candidate) => candidate.build === packageVersion,
    );
    if (!selectedPackage) {
      error = "Выберите доступную версию агента.";
      return;
    }
    const server =
      mode.kind === "install"
        ? {
            agent_id: crypto.randomUUID(),
            id: crypto.randomUUID(),
            profile_id: crypto.randomUUID(),
            name: name.trim(),
          }
        : mode.server;
    const attempt: EnrollmentAttempt = {
      operationId: crypto.randomUUID(),
      agentId: server.agent_id,
      serverId: server.id,
      profileId: server.profile_id,
      name: server.name,
      fingerprint: challenge.fingerprint,
      packageVersion: selectedPackage.build,
      expectedSha256: selectedPackage.sha256,
    };
    wizard = { kind: "submitting", attempt };
    error = "";
    onlock(true);
    try {
      const view = await (mode.kind === "install"
        ? install({
            body: {
              operation_id: attempt.operationId,
              agent_id: attempt.agentId,
              server_id: attempt.serverId,
              profile_id: attempt.profileId,
              name: attempt.name,
              endpoint,
              target: challenge.target,
              fingerprint: attempt.fingerprint,
              credentials,
              package_version: attempt.packageVersion,
              legacy: null,
            },
            signal: controller.signal,
          })
        : update({
            agentId: attempt.agentId,
            body: {
              mode: mode.kind,
              operation_id: attempt.operationId,
              target: challenge.target,
              fingerprint: attempt.fingerprint,
              credentials,
              package_version: attempt.packageVersion,
            },
            signal: controller.signal,
          }));
      if (!controller.signal.aborted)
        wizard = {
          kind: "operation",
          attempt,
          view: verifiedOperation({ attempt, view }),
        };
    } catch (failure) {
      const outcome = await resolveSubmissionFailure({
        error: failure,
        attempt,
        signal: controller.signal,
      });
      if (controller.signal.aborted) return;
      switch (outcome.kind) {
        case "rejected":
          wizard = { kind: "credentials", challenge };
          error = outcome.message;
          onlock(false);
          break;
        case "accepted":
          wizard = { kind: "operation", attempt, view: outcome.view };
          break;
        case "unknown":
          wizard = { kind: "operation", attempt, view: null };
          break;
        default: {
          const unreachable: never = outcome;
          throw new Error(`Unexpected admission outcome: ${unreachable}`);
        }
      }
    }
  };
</script>

<section class="card" aria-label={title.title}>
  <h2>{title.title}</h2>
  {#if mode.kind !== "install"}<p class="target">
      Целевой сервер: <strong>{mode.server.name}</strong>
    </p>{/if}
  {#if wizard.kind === "operation"}
    <OperationStatus
      attempt={wizard.attempt}
      initial={wizard.view}
      {configured}
      {onrefresh}
      {onlock}
    />
  {:else if wizard.kind === "submitting"}
    <p role="status">
      Отправка запроса для сервера <strong>{wizard.attempt.name}</strong>…
    </p>
  {:else}
    <p class="muted intro">
      SSH-сервер должен быть подготовлен заранее. Сверьте ключ хоста по
      независимому каналу перед вводом реквизитов ОС.
    </p>
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <div class="field-grid details">
      {#if mode.kind === "install"}
        <label class="field"
          >Название сервера<input
            bind:value={name}
            disabled={wizard.kind !== "target" || probing}
            maxlength="128"
            placeholder="Почтовый сервер"
          /></label
        >
        <label class="field"
          >Адрес API агента<input
            type="url"
            bind:value={endpoint}
            disabled={wizard.kind !== "target" || probing}
            placeholder="https://mail.example:9443"
          /><small>HTTPS на том же хосте, что SSH. Порт можно изменить.</small
          ></label
        >
      {/if}
      <label class="field"
        >Версия агента<select
          bind:value={packageVersion}
          disabled={wizard.kind !== "target" ||
            probing ||
            packages.length === 0}
          >{#each packages as candidate}<option value={candidate.build}
              >{candidate.build}</option
            >{/each}</select
        ></label
      >
    </div>
    {#if packages.length === 0}<p class="notice">
        Доверенные пакеты пока недоступны. Добавьте совместимый пакет в поставку
        центра.
      </p>{/if}
    {#if wizard.kind === "target"}
      <form onsubmit={checkHost}>
        <div class="field-grid">
          <label class="field"
            >Хост SSH<input
              bind:value={host}
              required
              disabled={probing}
              maxlength="253"
              placeholder="mail.example"
              autocomplete="off"
              spellcheck="false"
            /></label
          >
          <label class="field"
            >Порт SSH<input
              type="number"
              bind:value={port}
              required
              min="1"
              max={MAX_PORT}
              step="1"
              disabled={probing}
            /></label
          >
        </div>
        <div class="actions">
          <button type="submit" class="button primary" disabled={probing}
            >{probing ? "Проверка ключа…" : "Проверить SSH-ключ"}</button
          >
        </div>
      </form>
    {:else if wizard.kind === "trust"}
      <p class="target">
        SSH-сервер: <strong
          >{wizard.challenge.target.host}:{wizard.challenge.target.port}</strong
        >
      </p>
      <p class="fingerprint"><code>{wizard.challenge.fingerprint}</code></p>
      <label class="check"
        ><input type="checkbox" bind:checked={acknowledged} />Я сверил SSH-ключ
        сервера</label
      >
      <div class="actions">
        <button type="button" class="button" onclick={reset}
          >Изменить SSH-сервер</button
        ><button
          type="button"
          class="button primary"
          disabled={!acknowledged}
          onclick={continueToCredentials}>Продолжить</button
        >
      </div>
    {:else if wizard.kind === "credentials"}
      <p class="target">
        Проверенный SSH-сервер: <strong
          >{wizard.challenge.target.host}:{wizard.challenge.target.port}</strong
        >
      </p>
      <Credentials
        submitLabel={title.submit}
        onsubmit={submit}
        onback={reset}
      />
    {/if}
  {/if}
</section>

<style>
  .intro,
  .target {
    font-size: 12px;
  }
  .details {
    margin: 20px 0;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: 12px;
  }
  .fingerprint {
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .fingerprint code {
    overflow-wrap: anywhere;
  }
</style>
