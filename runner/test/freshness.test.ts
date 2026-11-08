import { expect, test } from "bun:test";
import { FRESHNESS_URL, FreshnessError, checkFreshness, parseFreshness } from "../src/freshness";

const DAY = 86_400_000;
const NOW = Date.parse("2026-11-08T12:00:00Z");
const record = (iso: string) => JSON.stringify({ schemaVersion: 1, lastSuccessfulPinCheck: iso });
const at = (ms: number) => new Date(NOW - ms).toISOString().replace(/\.\d{3}Z$/, "Z");
const ok = (text: string) => async () => text;

test("the record lives on the freshness branch of the package repository", () => {
  expect(FRESHNESS_URL).toBe("https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/freshness/freshness.json");
});

test("a valid record parses to its UTC time", () => {
  expect(parseFreshness(record("2026-11-01T08:30:00Z")).getTime()).toBe(Date.parse("2026-11-01T08:30:00Z"));
});

test("anything that is not a valid record is refused", () => {
  const bad = ["", "nope", "[]", "{}", JSON.stringify({ schemaVersion: 2, lastSuccessfulPinCheck: "2026-11-01T08:30:00Z" }),
    record("2026-11-01"), record("2026-11-01T08:30:00+01:00"), record("2026-11-01T08:30:00"), record("2026-13-45T25:61:61Z"), record("yesterday"),
    JSON.stringify({ schemaVersion: 1, lastSuccessfulPinCheck: 5 })];
  for (const text of bad) expect(() => parseFreshness(text), text).toThrow(FreshnessError);
});

test("a record up to 30 days old is fresh and older than that is stale", async () => {
  const run = (age: number) => checkFreshness({ fetcher: ok(record(at(age))), now: NOW, delayMs: 1 });
  expect((await run(0)).state).toBe("fresh");
  expect((await run(29.99 * DAY)).state).toBe("fresh");
  expect((await run(30 * DAY)).state).toBe("fresh");
  expect((await run(30 * DAY + 1000)).state).toBe("stale");
  expect((await run(90 * DAY)).state).toBe("stale");
});

test("the age is reported in whole days and the limit can be changed", async () => {
  const r = await checkFreshness({ fetcher: ok(record(at(3.9 * DAY))), now: NOW, delayMs: 1 });
  expect(r).toMatchObject({ state: "fresh", ageDays: 3 });
  expect((await checkFreshness({ fetcher: ok(record(at(3.9 * DAY))), now: NOW, maxAgeDays: 2, delayMs: 1 })).state).toBe("stale");
});

test("a record dated a little in the future is fresh, and one far in the future is refused as a clock or data error", async () => {
  expect((await checkFreshness({ fetcher: ok(record(at(-3_600_000))), now: NOW, delayMs: 1 })).state).toBe("fresh");
  await expect(checkFreshness({ fetcher: ok(record(at(-3 * DAY))), now: NOW, delayMs: 1 })).rejects.toThrow("in the future");
});

test("a fetch that fails is retried and the third attempt succeeds", async () => {
  let calls = 0;
  const fetcher = async () => {
    if (++calls < 3) throw new Error("HTTP 504");
    return record(at(DAY));
  };
  expect((await checkFreshness({ fetcher, now: NOW, attempts: 3, delayMs: 1 })).state).toBe("fresh");
  expect(calls).toBe(3);
});

test("a fetch that never works fails closed after every attempt, it never reports fresh", async () => {
  let calls = 0;
  const fetcher = async () => {
    calls++;
    throw new Error("HTTP 504");
  };
  await expect(checkFreshness({ fetcher, now: NOW, attempts: 3, delayMs: 1 })).rejects.toThrow("after 3 attempts");
  expect(calls).toBe(3);
});

test("a record that is fetched but invalid is not retried", async () => {
  let calls = 0;
  const fetcher = async () => {
    calls++;
    return "garbage";
  };
  await expect(checkFreshness({ fetcher, now: NOW, attempts: 3, delayMs: 1 })).rejects.toThrow(FreshnessError);
  expect(calls).toBe(1);
});
