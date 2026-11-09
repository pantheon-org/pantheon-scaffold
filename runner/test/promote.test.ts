import { expect, test } from "bun:test";
import { formatChecksums } from "../src/checksums";
import { PromoteError, promote, stableWording, type PromoteDeps, type ReleaseInfo } from "../src/promote";
import { assetNames } from "../src/release";

const TAG = "v0.1.5";
const VERSION = "0.1.5";
const enc = (s: string): Uint8Array => new TextEncoder().encode(s);
const sha = (b: Uint8Array): string => new Bun.CryptoHasher("sha256").update(b).digest("hex");

type World = {
  release: ReleaseInfo;
  files: Map<string, Uint8Array>;
  badAttestation?: string;
  knownBad: { version: string; reason: string }[];
  run: { conclusion: string; jobs: { name: string; conclusion: string }[] } | undefined;
  latest: string | undefined;
  /** What the edit does to the world, so a test can make it go wrong. */
  onEdit?: (w: World) => void;
  edits: { title: string; notes: string }[];
};

/** A release as the release workflow publishes it: the files, a checksums.txt that matches them, and the pre-release wording. */
function world(over: Partial<World> = {}): World {
  const names = assetNames(VERSION);
  const files = new Map<string, Uint8Array>();
  for (const n of names.filter((n) => n !== "checksums.txt")) files.set(n, enc(`bytes of ${n}`));
  const listed = names.filter((n) => n !== "checksums.txt" && !n.endsWith(".sha256"));
  files.set("checksums.txt", enc(formatChecksums(listed.map((n) => ({ name: n, sha256: sha(files.get(n)!) })))));
  return {
    release: { tagName: TAG, name: `${TAG} (pre-release)`, body: "Pre-release of the package and the runner, released together.", isPrerelease: true, isDraft: false, assets: names },
    files,
    knownBad: [],
    run: { conclusion: "success", jobs: [{ name: "Attest and publish", conclusion: "success" }, { name: "Verify the published release on ubuntu-latest", conclusion: "success" }] },
    latest: "v0.1.4",
    edits: [],
    ...over,
  };
}

function deps(w: World): PromoteDeps {
  return {
    viewRelease: async () => ({ ...w.release }),
    downloadAsset: async (_t, name) => {
      const b = w.files.get(name);
      if (!b) throw new Error(`no asset ${name}`);
      return b;
    },
    verifyAttestation: async (name) => {
      if (w.badAttestation === name) throw new Error("no valid attestation");
    },
    knownBad: async () => ({ versions: w.knownBad }),
    releaseRun: async () => w.run,
    latestTag: async () => w.latest,
    edit: async (_t, e) => {
      w.edits.push(e);
      w.release = { ...w.release, isPrerelease: false, name: e.title, body: e.notes };
      w.latest = TAG;
      w.onEdit?.(w);
    },
  };
}

const MAIN = "refs/heads/main";
const run = (w: World, opts: { dryRun?: boolean; ref?: string; tag?: string } = {}) => promote(opts.tag ?? TAG, opts.ref ?? MAIN, deps(w), { dryRun: opts.dryRun ?? false });
const refuses = async (w: World, guard: string, opts: { dryRun?: boolean; ref?: string; tag?: string } = {}): Promise<PromoteError> => {
  const e = await run(w, opts).then(() => undefined, (x: unknown) => x);
  expect(e).toBeInstanceOf(PromoteError);
  expect((e as PromoteError).message.startsWith(`${guard}:`), (e as Error).message).toBe(true);
  return e as PromoteError;
};

test("a dry run passes every guard and edits nothing", async () => {
  const w = world();
  const r = await run(w, { dryRun: true });
  expect(r.promoted).toBe(false);
  expect(r.steps.every((s) => s.ok)).toBe(true);
  expect(r.steps.map((s) => s.guard)).toEqual(["run from main", "tag shape", "release state", "assets, attestation and digests", "known bad", "release run"]);
  expect(w.edits).toEqual([]);
  expect(w.release.isPrerelease).toBe(true);
});

test("a promotion edits the flag, the title and the notes once, and checks the result", async () => {
  const w = world();
  const r = await run(w);
  expect(r.promoted).toBe(true);
  expect(w.edits).toEqual([{ title: TAG, notes: "Release of the package and the runner, released together." }]);
  expect(r.steps.at(-1)).toMatchObject({ guard: "after the edit", ok: true });
  expect(w.release.isPrerelease).toBe(false);
});

