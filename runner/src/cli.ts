import { join, resolve } from "node:path";
import { apply } from "./apply";
import { check } from "./check";
import contractSchema from "../../contract.schema.json";
import { ContractError, parseContract, type Contract } from "./contract";
import { detach } from "./detach";
import { FRESHNESS_URL, FreshnessError, MAX_AGE_DAYS, checkFreshness } from "./freshness";
import { loadKnownBad } from "./knownbad";
import { LatestError, fetchReleases, latestVersion } from "./latest";
import { LockError } from "./lock";
import { ManifestError, emptyManifest, readManifest } from "./manifest";
import { PathError } from "./paths";
import { PinError } from "./pins";
import { evalContractJson } from "./pkl";
import { packageVersionOf, preflight, type KnownBad } from "./preflight";
import type { Schema } from "./schema";
import { UpgradeError, upgrade } from "./upgrade";
import { VERSION } from "./version";

export { VERSION };

// Exit codes: 0 ok, 1 error, 2 usage, 3 refused (a hand edit, an existing file, the delete guard, an older package or --strict), 4 drift (check found a difference).
export const EXIT = { ok: 0, error: 1, usage: 2, refused: 3, drift: 4 } as const;

const usage = `usage (exit codes: 0 ok, 1 error, 2 usage, 3 refused, 4 drift; --dry-run exits 3 when the render would be refused, otherwise 0):
  pantheon-scaffold --version
  pantheon-scaffold contract <pantheon.pkl>
  pantheon-scaffold render <pantheon.pkl> [--root <dir>] [--dry-run] [--force] [--repair] [--strict] [--cache-dir <dir>] [--allow-local-package <dir>]... [--attempts <n>]
  pantheon-scaffold check <pantheon.pkl> [--root <dir>] [--strict] [--cache-dir <dir>] [--allow-local-package <dir>]... [--attempts <n>]
  pantheon-scaffold detach [--root <dir>] [--dry-run] [--force]
  pantheon-scaffold latest [--include-prereleases] [--url <url>] [--root <dir>] [--attempts <n>]
  pantheon-scaffold freshness [--url <url>] [--max-age-days <n>] [--attempts <n>]
  pantheon-scaffold upgrade <version> [--root <dir>] [--config <file>] [--dry-run] [--force] [--no-render]`;

type Flags = { includePrereleases: boolean; url?: string; maxAgeDays: number; root: string; dryRun: boolean; force: boolean; repair: boolean; strict: boolean; noRender: boolean; config?: string; cacheDir?: string; local: string[]; attempts: number };

function parseFlags(args: string[]): Flags | string {
  const f: Flags = { includePrereleases: false, maxAgeDays: MAX_AGE_DAYS, root: process.cwd(), dryRun: false, force: false, repair: false, strict: false, noRender: false, local: [], attempts: 3 };
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--dry-run") f.dryRun = true;
    else if (a === "--force") f.force = true;
    else if (a === "--repair") f.repair = true;
    else if (a === "--strict") f.strict = true;
    else if (a === "--no-render") f.noRender = true;
    else if (a === "--include-prereleases") f.includePrereleases = true;
    else if (a === "--max-age-days") {
      const n = Number(args[++i]);
      if (!Number.isInteger(n) || n < 1) return "--max-age-days needs a whole number of at least 1";
      f.maxAgeDays = n;
    } else if (a === "--url") {
      const v = args[++i];
      if (!v) return "--url needs a value";
      f.url = v;
    } else if (a === "--attempts") {
      const n = Number(args[++i]);
      if (!Number.isInteger(n) || n < 1) return "--attempts needs a whole number of at least 1";
      f.attempts = n;
    } else if (a === "--root" || a === "--allow-local-package" || a === "--cache-dir" || a === "--config") {
      const v = args[++i];
      if (!v) return `${a} needs a value`;
      if (a === "--root") f.root = resolve(v);
      else if (a === "--cache-dir") f.cacheDir = resolve(v);
      else if (a === "--config") f.config = v;
      else f.local.push(resolve(v));
    } else return `unknown option ${a}`;
  }
  return f;
}

