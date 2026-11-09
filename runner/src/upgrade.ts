import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { checksumAssetUrl, findChecksum } from "./checksums";
import { checkRunnerLock } from "./lockcheck";
import { MANIFEST_PATH, readManifest, serialiseManifest, sha256 } from "./manifest";
import { acquireLock, type LockOptions } from "./lock";
import { RUNNER_TOOL, readPins, writePins, countPackageImports } from "./pins";
import { compareSemver, parseSemver } from "./semver";

export const REPO = "pantheon-org/pantheon-scaffold";
export const RELEASE_BASE = `https://github.com/${REPO}/releases/download`;

export class UpgradeError extends Error {}

/** The three things upgrade reaches outside the repository for. Each can be replaced, which is how the tests run without a release. */
export type Deps = {
  fetchText(url: string): Promise<string>;
  /** Throws unless the file has a valid build provenance attestation from the repository. */
  verifyAttestation(file: string, repo: string): Promise<void>;
  exec(cmd: string[], cwd: string): Promise<number>;
};

export type UpgradeOptions = { config?: string; dryRun?: boolean; force?: boolean; noRender?: boolean; baseUrl?: string; deps?: Partial<Deps>; lock?: LockOptions };
export type UpgradeReport = { from: string; to: string; changes: string[]; dryRun: boolean; renderExit?: number };

const defaultDeps: Deps = {
  async fetchText(url) {
    const res = await fetch(url, { signal: AbortSignal.timeout(15_000) });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return res.text();
  },
  async verifyAttestation(file, repo) {
    // Needs the gh CLI. Not yet exercised against a real release: that happens with the release pipeline (wave D).
    const p = Bun.spawn(["gh", "attestation", "verify", file, "--repo", repo], { stdout: "pipe", stderr: "pipe" });
    const [err, code] = await Promise.all([new Response(p.stderr).text(), p.exited]);
    if (code !== 0) throw new Error(err.trim() || `gh attestation verify exited ${code}`);
  },
  async exec(cmd, cwd) {
    return await Bun.spawn(cmd, { cwd, stdout: "inherit", stderr: "inherit" }).exited;
  },
};

type Backup = { path: string; text: string | null };
const backup = (path: string): Backup => ({ path, text: existsSync(path) ? readFileSync(path, "utf8") : null });
const restore = (b: Backup[]): void => {
  for (const { path, text } of b) {
    if (text === null) rmSync(path, { force: true });
    else writeFileSync(path, text);
  }
};

/**
 * When mise.toml is an owned file whose text still matches the manifest, returns a function that records the text after the pin edit in the
 * manifest, so the render that follows sees no hand edit. A mise.toml that was edited by hand, a seeded one, or no manifest: nothing is
 * recorded, and the render judges the file as it did before.
 */
async function sealedMise(root: string): Promise<() => Promise<void>> {
  const path = join(root, "mise.toml");
  if (!existsSync(path) || !existsSync(join(root, MANIFEST_PATH))) return async () => {};
  const manifest = await readManifest(root);
  const entry = manifest.files["mise.toml"];
  if (entry?.class !== "owned" || entry.hash !== sha256(readFileSync(path, "utf8"))) return async () => {};
  return async () => {
    manifest.files["mise.toml"] = { ...entry, hash: sha256(readFileSync(path, "utf8")) };
    writeFileSync(join(root, MANIFEST_PATH), serialiseManifest(manifest));
  };
}

/** Downloads the release's checksums file and returns the package checksum, after the attestation says the file is genuine. */
async function verifiedChecksum(version: string, base: string, deps: Deps): Promise<string> {
  const url = checksumAssetUrl(base, version);
  let text: string;
  try {
    text = await deps.fetchText(url);
  } catch (e) {
    throw new UpgradeError(`could not fetch ${url}: ${(e as Error).message}`);
  }
  const dir = mkdtempSync(join(tmpdir(), "scaffold-checksums-"));
  try {
    const file = join(dir, "checksums.txt");
    writeFileSync(file, text);
    try {
      await deps.verifyAttestation(file, REPO);
    } catch (e) {
      throw new UpgradeError(`attestation check failed for ${url}: ${(e as Error).message}`);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
  try {
    return findChecksum(text, `pantheon-scaffold@${version}`);
  } catch (e) {
    throw new UpgradeError((e as Error).message);
  }
}

/**
 * `upgrade <version>` (decision 45): rewrites the amends URI, its checksum and the runner pin, refreshes mise, and renders with the
 * new runner. Nothing is written until the release's checksums file has been downloaded and its attestation verified. If mise fails,
 * every file it or this command touched is put back. The version-mismatch rule is not involved here: the new runner checks it.
 */
export async function upgrade(root: string, requested: string, opts: UpgradeOptions = {}): Promise<UpgradeReport> {
  const to = requested.replace(/^v/, "");
  if (!parseSemver(to)) throw new UpgradeError(`${JSON.stringify(requested)} is not a version`);
  const config = opts.config ?? "pantheon.pkl";
  const deps: Deps = { ...defaultDeps, ...opts.deps };
  const pins = readPins(root, config);
  const from = pins.packageVersion;
  if (compareSemver(to, from) === 0) throw new UpgradeError(`already at ${to}`);
  if (compareSemver(to, from) < 0 && !opts.force) throw new UpgradeError(`${to} is older than ${from}, which is pinned now (use --force to go back)`);

  const checksum = await verifiedChecksum(to, opts.baseUrl ?? RELEASE_BASE, deps);
  const imports = countPackageImports(root, config);
  const changes = [`${config}: ${from} -> ${to}, checksum ${checksum.slice(0, 12)}...${imports ? `, ${imports} import${imports === 1 ? "" : "s"} of the package` : ""}`, `mise.toml: ${RUNNER_TOOL} ${pins.runnerVersion} -> ${to}`];
  if (opts.dryRun) return { from, to, changes, dryRun: true };

  const release = acquireLock(root, opts.lock);
  const saved = [backup(join(root, config)), backup(join(root, "mise.toml")), backup(join(root, "mise.lock")), backup(join(root, MANIFEST_PATH))];
  try {
    // The package owns mise.toml when it renders it, so the pin edit below must not look like a hand edit to the render that follows.
    const sealed = await sealedMise(root);
    writePins(root, config, { version: to, checksum });
    for (const step of ["install", "lock"]) {
      const code = await deps.exec(["mise", step], root);
      if (code !== 0) {
        restore(saved);
        throw new UpgradeError(`mise ${step} failed (exit ${code}), the pins and the lock file were put back`);
      }
    }
    // mise locks every platform it knows unless told which, and for one with no runner it picks whatever asset it can.
    const lockPath = join(root, "mise.lock");
    const problems = existsSync(lockPath) ? checkRunnerLock(readFileSync(lockPath, "utf8")) : [];
    if (problems.length) {
      restore(saved);
      throw new UpgradeError(
        `mise lock recorded the wrong runner for some platforms, so the pins and the lock file were put back:\n  ${problems.join("\n  ")}\nLimit the platforms in mise.toml: [settings] lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]`,
      );
    }
    await sealed();
  } finally {
    release();
  }
  if (opts.noRender) return { from, to, changes, dryRun: false };
  const renderExit = await deps.exec(["mise", "x", "--", "pantheon-scaffold", "render", config, "--root", root], root);
  return { from, to, changes, dryRun: false, renderExit };
}
