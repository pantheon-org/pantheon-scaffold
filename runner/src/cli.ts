import { join, resolve } from "node:path";
import { apply } from "./apply";
import { ContractError, parseContract } from "./contract";
import { ManifestError } from "./manifest";
import { PathError } from "./paths";
import { evalContractJson } from "./pkl";
import type { Schema } from "./schema";

export const VERSION = "0.0.0";
const SCHEMA_PATH = join(import.meta.dir, "../../contract.schema.json");

// Exit codes: 0 ok, 1 error, 2 usage, 3 refused (a hand edit or an existing file would be overwritten).
export const EXIT = { ok: 0, error: 1, usage: 2, refused: 3 } as const;

const usage = `usage:
  pantheon-scaffold --version
  pantheon-scaffold contract <pantheon.pkl>
  pantheon-scaffold render <pantheon.pkl> [--root <dir>] [--dry-run] [--force] [--repair] [--allow-local-package <dir>]...`;

type Flags = { root: string; dryRun: boolean; force: boolean; repair: boolean; local: string[] };

function parseFlags(args: string[]): Flags | string {
  const f: Flags = { root: process.cwd(), dryRun: false, force: false, repair: false, local: [] };
  for (let i = 0; i < args.length; i++) {
    const a = args[i];
    if (a === "--dry-run") f.dryRun = true;
    else if (a === "--force") f.force = true;
    else if (a === "--repair") f.repair = true;
    else if (a === "--root" || a === "--allow-local-package") {
      const v = args[++i];
      if (!v) return `${a} needs a value`;
      if (a === "--root") f.root = resolve(v);
      else f.local.push(resolve(v));
    } else return `unknown option ${a}`;
  }
  return f;
}

async function loadContract(config: string, root: string, local: string[]) {
  const schema = (await Bun.file(SCHEMA_PATH).json()) as Schema;
  return parseContract(await evalContractJson(resolve(config), root, { allowFilesUnder: local }), schema);
}

export async function main(argv: string[]): Promise<number> {
  const [cmd, config, ...rest] = argv;
  if (cmd === "--version") {
    console.log(VERSION);
    return EXIT.ok;
  }
  if ((cmd === "contract" || cmd === "render") && config) {
    const flags = parseFlags(rest);
    if (typeof flags === "string") {
      console.error(`${flags}\n${usage}`);
      return EXIT.usage;
    }
    try {
      const contract = await loadContract(config, flags.root, flags.local);
      if (cmd === "contract") {
        const { uses, constructs } = contract.escapeHatch;
        console.log(`contract ok: ${contract.files.length} files, ${contract.pins.length} pins, escape hatch ${uses} of ${constructs} constructs`);
        return EXIT.ok;
      }
      const report = await apply(contract, flags.root, { dryRun: flags.dryRun, force: flags.force, repair: flags.repair });
      for (const a of report.actions) console.log(`${a.kind.padEnd(9)} ${a.path}`);
      if (report.refusals.length) {
        console.error(`refused ${report.refusals.length} file(s), nothing was written:\n${report.refusals.map((r) => `  ${r}`).join("\n")}`);
        return flags.repair ? EXIT.ok : EXIT.refused;
      }
      console.log(`${report.dryRun ? "dry run: would write" : "written"} ${report.dryRun ? report.actions.filter((a) => ["create", "update", "seed"].includes(a.kind)).length : report.written} file(s)`);
      return EXIT.ok;
    } catch (e) {
      if (e instanceof ContractError || e instanceof PathError || e instanceof ManifestError) {
        console.error(e.message);
        return EXIT.error;
      }
      throw e;
    }
  }
  console.error(usage);
  return EXIT.usage;
}

if (import.meta.main) process.exit(await main(Bun.argv.slice(2)));
