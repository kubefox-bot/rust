// Runs synchronously before the header is painted. This only chooses display;
// Rust independently authenticates every protected request using HttpOnly cookies.
(() => {
  const config = document.currentScript?.dataset;
  const cookie = document.cookie
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.startsWith(`${config?.cookieName}=`));
  if (!cookie) return;
  try {
    const token = cookie.slice(cookie.indexOf("=") + 1);
    const segments = token.split(".");
    if (
      segments.length !== 3 ||
      segments.some((part) => !/^[A-Za-z0-9_-]+$/.test(part))
    )
      return;
    const decode = (part) =>
      JSON.parse(
        new TextDecoder("utf-8", { fatal: true }).decode(
          Uint8Array.from(
            atob(part.replace(/-/g, "+").replace(/_/g, "/")),
            (char) => char.charCodeAt(0),
          ),
        ),
      );
    const header = decode(segments[0]);
    const profile = decode(segments[1]);
    const nowSeconds = Math.floor(Date.now() / 1000);
    if (
      header?.alg !== "HS256" ||
      header?.typ !== "JWT" ||
      profile?.iss !== config?.issuer ||
      profile?.aud !== config?.audience ||
      profile?.sub !== "1" ||
      profile?.role !== "superadmin" ||
      profile?.name !== "Суперадмин" ||
      typeof profile?.login !== "string" ||
      !profile.login ||
      !Number.isSafeInteger(profile.exp) ||
      !Number.isSafeInteger(profile.iat) ||
      profile.exp <= nowSeconds ||
      profile.iat > nowSeconds ||
      profile.exp <= profile.iat
    )
      return;
    // Decoding is not signature verification and grants no permissions.
    document.documentElement.dataset.headerProfile = "admin";
  } catch {
    // Malformed or unrelated display data is treated as absent.
  }
})();
