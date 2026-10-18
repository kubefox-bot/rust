import { expect, test } from "bun:test";
import { isNewerAgentBuild } from "../src/agents";

test("agent update requires a provably newer release or build", () => {
  expect(isNewerAgentBuild({ current: "1.2.9", available: "1.2.10" })).toBe(
    true,
  );
  expect(isNewerAgentBuild({ current: "1.2.10", available: "1.2.9" })).toBe(
    false,
  );
  expect(
    isNewerAgentBuild({ current: "1.0.0+abc.100", available: "1.0.0+def.200" }),
  ).toBe(true);
  expect(
    isNewerAgentBuild({ current: "1.0.0+abc.100", available: "1.0.0+abc.100" }),
  ).toBe(false);
  expect(
    isNewerAgentBuild({ current: "tls-fixture", available: "1.0.0" }),
  ).toBe(false);
  expect(isNewerAgentBuild({ current: "1.0.0", available: "invalid" })).toBe(
    false,
  );
  expect(
    isNewerAgentBuild({ current: "1.0.0", available: "1.0.0+abc.100" }),
  ).toBe(false);
});
