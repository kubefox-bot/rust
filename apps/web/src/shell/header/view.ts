export const showHeaderProfile = (): void => {
  // Rust has confirmed the session and issued the encrypted HttpOnly profile.
  document.documentElement.dataset.headerProfile = "admin";
};

export const forgetHeaderProfile = (): void => {
  // Rust owns HttpOnly cookie lifecycle; this only clears the visible header.
  document.documentElement.dataset.headerProfile = "guest";
};
