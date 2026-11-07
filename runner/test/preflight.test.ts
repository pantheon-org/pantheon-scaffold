import { expect, test } from "bun:test";
import { emptyManifest } from "../src/manifest";
import { packageVersionOf, preflight } from "../src/preflight";
import { contractOf, owned } from "./helpers";

const uri = (v: string) => `package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${v}/pantheon-scaffold@${v}#/Config.pkl`;
const withUri = (v?: string, extra: Record<string, unknown> = {}) => ({ ...contractOf(owned("a.txt", "x\n")), ...(v ? { packageUri: uri(v) } : {}), ...extra });
const base = { mode: "render" as const, runnerVersion: "0.5.0" };
const manifestAt = (v: string) => ({ ...emptyManifest(), packageVersion: v });

test("the version is read from the package URI, pre-releases included", () => {
  expect(packageVersionOf(uri("0.2.0"))).toBe("0.2.0");
  expect(packageVersionOf(uri("0.2.0-rc.1"))).toBe("0.2.0-rc.1");
});
test("a local package (file:) or no URI has no version", () => {
  expect(packageVersionOf(undefined)).toBeUndefined();
  expect(packageVersionOf("file:///work/pkl/Config.pkl")).toBeUndefined();
});
test("a package URI with no readable version is an error, never ignored", () => {
  expect(() => packageVersionOf("package://github.com/x/y/releases/download/v1/y@latest#/Config.pkl")).toThrow("readable version");
  expect(() => packageVersionOf("package://github.com/x/y#/Config.pkl")).toThrow("readable version");
});

test("a render with no package version and nothing to report is clean", () => {
  expect(preflight(withUri(), emptyManifest(), base)).toEqual({ errors: [], refusals: [], warnings: [] });
});
test("the version is returned for the manifest", () => expect(preflight(withUri("0.2.0"), emptyManifest(), base).packageVersion).toBe("0.2.0"));

test("a runner older than the package requires is an error", () => {
  const r = preflight(withUri("0.2.0", { requires: { runner: "0.6.0" } }), emptyManifest(), base);
  expect(r.errors.join()).toContain("runner 0.5.0 is older than the 0.6.0 this package requires");
});
test("a runner that meets the requirement, or exceeds it, passes", () => {
  expect(preflight(withUri("0.2.0", { requires: { runner: "0.5.0" } }), emptyManifest(), base).errors).toEqual([]);
  expect(preflight(withUri("0.2.0", { requires: { runner: "0.4.0" } }), emptyManifest(), base).errors).toEqual([]);
});
test("a requirement that is not a version is an error", () => {
  expect(preflight(withUri("0.2.0", { requires: { runner: "soon" } }), emptyManifest(), base).errors.join()).toContain("not a version");
});
test("a requirement is checked for a local package too", () => {
  expect(preflight(withUri(undefined, { requires: { runner: "9.0.0" } }), emptyManifest(), base).errors).toHaveLength(1);
});

test("an older package than the last render is refused, and --force allows it with a warning", () => {
  const refused = preflight(withUri("0.2.0"), manifestAt("0.3.0"), base);
  expect(refused.refusals.join()).toContain("package 0.2.0 is older than 0.3.0");
  const forced = preflight(withUri("0.2.0"), manifestAt("0.3.0"), { ...base, force: true });
  expect(forced.refusals).toEqual([]);
  expect(forced.warnings.join()).toContain("older than 0.3.0");
});
test("a pre-release of the recorded version counts as older", () => {
  expect(preflight(withUri("0.3.0-rc.1"), manifestAt("0.3.0"), base).refusals).toHaveLength(1);
});
test("the same version and a newer version are fine", () => {
  expect(preflight(withUri("0.3.0"), manifestAt("0.3.0"), base).refusals).toEqual([]);
  expect(preflight(withUri("0.4.0"), manifestAt("0.3.0"), base).refusals).toEqual([]);
});
test("a check only warns about an older package, it writes nothing", () => {
  const r = preflight(withUri("0.2.0"), manifestAt("0.3.0"), { ...base, mode: "check" });
  expect(r.refusals).toEqual([]);
  expect(r.warnings.join()).toContain("older than 0.3.0");
});
test("a manifest version that is not a version is an error", () => {
  expect(preflight(withUri("0.2.0"), manifestAt("weird"), base).errors.join()).toContain("not a version");
});

const bad = { versions: [{ version: "0.2.0", reason: "renders a broken deny.toml" }] };
test("a known-bad package version warns with the reason", () => {
  const r = preflight(withUri("0.2.0"), emptyManifest(), { ...base, knownBad: bad });
  expect(r.warnings.join()).toContain("0.2.0 is on the known-bad list: renders a broken deny.toml");
  expect(r.refusals).toEqual([]);
});
test("--strict turns a known-bad package version into a refusal", () => {
  expect(preflight(withUri("0.2.0"), emptyManifest(), { ...base, knownBad: bad, strict: true }).refusals.join()).toContain("known-bad");
});
test("a version that is not on the list passes, even with --strict", () => {
  const r = preflight(withUri("0.3.0"), emptyManifest(), { ...base, knownBad: bad, strict: true });
  expect(r.warnings).toEqual([]);
  expect(r.refusals).toEqual([]);
});

test("a disabled Security builtin is a warning for each one, and an enabled one is silent", () => {
  const c = withUri("0.2.0", { securityBuiltins: [{ name: "cargo-deny", enabled: false }, { name: "secrets-scan", enabled: true }, { name: "codeql", enabled: false }] });
  const r = preflight(c, emptyManifest(), base);
  expect(r.warnings).toEqual(["required Security builtin cargo-deny is disabled", "required Security builtin codeql is disabled"]);
  expect(r.refusals).toEqual([]);
  expect(preflight(c, emptyManifest(), { ...base, strict: true }).refusals).toEqual([]);
});

test("a pre-release runner of the required version is older than the requirement, so a requirement must be a version pre-releases of the next release can meet", () => {
  const r = preflight(withUri("0.2.0", { requires: { runner: "0.1.0" } }), emptyManifest(), { ...base, runnerVersion: "0.1.0-rc.1" });
  expect(r.errors.join()).toContain("runner 0.1.0-rc.1 is older than the 0.1.0 this package requires");
  const next = preflight(withUri("0.2.0", { requires: { runner: "0.1.0" } }), emptyManifest(), { ...base, runnerVersion: "0.1.1-rc.1" });
  expect(next.errors).toEqual([]);
});

// A consumer pins the package with its checksum, so the URI the package reports carries ::sha256:<hash> before the #.
const SUM = "a".repeat(64);
const pinned = (v: string, sum = SUM) => `package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${v}/pantheon-scaffold@${v}::sha256:${sum}#/Config.pkl`;

test("the version is read from a package URI that carries a checksum, pre-releases included", () => {
  expect(packageVersionOf(pinned("0.1.0"))).toBe("0.1.0");
  expect(packageVersionOf(pinned("0.2.0-rc.1"))).toBe("0.2.0-rc.1");
});
test("a checksum that is not 64 hex characters is not accepted as part of the URI", () => {
  expect(() => packageVersionOf(pinned("0.1.0", "xyz"))).toThrow("readable version");
  expect(() => packageVersionOf(pinned("0.1.0", "a".repeat(63)))).toThrow("readable version");
});
test("a render with a checksum-pinned package URI gets its version, the way every real consumer does", () => {
  const r = preflight(withUri("0.1.0", { packageUri: pinned("0.1.0") }), emptyManifest(), base);
  expect(r.errors).toEqual([]);
  expect(r.packageVersion).toBe("0.1.0");
});