async function loadContract(config: string, flags: Flags) {
  const schema = contractSchema as Schema;
  const cacheDir = flags.cacheDir ?? join(flags.root, ".pkl-gen/cache");
  return parseContract(await evalContractJson(resolve(config), flags.root, { allowFilesUnder: flags.local, attempts: flags.attempts, cacheDir }), schema);
}

/** The rules checked before anything is rendered or compared. Returns an exit code to stop with, or the package version to record. */
async function gate(contract: Contract, flags: Flags, mode: "render" | "check"): Promise<{ stop?: number; packageVersion?: string }> {
  let knownBad: KnownBad | undefined;
  try {
    if (packageVersionOf(contract.packageUri)) {
      const kb = await loadKnownBad(flags.root, { url: process.env.PANTHEON_SCAFFOLD_KNOWN_BAD_URL });
      if (kb.note) console.error(`warning: ${kb.note}`);
      knownBad = kb.list;
    }
  } catch (e) {
    console.error((e as Error).message);
    return { stop: EXIT.error };
  }
  // --repair exists to fix a manifest that cannot be read, so it must not read it here.
  const manifest = flags.repair ? emptyManifest() : await readManifest(flags.root);
  const pre = preflight(contract, manifest, { mode, runnerVersion: VERSION, force: flags.force, strict: flags.strict, knownBad });
  for (const w of pre.warnings) console.error(`warning: ${w}`);
  if (pre.errors.length) {
    console.error(pre.errors.join("\n"));
    return { stop: EXIT.error };
  }
  if (pre.refusals.length) {
    console.error(`refused, nothing was written:\n${pre.refusals.map((r) => `  ${r}`).join("\n")}`);
    return { stop: EXIT.refused };
  }
  return { packageVersion: pre.packageVersion };
}

function runCheck(contract: Contract, flags: Flags): number {
  const { drift, checked } = check(contract, flags.root);
  if (drift.length) {
    console.error(`drift in ${drift.length} of ${checked} owned file(s):\n${drift.map((d) => `  ${d.path}: ${d.reason}${d.diff ? "\n" + d.diff : ""}`).join("\n")}`);
    return EXIT.drift;
  }
  console.log(`check ok: ${checked} owned file(s) match the render`);
  return EXIT.ok;
}

async function runRender(contract: Contract, flags: Flags, packageVersion: string | undefined): Promise<number> {
  const report = await apply(contract, flags.root, { dryRun: flags.dryRun, force: flags.force, repair: flags.repair, packageVersion });
  for (const a of report.actions) console.log(`${a.kind.padEnd(9)} ${a.path}`);
  if (report.refusals.length) {
    console.error(`refused ${report.refusals.length} file(s), nothing was written:\n${report.refusals.map((r) => `  ${r}`).join("\n")}`);
    return flags.repair ? EXIT.ok : EXIT.refused;
  }
  const count = (kinds: string[]) => report.actions.filter((a) => kinds.includes(a.kind)).length;
  console.log(
    report.dryRun
      ? `dry run: would write ${count(["create", "update", "seed"])} file(s), delete ${count(["delete"])}`
      : `written ${report.written} file(s), deleted ${report.deleted}`,
  );
  return EXIT.ok;
}

async function runDetach(flags: Flags): Promise<number> {
  const report = await detach(flags.root, { dryRun: flags.dryRun, force: flags.force });
  for (const a of report.actions) console.log(`${a.kind.padEnd(9)} ${a.path}`);
  if (report.refusals.length) {
    console.error(`refused, nothing was changed:\n${report.refusals.map((r) => `  ${r}`).join("\n")}`);
    return EXIT.refused;
  }
  const deleted = report.actions.filter((a) => a.kind === "delete").length;
  console.log(report.dryRun ? `dry run: would delete ${deleted} file(s) and remove the manifest` : `detached: deleted ${deleted} file(s) and removed the manifest, every other file is a plain file now`);
  return EXIT.ok;
}

