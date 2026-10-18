import { expect, test } from "bun:test";
import {
  resolveServerReference,
  serverUrlValue,
  agentUpdateUrl,
} from "../src/shell/navigation";
const first = {
  id: "8a88ba7c-0cd9-4af0-86f1-96f4a9766136",
  slug: "mail-one",
  name: "Renamed Mail",
};
const second = {
  id: "077e6c0f-4ff7-4b02-bb7f-63b169e64c43",
  slug: "почта-2",
  name: "Почта № 2",
};
test("server URLs resolve stable slugs and existing UUID bookmarks to internal IDs", () => {
  expect(
    resolveServerReference({ servers: [first, second], reference: "mail-one" }),
  ).toBe(first.id);
  expect(
    resolveServerReference({ servers: [first, second], reference: first.id }),
  ).toBe(first.id);
  expect(
    resolveServerReference({ servers: [first, second], reference: "почта-2" }),
  ).toBe(second.id);
  expect(serverUrlValue(first)).toBe("mail-one");
  expect(agentUpdateUrl(first)).toBe(
    "/settings/?server=mail-one&action=update-agent#agents",
  );
  expect(serverUrlValue({ id: first.id })).toBe(first.id);
  expect(() =>
    resolveServerReference({ servers: [first, second], reference: "unknown" }),
  ).toThrow();
});
