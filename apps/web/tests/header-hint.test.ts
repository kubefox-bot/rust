import { expect, test } from "bun:test";
import { runInNewContext } from "node:vm";
import {
  PROFILE_COOKIE_NAME,
  PROFILE_ISSUER,
  PROFILE_AUDIENCE,
} from "../src/shell/header/conts";

const bootstrap = await Bun.file(
  new URL("../src/shell/header/bootstrap.js", import.meta.url),
).text();
const token = (claims: unknown) =>
  `${btoa('{"alg":"HS256","typ":"JWT"}')}.${Buffer.from(JSON.stringify(claims)).toString("base64url")}.c2lnbmF0dXJl`;
const claims = {
  sub: "1",
  name: "Суперадмин",
  login: "admin",
  role: "superadmin",
  iss: "sochi-center",
  aud: "header-display",
  iat: 1,
  exp: 4102444800,
};
const render = (cookie: string) => {
  const root = { dataset: { headerProfile: "guest" } };
  runInNewContext(bootstrap, {
    document: {
      cookie,
      documentElement: root,
      currentScript: {
        dataset: {
          cookieName: PROFILE_COOKIE_NAME,
          issuer: PROFILE_ISSUER,
          audience: PROFILE_AUDIENCE,
        },
      },
    },
    atob,
    TextDecoder,
    Uint8Array,
    Date,
  });
  return root.dataset.headerProfile;
};
test("display JWT restores the header from a cookie without browser storage", () => {
  expect(render(`other=value; ${PROFILE_COOKIE_NAME}=${token(claims)}`)).toBe(
    "admin",
  );
  expect(render("")).toBe("guest");
});
test("expired, malformed and unrelated JWTs cannot populate the header", () => {
  for (const value of [
    "broken",
    token({ ...claims, exp: 1 }),
    token({ ...claims, aud: "api" }),
    token({ ...claims, role: "user" }),
    token({ ...claims, sub: null }),
    "e30=.bad.sig",
  ]) {
    expect(render(`${PROFILE_COOKIE_NAME}=${value}`)).toBe("guest");
  }
});
