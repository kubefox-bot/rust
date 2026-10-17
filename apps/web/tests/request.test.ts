import { expect, spyOn, test } from "bun:test";
import { ApiError, ContractError, requestJson } from "../src/api";

test("malformed successful response never exposes response text in an error", async () => {
  const fetch = spyOn(globalThis, "fetch").mockResolvedValue(
    new Response("PRIVATE-PROFILE-SENTINEL", { status: 200 }),
  );
  try {
    const error = await requestJson({
      path: "/auth/session",
      responseSchema: "session",
    }).catch((error: unknown) => error);
    expect(error).toBeInstanceOf(ContractError);
  } finally {
    fetch.mockRestore();
  }
});
test("unsafe requests use same-origin credentials and required JSON/CSRF headers", async () => {
  const calls: RequestInit[] = [];
  const fetch = spyOn(globalThis, "fetch").mockImplementation(
    Object.assign(
      async (
        _input: Parameters<typeof globalThis.fetch>[0],
        init?: Parameters<typeof globalThis.fetch>[1],
      ) => {
        if (init) calls.push(init);
        return Response.json({
          authenticated: true,
          generation: "9007199254740993",
        });
      },
      { preconnect: globalThis.fetch.preconnect },
    ),
  );
  try {
    const result = await requestJson({
      path: "/auth/login",
      method: "POST",
      body: { login: "admin", password: "fixture-password" },
      responseSchema: "session",
    });
    expect(result.generation).toBe("9007199254740993");
    expect(calls[0]?.credentials).toBe("same-origin");
    expect(calls[0]?.redirect).toBe("error");
    const headers = new Headers(calls[0]?.headers);
    expect(headers.get("x-hmail-request")).toBe("1");
    expect(headers.get("content-type")).toBe("application/json");
  } finally {
    fetch.mockRestore();
  }
});
test("malformed failure body still preserves HTTP failure status", async () => {
  const fetch = spyOn(globalThis, "fetch").mockResolvedValue(
    new Response("PRIVATE-FAILURE-SENTINEL", { status: 503 }),
  );
  try {
    const error = await requestJson({
      path: "/auth/session",
      responseSchema: "session",
    }).catch((error: unknown) => error);
    expect(error).toBeInstanceOf(ApiError);
    if (!(error instanceof ApiError)) throw new Error("Expected API failure");
    expect(error.status).toBe(503);
    expect(error.message).not.toContain("PRIVATE");
  } finally {
    fetch.mockRestore();
  }
});
