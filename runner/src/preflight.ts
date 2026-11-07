import type { Contract } from "./contract";
import type { Manifest } from "./manifest";
import { compareSemver, parseSemver } from "./semver";

export type KnownBad = { versions: { version: string; reason: string }[] };
export type PreflightOptions = { mode: "render" | "check"; runnerVersion: string; force?: boolean; strict?: boolean; knownBad?: KnownBad };
export type Preflight = { packageVersion?: string; errors: string[]; refusals: string[]; warnings: string[] };

/** The version the package was loaded as, read from its package:// URI. A local package has none. */
export function packageVersionOf(uri: string | undefined): string | undefined {
  if (!uri || !uri.startsWith("package://")) return undefined;
  // A consumer pins the package with its checksum, so the URI can read ...@0.1.0::sha256:<64 hex>#/Config.pkl.
  const version = /@([^@#/:]+)(?:::sha256:[0-9a-f]{64})?#/.exec(uri)?.[1];
  if (!version || !parseSemver(version)) throw new Error(`${uri}: the package URI has no readable version`);
  return version;
}

/**
 * The rules checked before anything is rendered: the runner is new enough for the package, the package is not older than the
 * one that last rendered this repo, the package is not on the known-bad list, and no required Security builtin is disabled.
 * errors stop the run (exit 1), refusals are exit 3, warnings are printed.
 */
export function preflight(contract: Contract, manifest: Manifest, opts: PreflightOptions): Preflight {
  const out: Preflight = { errors: [], refusals: [], warnings: [] };
  try {
    out.packageVersion = packageVersionOf(contract.packageUri);
  } catch (e) {
    out.errors.push((e as Error).message);
    return out;
  }
  const version = out.packageVersion;

  const need = contract.requires?.runner;
  if (need !== undefined) {
    if (!parseSemver(need)) out.errors.push(`the package requires runner ${JSON.stringify(need)}, which is not a version`);
    else if (compareSemver(opts.runnerVersion, need) < 0) out.errors.push(`runner ${opts.runnerVersion} is older than the ${need} this package requires, upgrade the runner`);
  }

  const last = manifest.packageVersion;
  if (version !== undefined && last !== undefined) {
    if (!parseSemver(last)) out.errors.push(`the manifest records packageVersion ${JSON.stringify(last)}, which is not a version`);
    else if (compareSemver(version, last) < 0) {
      const msg = `package ${version} is older than ${last}, which rendered this repository last`;
      if (opts.mode === "render" && !opts.force) out.refusals.push(`${msg} (use --force to render an older package)`);
      else out.warnings.push(msg);
    }
  }

  const listed = version === undefined ? undefined : opts.knownBad?.versions.find((v) => v.version === version);
  if (listed) {
    const msg = `package ${version} is on the known-bad list: ${listed.reason}`;
    if (opts.strict) out.refusals.push(`${msg} (--strict)`);
    else out.warnings.push(msg);
  }

  for (const b of contract.securityBuiltins ?? []) if (!b.enabled) out.warnings.push(`required Security builtin ${b.name} is disabled`);
  return out;
}
