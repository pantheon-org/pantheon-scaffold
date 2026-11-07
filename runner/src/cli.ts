import { join } from "node:path";
import { parseContract, ContractError } from "./contract";
import { evalContractJson } from "./pkl";
import type { Schema } from "./schema";

export const VERSION = "0.0.0";
const SCHEMA_PATH = join(import.meta.dir, "../../contract.schema.json");

const usage = "usage: pantheon-scaffold --version | contract <pantheon.pkl>";

export async function main(argv: string[]): Promise<number> {
  const [cmd, config] = argv;
  if (cmd === "--version") {
    console.log(VERSION);
    return 0;
  }
  if (cmd === "contract" && config) {
    try {
      const schema = (await Bun.file(SCHEMA_PATH).json()) as Schema;
      const contract = parseContract(await evalContractJson(config, process.cwd()), schema);
      const { uses, constructs } = contract.escapeHatch;
      console.log(`contract ok: ${contract.files.length} files, ${contract.pins.length} pins, escape hatch ${uses} of ${constructs} constructs`);
      return 0;
    } catch (e) {
      if (e instanceof ContractError) {
        console.error(e.message);
        return 1;
      }
      throw e;
    }
  }
  console.error(usage);
  return 2;
}

if (import.meta.main) process.exit(await main(Bun.argv.slice(2)));
