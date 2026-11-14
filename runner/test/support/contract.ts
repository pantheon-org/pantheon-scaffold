import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { ContractError } from "../../src/contract";

// What the package tests use to read what the package renders: the Rust runner's `contract --json`, which prints the contract the runner would act on.
// The TypeScript runner is retired; these tests no longer evaluate the package with pkl directly, so they keep working when a file's text is produced
// by the runner (templates) and not by pkl.
const root = join(import.meta.dir, "../../..");

export type EvalOptions = {
  /** Absolute folders whose files a local (file:) package may read, for working on a checkout of the package. */
  allowFilesUnder?: string[];
  /** Attempts for a failure that looks like a network or download problem (default 3). */
  attempts?: number;
  /** Where pkl keeps downloaded packages. */
  cacheDir?: string;
};

function newestSource(dir: string): number {
  let newest = 0;
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    if (e.name === "target" || e.name === ".git") continue;
    const p = join(dir, e.name);
    newest = Math.max(newest, e.isDirectory() ? newestSource(p) : e.name.endsWith(".rs") || e.name === "Cargo.toml" ? statSync(p).mtimeMs : 0);
  }
  return newest;
}

let resolved: string | undefined;
/**
 * The runner binary: `SCAFFOLD_BIN`, or `target/debug/pantheon-scaffold` when it exists and is newer than every Rust source. It never falls back to a
 * binary that may have been built from another branch: a missing or stale binary is an error that says how to build it.
 */
export function scaffoldBin(): string {
  if (resolved) return resolved;
  const named = process.env.SCAFFOLD_BIN;
  if (named) {
    if (!existsSync(named)) throw new Error(`SCAFFOLD_BIN names ${named}, which does not exist`);
    return (resolved = named);
  }
  const debug = join(root, "target/debug/pantheon-scaffold");
  if (!existsSync(debug)) throw new Error("the package tests need the Rust runner: run `cargo build -p scaffold-cli` or set SCAFFOLD_BIN");
  if (statSync(debug).mtimeMs < newestSource(join(root, "crates"))) throw new Error("target/debug/pantheon-scaffold is older than the Rust sources: run `cargo build -p scaffold-cli`");
  return (resolved = debug);
}

/** Evaluates a config with the Rust runner and returns the contract. A failure is an Error that carries the runner's message. */
export async function evalContractJson(configPath: string, cwd: string, opts: EvalOptions = {}): Promise<unknown> {
  const args = ["contract", configPath, "--json", ...(opts.cacheDir ? ["--cache-dir", opts.cacheDir] : []), ...(opts.attempts ? ["--attempts", String(opts.attempts)] : []), ...(opts.allowFilesUnder ?? []).flatMap((d) => ["--allow-local-package", d])];
  const p = Bun.spawn([scaffoldBin(), ...args], { cwd, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  if (code !== 0) throw new ContractError(err.trim() || `the runner exited with ${code}`);
  return JSON.parse(out);
}

/** Runs the runner with arguments from a directory; returns its exit code and output. */
export async function runScaffold(args: string[], cwd: string): Promise<{ code: number; out: string; err: string }> {
  const p = Bun.spawn([scaffoldBin(), ...args], { cwd, stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { code, out: out.trim(), err: err.trim() };
}
