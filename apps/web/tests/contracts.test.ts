import { describe, expect, test } from "bun:test";
import { decode } from "../src/api/decode";
import schema from "../src/api/generated/browser.schema.json";

describe("generated browser contracts", () => {
  test("counters remain exact above JavaScript safe integer range", () => {
    expect(decode("counter", "9007199254740993")).toBe("9007199254740993");
    expect(decode("counter", "18446744073709551615")).toBe(
      "18446744073709551615",
    );
  });
  test("rejects overflow, numeric and noncanonical counters", () => {
    for (const value of ["18446744073709551616", "00", "01", "-1", "1.0", 42]) {
      expect(() => decode("counter", value)).toThrow();
    }
  });
  test("rejects unknown phase and noncanonical or nil server identifiers", () => {
    expect(() => decode("phase", "perhaps_finished")).toThrow();
    for (const value of [
      "00000000-0000-0000-0000-000000000000",
      "INVALID",
      "75E684AD-64D2-4F24-A587-E02B172D3AC0",
    ]) {
      expect(() => decode("server_id", value)).toThrow();
    }
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
  const summary = decode("summary", summaryFixture);
  expect(summary.removed_bytes).toBe("9007199254740993");
  expect(summary.total_scan_files).toBeNull();
  expect(summary.remaining_old_files).toBeNull();
});