test("it runs only from main", async () => {
  const w = world();
  await refuses(w, "run from main", { ref: "refs/heads/feat/x" });
  await refuses(w, "run from main", { ref: "refs/tags/v0.1.5" });
  expect(w.edits).toEqual([]);
});

test("a tag that is not a release tag is refused", async () => {
  for (const tag of ["0.1.5", "v0.1", "main", "v0.1.5; rm -rf"]) await refuses(world(), "tag shape", { tag });
});

test("a release that is already stable, or a draft, is not promoted", async () => {
  const stable = world();
  stable.release.isPrerelease = false;
  await refuses(stable, "release state");
  const draft = world();
  draft.release.isDraft = true;
  await refuses(draft, "release state");
  expect(stable.edits.length + draft.edits.length).toBe(0);
});

test("the release must carry exactly the files a release carries", async () => {
  const missing = world();
  missing.release.assets = missing.release.assets.filter((n) => n !== "pantheon-scaffold-linux-x64");
  await refuses(missing, "assets");
  const extra = world();
  extra.release.assets = [...extra.release.assets, "surprise.bin"];
  await refuses(extra, "assets");
  expect(missing.edits.length + extra.edits.length).toBe(0);
});

test("a checksums file without a valid attestation stops the promotion", async () => {
  const w = world({ badAttestation: "checksums.txt" });
  const e = await refuses(w, "attestation");
  expect(e.report.steps.at(-1)).toMatchObject({ ok: false });
  expect(w.edits).toEqual([]);
});

test("an asset whose bytes do not match checksums.txt stops the promotion", async () => {
  const w = world();
  w.files.set("pantheon-scaffold-darwin-arm64", enc("tampered"));
  await refuses(w, "digests");
  expect(w.edits).toEqual([]);
});

test("a version on the known-bad list is refused with its reason", async () => {
  const w = world({ knownBad: [{ version: VERSION, reason: "it breaks upgrade" }] });
  const e = await refuses(w, "known bad");
  expect(e.message).toContain("it breaks upgrade");
  expect(w.edits).toEqual([]);
  const other = world({ knownBad: [{ version: "0.1.4", reason: "x" }] });
  await run(other, { dryRun: true });
});

test("the release workflow must have succeeded for the tag, verify job included", async () => {
  await refuses(world({ run: undefined }), "release run");
  await refuses(world({ run: { conclusion: "failure", jobs: [{ name: "Verify the published release on x", conclusion: "success" }] } }), "release run");
  await refuses(world({ run: { conclusion: "success", jobs: [{ name: "Verify the published release on x", conclusion: "failure" }] } }), "release run");
  const noVerify = world({ run: { conclusion: "success", jobs: [{ name: "Attest and publish", conclusion: "success" }] } });
  const e = await refuses(noVerify, "release run");
  expect(e.message).toContain("no verify-release job");
  expect(noVerify.edits).toEqual([]);
});

test("after the edit, a release that is still a pre-release, not latest, or whose checksums changed is reported as a failure", async () => {
  const still = world();
  still.onEdit = (x) => (x.release = { ...x.release, isPrerelease: true });
  await refuses(still, "after the edit");
  const notLatest = world();
  notLatest.onEdit = (x) => (x.latest = "v0.9.9");
  await refuses(notLatest, "after the edit");
  const changed = world();
  changed.onEdit = (x) => x.files.set("checksums.txt", enc(new TextDecoder().decode(x.files.get("checksums.txt")!) + "\n"));
  await refuses(changed, "after the edit");
});

test("the stable wording drops the pre-release label and nothing else", () => {
  const info: ReleaseInfo = { tagName: TAG, name: "v0.1.5 (pre-release)", body: "Pre-release of the package. checksums.txt lists every file.", isPrerelease: true, isDraft: false, assets: [] };
  expect(stableWording(info)).toEqual({ title: "v0.1.5", notes: "Release of the package. checksums.txt lists every file." });
  expect(stableWording({ ...info, name: "v0.1.5", body: "Notes." })).toEqual({ title: "v0.1.5", notes: "Notes." });
});
