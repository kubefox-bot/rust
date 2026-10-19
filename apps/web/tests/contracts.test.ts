import {
  counter as counterSchema,
  job_event as jobEventSchema,
  phase as phaseSchema,
  server_id as serverIdSchema,
  summary as summarySchema,
  update_policy as updatePolicySchema,
} from "../src/api/contracts";
import { describe, expect, test } from "bun:test";
import { ContractError, decode } from "../src/api/decode";
import schema from "../src/api/generated/browser.schema.json";
import { eventFixture } from "./fixtures/event";

describe("generated browser contracts", () => {
  test("counters remain exact above JavaScript safe integer range", () => {
    expect(decode(counterSchema, "9007199254740993")).toBe("9007199254740993");
    expect(decode(counterSchema, "18446744073709551615")).toBe(
      "18446744073709551615",
    );
  });
  test("rejects overflow, numeric and noncanonical counters", () => {
    for (const value of ["18446744073709551616", "00", "01", "-1", "1.0", 42]) {
      expect(() => decode(counterSchema, value)).toThrow();
    }
  });
  test("rejects unknown phase and noncanonical or nil server identifiers", () => {
    expect(() => decode(phaseSchema, "perhaps_finished")).toThrow();
    for (const value of [
      "00000000-0000-0000-0000-000000000000",
      "INVALID",
      "75E684AD-64D2-4F24-A587-E02B172D3AC0",
    ]) {
      expect(() => decode(serverIdSchema, value)).toThrow();
    }
  });
  test("nested job references reject invalid counters, dates and extra fields without changing the input", () => {
    const event = eventFixture(
      "7a96d933-25ae-4eca-b840-d573a62dc778",
      "18446744073709551615",
    );
    const intent = {
      ...event.job.intent,
      profile_version: "18446744073709551615",
      resolved_cutoff: {
        ...event.job.intent.resolved_cutoff,
        local: "2024-02-29T23:59:59.123456789",
      },
    };
    const valid = { ...event, job: { ...event.job, intent } };
    const original = structuredClone(valid);
    expect(decode(jobEventSchema, valid)).toBe(valid);
    for (const invalidIntent of [
      { ...intent, profile_version: "18446744073709551616" },
      { ...intent, profile_version: 42 },
      {
        ...intent,
        resolved_cutoff: {
          ...intent.resolved_cutoff,
          local: "2024-02-30T23:59:59.123456789",
        },
      },
      { ...intent, unexpected: true },
    ]) {
      const invalid = { ...valid, job: { ...valid.job, intent: invalidIntent } };
      expect(() => decode(jobEventSchema, invalid)).toThrow(ContractError);
      expect(decode(jobEventSchema, valid)).toBe(valid);
    }
    expect(valid).toEqual(original);
  });
  test("exports only browser contracts, with no agent identity or mail credentials", () => {
    const text = JSON.stringify(schema);
    for (const name of [
      "AgentCredentials",
      "ClientIdentity",
      "MailProfile",
      "JobCommand",
      "identity_key",
      "ca_private_key",
    ]) {
      expect(text).not.toContain(name);
    }
  });
});

test("summary retains exact removed bytes and unknown counts", async () => {
  const { summaryFixture } = await import("./fixtures/summary");
  const summary = decode(summarySchema, summaryFixture);
  expect(summary.removed_bytes).toBe("9007199254740993");
  expect(summary.total_scan_files).toBeNull();
  expect(summary.remaining_old_files).toBeNull();
});

test("update policy rejects unsupported timing, missing fields and operational switches", () => {
  expect(
    decode(updatePolicySchema, {
      check_interval_seconds: 30,
      graceful_timeout_minutes: 1440,
    }),
  ).toEqual({ check_interval_seconds: 30, graceful_timeout_minutes: 1440 });
  for (const value of [
    { check_interval_seconds: 29, graceful_timeout_minutes: 15 },
    { check_interval_seconds: 86401, graceful_timeout_minutes: 15 },
    { check_interval_seconds: 60, graceful_timeout_minutes: 0 },
    { check_interval_seconds: 60, graceful_timeout_minutes: 1441 },
    { check_interval_seconds: 60.5, graceful_timeout_minutes: 15 },
    { check_interval_seconds: 60 },
    {
      check_interval_seconds: 60,
      graceful_timeout_minutes: 15,
      disable_auto: true,
    },
  ])
    expect(() => decode(updatePolicySchema, value)).toThrow();
});
