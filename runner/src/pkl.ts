import { ContractError } from "./contract";

/** Flags that restrict what the consumer's pantheon.pkl may load or read (task 2.3, Trust). */
export const ALLOWED_MODULES = "pkl:,file:,package:,projectpackage:";
// package: lets a package module read its own assets, https: lets pkl download the package (spike S.1).
// There is no file: here, so a pantheon.pkl cannot read arbitrary files.
export const ALLOWED_RESOURCES = "package:,projectpackage:,https:";

export type EvalOptions = {
  /** Absolute folders whose files a local (file:) package may read, for working on a checkout of the package. */
  allowFilesUnder?: string[];
  /** Attempts for a failure that looks like a network or download problem (default 3). Other failures are not retried. */
  attempts?: number;
  /** Milliseconds to wait between attempts (default 2000). */
  delayMs?: number;
  /** Environment for the pkl process, for tests. */
  env?: Record<string, string>;
};

const escapeRegex = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export function allowedResources(opts: EvalOptions = {}): string {
  const local = (opts.allowFilesUnder ?? []).map((dir) => `^file://${escapeRegex(dir.endsWith("/") ? dir : dir + "/")}`);
  return [ALLOWED_RESOURCES, ...local].join(",");
}

/** A failure worth retrying: the package or a pin could not be fetched. A newly created release can answer 404 for a while (spike S.1). */
export const looksLikeFetchFailure = (stderr: string): boolean =>
  /Exception when making request|Cannot download package|Received unexpected status code\s*`?(404|429|5\d\d)|Unable to (resolve|connect)|timed out|Connection (refused|reset)/i.test(stderr);

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function once(configPath: string, cwd: string, opts: EvalOptions): Promise<{ out: string; err: string; code: number }> {
  const proc = Bun.spawn(
    ["pkl", "eval", "-f", "json", "--allowed-modules", ALLOWED_MODULES, "--allowed-resources", allowedResources(opts), configPath],
    { cwd, stdout: "pipe", stderr: "pipe", env: opts.env ? { ...process.env, ...opts.env } : process.env },
  );
  const [out, err, code] = await Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]);
  return { out, err, code };
}

/** Fails closed: after the attempts are used up a fetch problem is an error, never a pass. */
export async function evalContractJson(configPath: string, cwd: string, opts: EvalOptions = {}): Promise<unknown> {
  const attempts = Math.max(1, opts.attempts ?? 3);
  let last = { out: "", err: "", code: 0 };
  for (let i = 1; i <= attempts; i++) {
    last = await once(configPath, cwd, opts);
    if (last.code === 0) break;
    if (!looksLikeFetchFailure(last.err) || i === attempts) {
      const tries = i > 1 ? ` after ${i} attempts` : "";
      throw new ContractError(`pkl eval failed (exit ${last.code})${tries}${looksLikeFetchFailure(last.err) ? ", failing closed" : ""}:\n${last.err.trim()}`);
    }
    await sleep(opts.delayMs ?? 2000);
  }
  try {
    return JSON.parse(last.out);
  } catch {
    throw new ContractError("pkl eval did not print JSON");
  }
}
