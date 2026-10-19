import { expect, test } from "bun:test";
import { agentStatus, agentBadge, footerText } from "../src/shell/agent-status";
import type { BrowserContracts } from "../src/api";

class Visibility extends EventTarget {
  visibilityState: DocumentVisibilityState = "visible";
  change(state: DocumentVisibilityState) {
    this.visibilityState = state;
    this.dispatchEvent(new Event("visibilitychange"));
  }
}

test("data polling pauses while hidden, refreshes on return and releases its listener", async () => {
  const { mountVisiblePolling } = await import("../src/shell/visibility");
  const visibility = new Visibility();
  const refreshed: string[] = [];
  const dispose = mountVisiblePolling({
    visibility,
    intervalMs: 10,
    refresh: () => {
      refreshed.push(visibility.visibilityState);
    },
  });
  expect(refreshed).toEqual(["visible"]);
  visibility.change("hidden");
  await Bun.sleep(25);
  expect(refreshed).toEqual(["visible"]);
  visibility.change("visible");
  expect(refreshed).toEqual(["visible", "visible"]);
  visibility.change("visible");
  expect(refreshed).toHaveLength(2);
  await Bun.sleep(15);
  expect(refreshed.length).toBeGreaterThan(2);
  dispose();
  const finalCount = refreshed.length;
  visibility.change("hidden");
  visibility.change("visible");
  await Bun.sleep(15);
  expect(refreshed).toHaveLength(finalCount);
});

test("an initially hidden page performs no data poll until it becomes visible", async () => {
  const { mountVisiblePolling } = await import("../src/shell/visibility");
  const visibility = new Visibility();
  visibility.change("hidden");
  let refreshed = 0;
  const dispose = mountVisiblePolling({
    visibility,
    intervalMs: 10,
    refresh: () => {
      refreshed++;
    },
  });
  await Bun.sleep(15);
  expect(refreshed).toBe(0);
  visibility.change("visible");
  expect(refreshed).toBe(1);
  dispose();
});
const agent: BrowserContracts["agents"][number] = {
  agent_id: "11111111-1111-4111-8111-111111111111",
  server_id: "22222222-2222-4222-8222-222222222222",
  name: "Mail",
  revoked: false,
  certificate_expired: false,
  certificate_expiring: false,
  health: {
    agent_id: "11111111-1111-4111-8111-111111111111",
    online: true,
    stale: false,
    version: "v1",
    observed_at: "2026-10-17T10:00:00Z",
  },
};
test("footer classifies actual observed stale health before the offline flag", () => {
  const stale = {
    ...agent,
    health: { ...agent.health, online: false, stale: true },
  };
  const revoked = { ...stale, revoked: true };
  const expired = { ...stale, certificate_expired: true };
  const pending = {
    ...agent,
    health: {
      ...agent.health,
      online: false,
      stale: true,
      observed_at: null,
      version: null,
    },
  };
  expect(agentStatus(agent)).toBe("online");
  expect(agentStatus(stale)).toBe("stale");
  expect(agentStatus(revoked)).toBe("offline");
  expect(agentStatus(expired)).toBe("offline");
  expect(agentStatus(pending)).toBe("pending");
  expect(agentStatus({ ...pending, revoked: true })).toBe("offline");
  expect(agentStatus({ ...pending, certificate_expired: true })).toBe(
    "offline",
  );
  expect(agentBadge(agent)).toEqual({
    state: "online",
    label: "Агент на связи",
  });
  expect(agentBadge(expired)).toEqual({
    state: "offline",
    label: "Сертификат истёк",
  });
  expect(agentBadge(revoked)).toEqual({
    state: "offline",
    label: "Доступ отозван",
  });
  expect(agentBadge(pending).state).toBe("pending");
  expect(
    agentStatus({ ...agent, health: { ...agent.health, observed_at: null } }),
  ).toBe("pending");
  expect(
    agentStatus({ ...agent, health: { ...agent.health, online: false } }),
  ).toBe("offline");
  const text = footerText([agent, stale, revoked, pending]);
  expect(text).toContain("1 на связи");
  expect(text).toContain("1 устарело");
  expect(text).toContain("1 недоступен");
  expect(text).toContain("1 ожидает проверки");
  expect(text).toContain("v1");
  expect(text).toContain("2026");
  expect(footerText([])).toContain("не добавлены");
});
