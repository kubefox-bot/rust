<script lang="ts">
  import { Button } from "../../ui/button";
  import { Input } from "../../ui/input";
  import { Select } from "../../ui/select";
  import { Checkbox } from "../../ui/checkbox";
  import { onMount } from "svelte";
  import type { Profile } from "./types";
  import { MYSQL_PORT_MAX, MYSQL_TLS_OPTIONS } from "./conts";
  import * as api from "./api";
  let {
    serverId,
    name,
    onrefresh,
  }: { serverId: string; name: string; onrefresh: () => void } = $props();
  let current = $state<Profile | null>(null);
  let password = $state("");
  let replace = $state(false);
  let loading = $state(true);
  let busy = $state(false);
  let error = $state("");
  let notice = $state("");
  const scope = new AbortController();
  const fail = (cause: unknown) => {
    if (!scope.signal.aborted)
      error = cause instanceof Error ? cause.message : "Операция не выполнена.";
  };
  const load = async () => {
    loading = true;
    error = "";
    notice = "";
    password = "";
    try {
      const value = await api.profile(serverId, scope.signal);
      if (!scope.signal.aborted) {
        current = value;
        replace = !value.password_set;
      }
    } finally {
      if (!scope.signal.aborted) loading = false;
    }
  };
  const save = async (event: SubmitEvent) => {
    event.preventDefault();
    if (!current || busy) return;
    busy = true;
    error = "";
    notice = "";
    const target = current;
    try {
      await api.save(
        serverId,
        {
          expected_version: target.version,
          profile: {
            mysql_host: target.mysql_host,
            mysql_port: target.mysql_port,
            mysql_database: target.mysql_database,
            mysql_username: target.mysql_username,
            mysql_tls: target.mysql_tls,
            data_dir: target.data_dir,
            hmail_ini: target.hmail_ini,
            service_name: target.service_name,
            password: replace
              ? { kind: "replace", value: password }
              : { kind: "keep" },
          },
        },
        scope.signal,
      );
      if (scope.signal.aborted) return;
      password = "";
      await load();
      if (!scope.signal.aborted) {
        notice = "Профиль сохранён";
        onrefresh();
      }
    } catch (cause) {
      fail(cause);
    } finally {
      password = "";
      if (!scope.signal.aborted) busy = false;
    }
  };
  const validate = async () => {
    if (busy || !current?.configured) return;
    busy = true;
    error = "";
    notice = "";
    try {
      await api.validate(serverId, scope.signal);
      if (!scope.signal.aborted)
        notice = "Подключение к агенту и профиль проверены";
    } catch (cause) {
      fail(cause);
    } finally {
      if (!scope.signal.aborted) busy = false;
    }
  };
  onMount(() => {
    load().catch(fail);
    return () => {
      scope.abort();
      password = "";
      current = null;
    };
  });
</script>

<section class="card">
  <h2>Технический профиль · {name}</h2>
  <p class="subtitle">MySQL и пути на выбранном почтовом сервере.</p>
  {#if error}<div class="error" role="alert">
      {error}
      <Button variant="link" onclick={() => load().catch(fail)}
        >Перечитать профиль</Button
      >
    </div>{/if}
  {#if notice}<div class="notice" role="status">{notice}</div>{/if}
  {#if loading}<div class="empty">Загрузка профиля…</div>
  {:else if current}
    {#if !current.configured}<div class="notice">Требуется настройка</div>{/if}
    <form onsubmit={save}>
      <fieldset disabled={busy}>
        <div class="field-grid">
          <label class="field"
            >Хост MySQL<Input
              aria-label="Хост MySQL"
              bind:value={current.mysql_host}
              required
              maxlength={253}
            /></label
          >
          <label class="field"
            >Порт MySQL<Input
              aria-label="Порт MySQL"
              type="number"
              bind:value={current.mysql_port}
              min="1"
              max={MYSQL_PORT_MAX}
              required
            /></label
          >
          <label class="field"
            >База MySQL<Input
              aria-label="База MySQL"
              bind:value={current.mysql_database}
              required
              maxlength={128}
            /></label
          >
          <label class="field"
            >Пользователь MySQL<Input
              aria-label="Пользователь MySQL"
              bind:value={current.mysql_username}
              required
              maxlength={128}
              autocomplete="off"
            /></label
          >
          <Select
            label="Шифрование MySQL"
            options={MYSQL_TLS_OPTIONS}
            bind:value={current.mysql_tls}
            disabled={busy}
          />
          <div class="password">
            {#if current.password_set}<Checkbox
                label="Заменить пароль MySQL"
                bind:checked={replace}
                onchange={() => (password = "")}
              />{/if}
            <label class="field"
              >Пароль MySQL<Input
                aria-label="Пароль MySQL"
                type="password"
                bind:value={password}
                disabled={!replace}
                required={replace}
                autocomplete="new-password"
                maxlength={4096}
              /><small
                >{replace
                  ? "Введите новый пароль."
                  : "Сохранённый пароль останется прежним."}</small
              ></label
            >
          </div>
          <label class="field"
            >Каталог почты<Input
              aria-label="Каталог почты"
              bind:value={current.data_dir}
              required
              maxlength={32767}
            /></label
          >
          <label class="field"
            >Файл hMailServer.ini<Input
              aria-label="Файл hMailServer.ini"
              bind:value={current.hmail_ini}
              required
              maxlength={32767}
            /></label
          >
          <label class="field"
            >Имя службы<Input
              aria-label="Имя службы"
              bind:value={current.service_name}
              required
              maxlength={256}
            /></label
          >
        </div>
        <div class="actions">
          <Button variant="primary" type="submit">Сохранить профиль</Button
          ><Button
            type="button"
            disabled={!current.configured}
            onclick={validate}>Проверить подключение</Button
          ><span class="muted">Версия {current.version}</span>
        </div>
      </fieldset>
    </form>
  {/if}
</section>

<style>
  fieldset {
    border: 0;
    padding: 0;
    margin: 22px 0 0;
    min-width: 0;
  }
  .password > :global(.checkbox-field) {
    margin-bottom: 10px;
  }
  .password {
    min-width: 0;
  }
  .actions .muted {
    font-size: 11px;
  }
</style>
