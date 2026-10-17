import { expect, test } from "bun:test";
import { createSubscriptions } from "../src/api/events";
class Source extends EventTarget {
  closed = false;
  close() {
    this.closed = true;
  }
  emit(name: string, data: unknown) {
    this.dispatchEvent(new MessageEvent(name, { data: JSON.stringify(data) }));
  }
}
const serverId = "75e684ad-64d2-4f24-a587-e02b172d3ac0";

test("subscribers share one transport, released by the last owner", () => {
  const sources: Source[] = [];
  const bus = createSubscriptions({
    open: () => {
      const source = new Source();
      sources.push(source);
      return source;
    },
    authenticated: async () => true,
  });
  const options = {
    serverId,
    onEvent: () => {},
    onResync: () => {},
    onUnauthorized: () => {},
  };
  const disposeOne = bus.subscribe(options);
  const disposeTwo = bus.subscribe(options);
  expect(sources).toHaveLength(1);
  disposeOne();
  expect(sources[0]?.closed).toBe(false);
  disposeTwo();
  expect(sources[0]?.closed).toBe(true);
});

test("malformed events require one resync and never reach subscribers", () => {
  const source = new Source();
  const bus = createSubscriptions({
    open: () => source,
    authenticated: async () => true,
  });
  const counts = { resync: 0, delivered: 0 };
  const dispose = bus.subscribe({
    serverId,
    onEvent: () => {
      counts.delivered++;
    },
    onResync: () => {
      counts.resync++;
    },
    onUnauthorized: () => {},
  });
  source.emit("snapshot", { stream_epoch: "invalid", revision: "9" });
  expect(counts).toEqual({ resync: 1, delivered: 0 });
  dispose();
});

test("changed epoch and revision gap require fresh snapshot; stale revisions are ignored", async () => {
  const { eventFixture } = await import("./fixtures/event");
  const source = new Source();
  const bus = createSubscriptions({
    open: () => source,
    authenticated: async () => true,
  });
  const counts = { resync: 0, delivered: 0 };
  const dispose = bus.subscribe({
    serverId,
    onEvent: () => {
      counts.delivered++;
    },
    onResync: () => {
      counts.resync++;
    },
    onUnauthorized: () => {},
  });
  const epoch = "7a96d933-25ae-4eca-b840-d573a62dc778";
  source.emit("snapshot", eventFixture(epoch, "1"));
  source.emit("snapshot", eventFixture(epoch, "1"));
  expect(counts.delivered).toBe(1);
  source.emit(
    "snapshot",
    eventFixture("daaa4959-b5f2-4c30-a6fd-e6ad08495ffb", "2"),
  );
  expect(counts).toEqual({ resync: 1, delivered: 1 });
  source.emit(
    "snapshot",
    eventFixture("daaa4959-b5f2-4c30-a6fd-e6ad08495ffb", "4"),
  );
  expect(counts.resync).toBe(2);
  dispose();
});
