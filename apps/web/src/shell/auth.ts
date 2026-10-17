import { activity, logout, session } from "../features/auth";
import { ACTIVITY_INTERVAL_MS, SESSION_CHECK_MS } from "./conts";
type Callbacks = Readonly<{
  onAuthorized: (generation: string) => Promise<void>;
  onAnonymous: () => void;
  onFailure: (error: Error) => void;
}>;
const header = (authenticated: boolean) => {
  const label = document.querySelector("[data-account-label]");
  const avatar = document.querySelector("[data-account-link] .avatar");
  const link = document.querySelector<HTMLAnchorElement>("[data-account-link]");
  const button = document.querySelector<HTMLButtonElement>("[data-logout]");
  if (label) label.textContent = authenticated ? "Суперадмин" : "Вход";
  if (avatar) avatar.textContent = authenticated ? "SA" : "··";
  if (link) link.href = authenticated ? "/settings/#account" : "/login/";
  if (button) button.hidden = !authenticated;
};
export const mountAuth = (callbacks: Callbacks) => {
  const controller = new AbortController();
  let generation: string | null = null;
  let checking = false;
  let hidden = false;
  let lastActivity = 0;
  const clear = () => {
    generation = null;
    header(false);
    callbacks.onAnonymous();
  };
  const refresh = async () => {
    if (checking || hidden || controller.signal.aborted) return;
    checking = true;
    try {
      const current = await session(controller.signal);
      if (controller.signal.aborted || hidden) return;
      if (!current.authenticated || !current.generation) clear();
      else if (current.generation !== generation) {
        clear();
        generation = current.generation;
        const admitted = generation;
        await callbacks.onAuthorized(admitted);
        if (hidden || controller.signal.aborted || generation !== admitted)
          return;
        header(true);
      }
      document.documentElement.classList.remove("session-hidden");
    } catch (error) {
      if (!controller.signal.aborted && !hidden) {
        clear();
        callbacks.onFailure(
          error instanceof Error
            ? error
            : new Error("Не удалось проверить сеанс."),
        );
        document.documentElement.classList.remove("session-hidden");
      }
    } finally {
      checking = false;
    }
  };
  const userActivity = (event: Event) => {
    if (
      !event.isTrusted ||
      !generation ||
      document.visibilityState !== "visible" ||
      Date.now() - lastActivity < ACTIVITY_INTERVAL_MS
    )
      return;
    lastActivity = Date.now();
    activity(controller.signal).catch(() => refresh());
  };
  const hide = () => {
    hidden = true;
    document.documentElement.classList.add("session-hidden");
    clear();
  };
  const show = () => {
    hidden = false;
    refresh().catch((error) => callbacks.onFailure(error));
  };
  const signOut = () => {
    logout()
      .then(() => {
        clear();
        window.location.replace("/login/");
      })
      .catch((error) => {
        callbacks.onFailure(
          error instanceof Error ? error : new Error("Выход не подтверждён."),
        );
        refresh().catch((error) => callbacks.onFailure(error));
      });
  };
  const button = document.querySelector("[data-logout]");
  button?.addEventListener("click", signOut);
  window.addEventListener("pagehide", hide);
  window.addEventListener("pageshow", show);
  document.addEventListener("pointerdown", userActivity);
  document.addEventListener("keydown", userActivity);
  document.addEventListener("input", userActivity);
  const timer = window.setInterval(() => {
    refresh().catch((error) => callbacks.onFailure(error));
  }, SESSION_CHECK_MS);
  refresh().catch((error) => callbacks.onFailure(error));
  return {
    refresh,
    dispose: () => {
      controller.abort();
      clearInterval(timer);
      button?.removeEventListener("click", signOut);
      window.removeEventListener("pagehide", hide);
      window.removeEventListener("pageshow", show);
      document.removeEventListener("pointerdown", userActivity);
      document.removeEventListener("keydown", userActivity);
      document.removeEventListener("input", userActivity);
      header(false);
    },
  };
};
