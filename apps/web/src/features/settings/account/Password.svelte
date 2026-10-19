<script lang="ts">
  import { Button } from "../../../ui/button";
  import { Input } from "../../../ui/input";
  import { onDestroy } from "svelte";
  import { changePassword } from "./api";
  import { MIN_PASSWORD_CHARACTERS, MAX_PASSWORD_INPUT_LENGTH } from "./conts";
  import { failureMessage } from "./failure-message";

  let { oncommitted }: { oncommitted: () => void } = $props();
  let currentPassword = $state("");
  let newPassword = $state("");
  let confirmation = $state("");
  let pending = $state(false);
  let error = $state("");
  let active = true;
  const clearSecrets = () => {
    currentPassword = "";
    newPassword = "";
    confirmation = "";
  };
  onDestroy(() => {
    active = false;
    clearSecrets();
  });
  const save = async (event: SubmitEvent) => {
    event.preventDefault();
    if (pending) return;
    error = "";
    if (newPassword !== confirmation) {
      error = "Пароли не совпадают.";
      clearSecrets();
      return;
    }
    pending = true;
    try {
      const operation = changePassword({
        current_password: currentPassword,
        new_password: newPassword,
        confirm_password: confirmation,
      });
      clearSecrets();
      await operation;
      if (active) oncommitted();
    } catch (failure) {
      if (active)
        error = failureMessage(
          failure,
          "Смена пароля не подтверждена. Проверьте обычный вход перед повтором.",
        );
    } finally {
      if (active) {
        clearSecrets();
        pending = false;
      }
    }
  };
</script>

<section class="card" aria-labelledby="password-heading">
  <h2 id="password-heading">Смена пароля</h2>
  <p class="subtitle">
    После сохранения потребуется обычный вход новым паролем. Новый пароль — не
    менее {MIN_PASSWORD_CHARACTERS} символов.
  </p>
  <form onsubmit={save} autocomplete="off">
    <label class="field"
      >Текущий пароль<Input
        type="password"
        autocomplete="current-password"
        required
        maxlength={MAX_PASSWORD_INPUT_LENGTH}
        bind:value={currentPassword}
        disabled={pending}
      /></label
    >
    <div class="field-grid">
      <label class="field"
        >Новый пароль<Input
          type="password"
          autocomplete="new-password"
          required
          minlength={MIN_PASSWORD_CHARACTERS}
          maxlength={MAX_PASSWORD_INPUT_LENGTH}
          bind:value={newPassword}
          disabled={pending}
        /></label
      >
      <label class="field"
        >Повторите новый пароль<Input
          type="password"
          autocomplete="new-password"
          required
          minlength={MIN_PASSWORD_CHARACTERS}
          maxlength={MAX_PASSWORD_INPUT_LENGTH}
          bind:value={confirmation}
          disabled={pending}
        /></label
      >
    </div>
    {#if error}<div class="error" role="alert">{error}</div>{/if}
    <div class="actions">
      <Button
        loading={pending}
        variant="primary"
        type="submit"
        disabled={pending}
        >{pending ? "Изменение пароля…" : "Изменить пароль"}</Button
      >
    </div>
  </form>
</section>

<style>
  form {
    display: grid;
    gap: 18px;
    margin-top: 22px;
  }
  form > .field {
    max-width: 480px;
  }
  .actions {
    margin-top: 2px;
  }
</style>
