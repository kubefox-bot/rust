import { PROFILE_COOKIE_NAME } from "./conts";

export const showHeaderProfile = (): void => {
  // Only called after Rust confirms the session. Rust also issues the JWT cookie.
  document.documentElement.dataset.headerProfile = "admin";
};

export const forgetHeaderProfile = (): void => {
  document.cookie = `${PROFILE_COOKIE_NAME}=; Max-Age=0; Path=/; Secure; SameSite=Strict`;
  document.documentElement.dataset.headerProfile = "guest";
};
