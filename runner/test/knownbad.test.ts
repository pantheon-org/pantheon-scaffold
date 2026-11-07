import { afterEach, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { CACHE_FILE, KNOWN_BAD_URL, loadKnownBad, parseKnownBad } from "../src/knownbad";
import { cleanupRepos, get, has, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const list = { versions: [{ version: "0.2.0", reason: "broken deny.toml" }] };
const bundled = { versions: [{ version: "0.1.0", reason: "bundled snapshot entry" }] };
const ok = async () => JSON.stringify(list);
const down = async () => {
  throw new Error("offline");
};

test("a valid list parses, anything else does not", () => {
  expect(parseKnownBad(JSON.stringify(list))).toEqual(list);
  for (const bad of ["", "nope", "[]", "{}", '{"versions":[{"version":"x","reason":"r"}]}', '{"versions":[{"version":"1.0.0"}]}', '{"versions":"no"}'])
    expect(parseKnownBad(bad)).toBeNull();
});

test("a fetched list is used and written to the cache", async () => {
  const root = tempRepo();
  const r = await loadKnownBad(root, { fetcher: ok, bundled });
  expect(r).toMatchObject({ source: "fetched", list });
  expect(JSON.parse(get(root, CACHE_FILE))).toEqual(list);
});

test("when the fetch fails the cached list is used, with a note", async () => {
  const root = tempRepo();
  put(root, CACHE_FILE, JSON.stringify(list));
  const r = await loadKnownBad(root, { fetcher: down, bundled });
  expect(r.source).toBe("cache");
  expect(r.list).toEqual(list);
  expect(r.note).toContain("using the cached list");
});

test("with no fetch and no cache the bundled snapshot is used, with a note", async () => {
  const r = await loadKnownBad(tempRepo(), { fetcher: down, bundled });
  expect(r).toMatchObject({ source: "bundled", list: bundled });
  expect(r.note).toContain("bundled snapshot");
});

test("a fetched list that does not validate is ignored and never cached", async () => {
  const root = tempRepo();
  const r = await loadKnownBad(root, { fetcher: async () => '{"versions":"oops"}', bundled });
  expect(r.source).toBe("bundled");
  expect(has(root, CACHE_FILE)).toBe(false);
});

test("a corrupt cache is ignored", async () => {
  const root = tempRepo();
  put(root, CACHE_FILE, "{not json");
  expect((await loadKnownBad(root, { fetcher: down, bundled })).source).toBe("bundled");
});

test("a fetch that succeeds after an earlier cache replaces the cache", async () => {
  const root = tempRepo();
  put(root, CACHE_FILE, JSON.stringify(bundled));
  await loadKnownBad(root, { fetcher: ok, bundled });
  expect(JSON.parse(get(root, CACHE_FILE))).toEqual(list);
});

test("the default fetcher reads the URL and fails on a bad status", async () => {
  const server = Bun.serve({ port: 0, fetch: (req) => (new URL(req.url).pathname === "/ok.json" ? new Response(JSON.stringify(list)) : new Response(JSON.stringify(list), { status: 404 })) });
  try {
    const root = tempRepo();
    const good = await loadKnownBad(root, { url: `http://localhost:${server.port}/ok.json`, bundled });
    expect(good.source).toBe("fetched");
    const missing = await loadKnownBad(tempRepo(), { url: `http://localhost:${server.port}/nope.json`, bundled });
    // The 404 body is a valid list on purpose: only the status check can reject it.
    expect(missing.source).toBe("bundled");
  } finally {
    server.stop(true);
  }
});

test("the default fetch gives up after the timeout", async () => {
  const server = Bun.serve({ port: 0, fetch: () => new Promise<Response>(() => {}) });
  try {
    const t0 = Date.now();
    const r = await loadKnownBad(tempRepo(), { url: `http://localhost:${server.port}/slow.json`, bundled, timeoutMs: 200 });
    expect(r.source).toBe("bundled");
    expect(Date.now() - t0).toBeLessThan(3000);
  } finally {
    server.stop(true);
  }
});

test("the default URL is the list on the package repo's main branch", () => {
  expect(KNOWN_BAD_URL).toBe("https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/main/known-bad.json");
  expect(join(CACHE_FILE)).toBe(".pkl-gen/cache/known-bad.json");
});

// known-bad.json on main is what the runner fetches, and the bundled snapshot is its last fallback.
const rootList = readFileSync(join(import.meta.dir, "../../known-bad.json"), "utf8");
const bundledText = readFileSync(join(import.meta.dir, "../src/known-bad.bundled.json"), "utf8");

test("known-bad.json on main is a valid list and names v0.1.0, which cannot read a checksum-pinned package URI", () => {
  const parsed = parseKnownBad(rootList);
  expect(parsed).not.toBeNull();
  const entry = parsed!.versions.find((v) => v.version === "0.1.0");
  expect(entry?.reason).toContain("checksum");
});

test("the bundled snapshot is a valid list, and everything in it is also in known-bad.json", () => {
  const bundledList = parseKnownBad(bundledText);
  const published = parseKnownBad(rootList);
  expect(bundledList).not.toBeNull();
  for (const v of bundledList!.versions) expect(published!.versions).toContainEqual(v);
});
