import { assetNames } from "./release";
import { findChecksum, parseChecksums } from "./checksums";
import type { KnownBad } from "./preflight";
import { parseSemver } from "./semver";

/** What the promotion needs to know about a release, as `gh release view` reports it. */
export type ReleaseInfo = { tagName: string; name: string; body: string; isPrerelease: boolean; isDraft: boolean; assets: string[] };

/** Everything the promotion reaches outside itself for. Each can be replaced, which is how the tests run without a release. */
export type PromoteDeps = {
  viewRelease(tag: string): Promise<ReleaseInfo>;
  downloadAsset(tag: string, name: string): Promise<Uint8Array>;
  /** Throws unless the bytes have a valid build provenance attestation from the repository. */
  verifyAttestation(name: string, bytes: Uint8Array): Promise<void>;
  knownBad(): Promise<KnownBad>;
  /** The conclusion of the release workflow run for the tag push, and of each of its jobs. Empty when no run is found. */
  releaseRun(tag: string): Promise<{ conclusion: string; jobs: { name: string; conclusion: string }[] } | undefined>;
  latestTag(): Promise<string | undefined>;
  edit(tag: string, edit: { title: string; notes: string }): Promise<void>;
};

export type Step = { guard: string; ok: boolean; detail: string };
export type PromoteReport = { tag: string; dryRun: boolean; steps: Step[]; promoted: boolean };

export class PromoteError extends Error {
  constructor(message: string, readonly report: PromoteReport) {
    super(message);
  }
}

const sha256 = (bytes: Uint8Array): string => new Bun.CryptoHasher("sha256").update(bytes).digest("hex");
const same = (a: Uint8Array, b: Uint8Array): boolean => a.length === b.length && a.every((x, i) => x === b[i]);

/** The title and notes of a stable release: the pre-release wording of the release workflow, without the word. */
export function stableWording(info: ReleaseInfo): { title: string; notes: string } {
  return { title: info.name.replace(/\s*\(pre-release\)\s*$/, ""), notes: info.body.replace(/^Pre-release of/, "Release of") };
}

/**
 * Promotion of a pre-release (decisions 46, 56, 60 and 90). Every guard runs before anything is edited, and each is recorded. The edit is the
 * flag, the title and the notes: the assets are never touched. After it the same checks run again, and the checksums file must be byte for
 * byte the one that was attested before the edit, which is decision 46's revisit trigger.
 */
export async function promote(tag: string, ref: string, deps: PromoteDeps, opts: { dryRun: boolean }): Promise<PromoteReport> {
  const report: PromoteReport = { tag, dryRun: opts.dryRun, steps: [], promoted: false };
  const fail = (guard: string, detail: string): never => {
    report.steps.push({ guard, ok: false, detail });
    throw new PromoteError(`${guard}: ${detail}`, report);
  };
  const pass = (guard: string, detail: string): void => void report.steps.push({ guard, ok: true, detail });

  if (ref !== "refs/heads/main") fail("run from main", `the workflow ran on ${JSON.stringify(ref)}, a promotion runs only from refs/heads/main`);
  pass("run from main", ref);

  const m = /^v([0-9A-Za-z.-]+)$/.exec(tag);
  if (!m || !parseSemver(m[1])) fail("tag shape", `${JSON.stringify(tag)} is not a release tag (expected v<major>.<minor>.<patch>[-pre-release])`);
  const version = m![1];
  pass("tag shape", `version ${version}`);

  const checks = async (): Promise<Uint8Array> => {
    const info = await deps.viewRelease(tag);
    if (info.isDraft) fail("release state", `${tag} is a draft`);
    const expected = assetNames(version);
    const got = [...info.assets].sort();
    if (JSON.stringify(got) !== JSON.stringify(expected)) fail("assets", `expected ${expected.join(", ")}, found ${got.join(", ")}`);
    const checksumsBytes = await deps.downloadAsset(tag, "checksums.txt");
    await deps.verifyAttestation("checksums.txt", checksumsBytes).catch((e: Error) => fail("attestation", `checksums.txt: ${e.message}`));
    const text = new TextDecoder().decode(checksumsBytes);
    for (const { name } of parseChecksums(text)) {
      if (!info.assets.includes(name)) fail("digests", `checksums.txt lists ${name}, which the release does not carry`);
      const bytes = await deps.downloadAsset(tag, name);
      if (sha256(bytes) !== findChecksum(text, name)) fail("digests", `${name} does not match checksums.txt`);
    }
    return checksumsBytes;
  };

  const before = await deps.viewRelease(tag);
  if (!before.isPrerelease) fail("release state", `${tag} is already a stable release: nothing to promote, and a published version is never re-published`);
  pass("release state", `${tag} is a pre-release`);
  const attested = await checks();
  pass("assets, attestation and digests", `${before.assets.length} assets, checksums.txt attested, every listed digest matches`);

  const bad = (await deps.knownBad()).versions.find((v) => v.version === version);
  if (bad) fail("known bad", `${version} is on the known-bad list: ${bad.reason}`);
  pass("known bad", `${version} is not on the list`);

  const run = await deps.releaseRun(tag);
  if (!run) fail("release run", `no run of the release workflow found for ${tag}`);
  const unsuccessful = [{ name: "run", conclusion: run!.conclusion }, ...run!.jobs].filter((j) => j.conclusion !== "success");
  if (unsuccessful.length) fail("release run", `not successful: ${unsuccessful.map((j) => `${j.name} (${j.conclusion})`).join(", ")}`);
  if (!run!.jobs.some((j) => j.name.startsWith("Verify the published release"))) fail("release run", "the run has no verify-release job");
  pass("release run", `the release workflow succeeded for ${tag}, ${run!.jobs.length} jobs`);

  if (opts.dryRun) return report;

  await deps.edit(tag, stableWording(before));
  const after = await deps.viewRelease(tag);
  if (after.isPrerelease) fail("after the edit", `${tag} is still a pre-release`);
  const afterChecksums = await checks();
  if (!same(attested, afterChecksums)) fail("after the edit", "checksums.txt is not byte for byte the one that was attested before the edit");
  const latest = await deps.latestTag();
  if (latest !== tag) fail("after the edit", `the latest release is ${JSON.stringify(latest)}, expected ${tag}`);
  pass("after the edit", "stable and latest, the assets and checksums.txt are unchanged and still attested");
  report.promoted = true;
  return report;
}
