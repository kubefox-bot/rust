<script lang="ts">
  import { Button } from "../../../ui/button";
  import { Input } from "../../../ui/input";
  import { Select } from "../../../ui/select";
  import { SSH_METHOD_OPTIONS } from "./conts";
  import { onDestroy } from "svelte";
  import type { BrowserContracts } from "../../../api";
  let {
    submitLabel,
    onsubmit,
    onback,
  }: {
    submitLabel: string;
    onsubmit: (
      credentials: BrowserContracts["install"]["credentials"],
    ) => Promise<void>;
    onback: () => void;
  } = $props();
  let method = $state<"password" | "key">("password");
  let username = $state("");
  let password = $state("");
  let privateKey = $state("");
  let passphrase = $state("");
  const clearSecrets = () => {
    password = "";
    privateKey = "";
    passphrase = "";
  };
  onDestroy(() => {
    clearSecrets();
    username = "";
  });
  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    const credentials: BrowserContracts["install"]["credentials"] =
      method === "password"
        ? { method, username, password }
        : {
            method,
            username,
            private_key: privateKey,
            passphrase: passphrase || null,
          };
    clearSecrets();
    username = "";
    await onsubmit(credentials);
  };
</script>

<form onsubmit={submit} autocomplete="off">
  <p class="notice">
    Реквизиты ОС используются только для этой операции. После отправки поля
    очищаются.
  </p>
  <div class="field-grid">
    <label class="field"
      >Пользователь SSH<Input
        bind:value={username}
        required
        maxlength={256}
        autocomplete="off"
        spellcheck="false"
      /></label
    >
    <Select
      label="Способ входа SSH"
      options={SSH_METHOD_OPTIONS}
      bind:value={method}
      onchange={clearSecrets}
    />
    {#if method === "password"}
      <label class="field"
        >Пароль SSH<Input
          type="password"
          bind:value={password}
          required
          maxlength={16384}
          autocomplete="off"
        /></label
      >
    {:else}
      <label class="field key"
        >Закрытый ключ SSH<textarea
          bind:value={privateKey}
          required
          maxlength="65536"
          rows="6"
          autocomplete="off"
          spellcheck="false"></textarea></label
      >
      <label class="field"
        >Пароль ключа SSH<Input
          type="password"
          bind:value={passphrase}
          maxlength={4096}
          autocomplete="off"
        /></label
      >
    {/if}
  </div>
  <div class="actions">
    <Button type="button" onclick={onback}>Изменить SSH-сервер</Button><Button
      type="submit"
      variant="primary">{submitLabel}</Button
    >
  </div>
</form>

<style>
  .key {
    grid-column: 1 / -1;
  }
</style>
