import { expect, test } from "bun:test";
import { agentStatus, agentBadge, footerText } from "../src/shell/agent-status";
import type { BrowserContracts } from "../src/api";
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
