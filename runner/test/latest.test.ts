import { expect, test } from "bun:test";
import { LatestError, RELEASES_API, fetchReleases, latestVersion, type Release } from "../src/latest";

const r = (tag: string, extra: Partial<Release> = {}): Release => ({ tag_name: tag, draft: false, prerelease: false, ...extra });

test("the releases come from the package repository", () => {
  expect(RELEASES_API).toBe("https://api.github.com/repos/pantheon-org/pantheon-scaffold/releases?per_page=100");
});

test("the newest stable release wins, compared as versions and not as text", () => {
  expect(latestVersion([r("v0.9.0"), r("v0.10.0"), r("v0.2.0")], {})).toBe("0.10.0");
});

test("drafts are never offered", () => {
  expect(latestVersion([r("v0.1.0"), r("v0.2.0", { draft: true })], {})).toBe("0.1.0");
});

test("pre-releases are skipped unless asked for", () => {
  const list = [r("v0.1.0"), r("v0.2.0", { prerelease: true })];
  expect(latestVersion(list, {})).toBe("0.1.0");
  expect(latestVersion(list, { includePrereleases: true })).toBe("0.2.0");
});

test("with pre-releases a pre-release ranks below its own release", () => {
  expect(latestVersion([r("v0.2.0-rc.1", { prerelease: true }), r("v0.2.0")], { includePrereleases: true })).toBe("0.2.0");
  expect(latestVersion([r("v0.2.0-rc.2", { prerelease: true }), r("v0.2.0-rc.10", { prerelease: true })], { includePrereleases: true })).toBe("0.2.0-rc.10");
});

test("a tag that is not a version is ignored, never guessed at", () => {
  expect(latestVersion([r("latest"), r("v1"), r("nightly-2026"), r("v0.3.0")], {})).toBe("0.3.0");
});

test("a version on the known-bad list is skipped", () => {
  const knownBad = { versions: [{ version: "0.2.0", reason: "broken" }] };
  expect(latestVersion([r("v0.1.0"), r("v0.2.0")], { knownBad })).toBe("0.1.0");
});

test("no release that qualifies gives null", () => {
  expect(latestVersion([], {})).toBeNull();
  expect(latestVersion([r("v0.1.0", { prerelease: true })], {})).toBeNull();
});

test("fetchReleases reads the list and sends the token when there is one", async () => {
  let seen: Record<string, string> = {};
  const fetcher = async (_url: string, headers: Record<string, string>) => {
    seen = headers;
    return JSON.stringify([{ tag_name: "v0.1.0", draft: false, prerelease: true }]);
  };
  expect(await fetchReleases({ fetcher, token: "t0ken", delayMs: 1 })).toEqual([r("v0.1.0", { prerelease: true })]);
  expect(seen.Authorization).toBe("Bearer t0ken");
  await fetchReleases({ fetcher, delayMs: 1 });
  expect(seen.Authorization).toBeUndefined();
});

test("a response that is not a list of releases is refused", async () => {
  for (const body of ["{}", "nope", JSON.stringify([{ tag_name: 3 }]), JSON.stringify([{ draft: false }])]) {
    await expect(fetchReleases({ fetcher: async () => body, delayMs: 1 }), body).rejects.toThrow(LatestError);
  }
});

test("a fetch that fails is retried, and fails closed after every attempt", async () => {
  let calls = 0;
  const flaky = async () => {
    if (++calls < 3) throw new Error("HTTP 504");
    return "[]";
  };
  expect(await fetchReleases({ fetcher: flaky, attempts: 3, delayMs: 1 })).toEqual([]);
  expect(calls).toBe(3);
  await expect(fetchReleases({ fetcher: async () => { throw new Error("HTTP 504"); }, attempts: 2, delayMs: 1 })).rejects.toThrow("after 2 attempts");
});
