<script lang="ts">
  import { Button } from "../../ui/button";
  import { Input } from "../../ui/input";
  import { login } from "./api";
  import { LOGIN_MAX_UTF16_UNITS } from "./conts";
  let username = $state("");
  let password = $state("");
  let pending = $state(false);
  let error = $state("");
  const submit = async (event: SubmitEvent) => {
    event.preventDefault();
    if (pending) return;
    pending = true;
    error = "";
    try {
      await login({ login: username, password });
      password = "";
      window.location.replace("/");
    } catch (failure) {
      error = failure instanceof Error ? failure.message : "Не удалось войти.";
    } finally {
      password = "";
      pending = false;
    }
  };
</script>

<div class="login-wrap">
  <section class="card login-card">
    <div class="login-icon" aria-hidden="true">↗</div>
    <h1>Добро пожаловать</h1>
    <p class="subtitle">Войдите в центр управления почтовыми серверами</p>
    <form onsubmit={submit}>
      <label class="field"
        >Логин<Input
          name="username"
          autocomplete="username"
          required
          maxlength={LOGIN_MAX_UTF16_UNITS}
          bind:value={username}
          disabled={pending}
        /></label
      ><label class="field"
        >Пароль<Input
          name="password"
          type="password"
          autocomplete="current-password"
          required
          maxlength={4096}
          bind:value={password}
          disabled={pending}
        /></label
      >{#if error}<div class="error" role="alert">{error}</div>{/if}<Button
        loading={pending}
        variant="primary"
        type="submit"
        disabled={pending}
        >{pending ? "Выполняется вход…" : "Войти в центр"}</Button
      >
    </form>
    <p class="login-note">
      Первый суперадмин создаётся локальной командой при установке центра.
    </p>
  </section>
</div>

<style>
  .login-wrap {
    display: grid;
    place-items: center;
    min-height: 64dvh;
  }
  .login-card {
    width: 100%;
    max-width: 430px;
    padding: 34px;
  }
  .login-icon {
    width: 42px;
    height: 42px;
    display: grid;
    place-items: center;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 22px;
    border-radius: 13px;
    margin-bottom: 22px;
  }
  .login-card h1 {
    font-size: 23px;
  }
  .login-card form {
    display: grid;
    gap: 19px;
    margin-top: 30px;
  }
  .login-note {
    font-size: 10px;
    color: var(--muted);
    line-height: 1.7;
    margin: 24px 0 0;
    border-top: 1px solid var(--line);
    padding-top: 20px;
  }
</style>
