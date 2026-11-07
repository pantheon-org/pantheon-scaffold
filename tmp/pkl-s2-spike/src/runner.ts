// Minimal spike runner: --version, and render <module.pkl> <outfile> which runs pkl and writes the "content" field.
import { $ } from "bun";
import { writeFileSync } from "node:fs";

const VERSION = "0.0.1";
const [cmd, moduleArg, outArg] = Bun.argv.slice(2);

if (cmd === "--version") {
  console.log(VERSION);
} else if (cmd === "render" && moduleArg && outArg) {
  const json = await $`pkl eval -f json ${moduleArg}`.quiet().text();
  const doc = JSON.parse(json) as { content: string };
  writeFileSync(outArg, doc.content);
  console.log(`rendered ${outArg}`);
} else {
  console.error("usage: pantheon-runner --version | render <module.pkl> <outfile>");
  process.exit(2);
}
