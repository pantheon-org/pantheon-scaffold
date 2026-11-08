// Moves the pinned actions in pkl/Pins.pkl to their latest published release, after checking each one, and writes the freshness record.
// The flag is not called --dry-run because bun itself consumes that one before the script sees it, so a "dry run" would write.
// usage: bun runner/scripts/update-pins.ts [--pins pkl/Pins.pkl] [--no-write] [--freshness-out <file>] [--summary <file>]
// Exit 0: every pin was checked (changes may have been written). Exit 1: a pin failed a check, and nothing is written, the freshness record
// included, so a failing check never looks like a successful one. Exit 2: usage. PANTHEON_SCAFFOLD_GITHUB_API replaces the API address, for tests.
import { readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { applyChanges, githubApi, parsePins, renderFreshness, updatePins } from "../src/pinupdate";

const argv = Bun.argv.slice(2);
const opts = { pins: join(import.meta.dir, "../../pkl/Pins.pkl"), noWrite: false, freshnessOut: undefined as string | undefined, summary: undefined as string | undefined };
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  const value = () => argv[++i] ?? (console.error(`${a} needs a value`), process.exit(2));
  if (a === "--no-write") opts.noWrite = true;
  else if (a === "--pins") opts.pins = resolve(value());
  else if (a === "--freshness-out") opts.freshnessOut = resolve(value());
  else if (a === "--summary") opts.summary = resolve(value());
  else {
    console.error(`unknown option ${a}\nusage: update-pins.ts [--pins <file>] [--no-write] [--freshness-out <file>] [--summary <file>]`);
    process.exit(2);
  }
}

const text = readFileSync(opts.pins, "utf8");
const pins = parsePins(text);
const api = githubApi(process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN, process.env.PANTHEON_SCAFFOLD_GITHUB_API);
const { changes, problems } = await updatePins(pins, api);

console.log(`checked ${pins.length} pin(s)`);
for (const c of changes) console.log(`  ${c.action} ${c.from} -> ${c.to}${c.major ? " (major)" : ""}`);
if (changes.length === 0) console.log("no pin changed");
if (problems.length) {
  for (const p of problems) console.error(`problem with ${p.name}: ${p.message}`);
  console.error(`${problems.length} problem(s): nothing was written, the freshness record included`);
  process.exit(1);
}
if (opts.noWrite) {
  console.log("no-write run: nothing written");
  process.exit(0);
}
if (changes.length) writeFileSync(opts.pins, applyChanges(text, changes));
if (opts.freshnessOut) writeFileSync(opts.freshnessOut, renderFreshness(Date.now()));
if (opts.summary && changes.length) {
  const rows = changes.map((c) => `- \`${c.action}\` ${c.from} to ${c.to}${c.major ? " (**major**, read its release notes)" : ""}`);
  writeFileSync(opts.summary, `Moves the pinned actions to their latest published release. Each commit was checked to belong to the upstream repository and the tag to be a release.\n\n${rows.join("\n")}\n`);
}