async function runUpgrade(version: string, flags: Flags): Promise<number> {
  const r = await upgrade(flags.root, version, { config: flags.config, dryRun: flags.dryRun, force: flags.force, noRender: flags.noRender, baseUrl: process.env.PANTHEON_SCAFFOLD_RELEASE_BASE });
  for (const c of r.changes) console.log(`${r.dryRun ? "would change" : "changed"}  ${c}`);
  if (r.dryRun) {
    console.log("dry run: the release was verified, nothing was changed");
    return EXIT.ok;
  }
  if (r.renderExit) {
    console.error(`upgrade: the pins are updated, but the render exited ${r.renderExit}. Fix what it reports and run render again.`);
    return r.renderExit;
  }
  console.log(`upgraded ${r.from} -> ${r.to}${r.renderExit === undefined ? ", run render with the new runner" : ""}`);
  return EXIT.ok;
}

async function runLatest(flags: Flags): Promise<number> {
  const token = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;
  const releases = await fetchReleases({ url: flags.url, token, attempts: flags.attempts });
  const knownBad = (await loadKnownBad(flags.root, { url: process.env.PANTHEON_SCAFFOLD_KNOWN_BAD_URL })).list;
  const version = latestVersion(releases, { includePrereleases: flags.includePrereleases, knownBad });
  if (version === null) {
    console.error(`no release found that qualifies (stable, not a draft, not known bad)${flags.includePrereleases ? "" : ". Use --include-prereleases to offer pre-releases"}`);
    return EXIT.error;
  }
  console.log(version);
  return EXIT.ok;
}

async function runFreshness(flags: Flags): Promise<number> {
  const url = flags.url ?? process.env.PANTHEON_SCAFFOLD_FRESHNESS_URL ?? FRESHNESS_URL;
  const r = await checkFreshness({ url, maxAgeDays: flags.maxAgeDays, attempts: flags.attempts });
  if (r.state === "stale") {
    console.error(`stale: the last successful pin check was ${r.ageDays} day(s) ago (${r.checkedAt}), more than ${flags.maxAgeDays}. The pin-update workflow has stopped running or failing.`);
    return EXIT.refused;
  }
  console.log(`fresh: the last successful pin check was ${r.ageDays} day(s) ago (${r.checkedAt})`);
  return EXIT.ok;
}

const isKnownError = (e: unknown): boolean =>
  e instanceof FreshnessError || e instanceof LatestError || e instanceof ContractError || e instanceof PathError || e instanceof ManifestError || e instanceof LockError || e instanceof PinError || e instanceof UpgradeError;

export async function main(argv: string[]): Promise<number> {
  const [cmd, ...rest] = argv;
  if (cmd === "--version") {
    console.log(VERSION);
    return EXIT.ok;
  }
  const isDetach = cmd === "detach" || cmd === "freshness" || cmd === "latest";
  const config = isDetach ? undefined : rest[0];
  if (!isDetach && !((cmd === "contract" || cmd === "render" || cmd === "check" || cmd === "upgrade") && config)) {
    console.error(usage);
    return EXIT.usage;
  }
  const flags = parseFlags(isDetach ? rest : rest.slice(1));
  if (typeof flags === "string") {
    console.error(`${flags}\n${usage}`);
    return EXIT.usage;
  }
  try {
    if (cmd === "freshness") return await runFreshness(flags);
    if (cmd === "latest") return await runLatest(flags);
    if (isDetach) return await runDetach(flags);
    if (cmd === "upgrade") return await runUpgrade(config!, flags);
    const contract = await loadContract(config!, flags);
    if (cmd === "contract") {
      const { uses, constructs } = contract.escapeHatch;
      console.log(`contract ok: ${contract.files.length} files, ${contract.pins.length} pins, escape hatch ${uses} of ${constructs} constructs`);
      return EXIT.ok;
    }
    const gated = await gate(contract, flags, cmd as "render" | "check");
    if (gated.stop !== undefined) return gated.stop;
    return cmd === "check" ? runCheck(contract, flags) : await runRender(contract, flags, gated.packageVersion);
  } catch (e) {
    if (isKnownError(e)) {
      console.error((e as Error).message);
      return EXIT.error;
    }
    throw e;
  }
}

if (import.meta.main) process.exit(await main(Bun.argv.slice(2)));
