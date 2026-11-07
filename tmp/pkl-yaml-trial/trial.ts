// Trial of Bun.YAML against the official YAML test suite, scored on the YAML features real workflows use.
// Usage: bun trial.ts <suite-dir> [results.json]   (suite-dir: unpacked yaml/yaml-test-suite data branch)
import { $ } from "bun";
import { existsSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const suite = Bun.argv[2];
if (!suite || !existsSync(suite)) throw new Error("usage: bun trial.ts <suite-dir> [results.json]");

// 1. Features used by the real workflows of the two proof repos (read-only gh api).
const REPOS = ["pantheon-org/pantheon-adr", "pantheon-org/pantheon-skill-auditor"];
const real = new Map<string, string>();
for (const repo of REPOS) {
  const names = (await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows --jq '.[].name'`.text()).trim().split("\n");
  for (const n of names) {
    real.set(`${repo}/${n}`, Buffer.from((await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows/${n} --jq .content`.text()).trim(), "base64").toString("utf8"));
  }
}
const FEATURES: Record<string, RegExp> = {
  "block scalar | or >": /[:-]\s*[|>][-+0-9]*\s*$/m,
  "flow sequence or map": /(:|-)\s*[\[{]/,
  "double-quoted string": /"[^"\n]*"/,
  "single-quoted string": /'[^'\n]*'/,
  "anchor &": /(:|-)\s+&[A-Za-z0-9_-]+/,
  "alias *": /(:|-)\s+\*[A-Za-z][A-Za-z0-9_-]*/,
  "merge key <<": /^\s*<<:/m,
  "tag !": /(:|-)\s+!{1,2}[A-Za-z<]/,
  "extra document ---": /\n---\s*$/m,
  "complex key ?": /^\s*\?\s/m,
};
const used: Record<string, number> = {};
for (const [k, re] of Object.entries(FEATURES)) used[k] = [...real.values()].filter((t) => re.test(t)).length;
const usesAnchorsOrTags = used["anchor &"] + used["alias *"] + used["merge key <<"] + used["tag !"] > 0;

// 2. The suite.
type Case = { id: string; dir: string };
const cases: Case[] = [];
for (const id of readdirSync(suite).sort()) {
  const dir = join(suite, id);
  if (!statSync(dir).isDirectory() || id.startsWith(".")) continue;
  if (existsSync(join(dir, "in.yaml"))) cases.push({ id, dir });
  else for (const sub of readdirSync(dir).sort()) if (statSync(join(dir, sub)).isDirectory() && existsSync(join(dir, sub, "in.yaml"))) cases.push({ id: `${id}/${sub}`, dir: join(dir, sub) });
}

function splitJsonStream(src: string): unknown[] {
  const out: unknown[] = [];
  let i = 0;
  while (i < src.length) {
    while (i < src.length && /\s/.test(src[i])) i++;
    if (i >= src.length) break;
    let depth = 0, inStr = false, j = i;
    for (; j < src.length; j++) {
      const c = src[j];
      if (inStr) { if (c === "\\") j++; else if (c === '"') { inStr = false; if (depth === 0) { j++; break; } } continue; }
      if (c === '"') inStr = true;
      else if (c === "{" || c === "[") depth++;
      else if (c === "}" || c === "]") { depth--; if (depth === 0) { j++; break; } }
      else if (depth === 0 && /\s/.test(c)) break;
    }
    out.push(JSON.parse(src.slice(i, j)));
    i = j;
  }
  return out;
}

type Status = "pass" | "wrong-result" | "unexpected-error" | "invalid-accepted" | "invalid-rejected" | "no-expected-parsed" | "no-expected-threw";
const results: { id: string; name: string; status: Status; workflowStyle: boolean; detail?: string }[] = [];
for (const c of cases) {
  const read = (f: string) => (existsSync(join(c.dir, f)) ? readFileSync(join(c.dir, f), "utf8") : null);
  const yaml = read("in.yaml")!;
  const name = (read("===") ?? "").trim();
  const events = read("test.event") ?? "";
  const invalid = existsSync(join(c.dir, "error"));
  const docs = (events.match(/^\+DOC/gm) ?? []).length;
  const hasTag = /<[^>\n]+>/.test(events);
  const hasAnchorOrAlias = /&\S|=ALI|\*\S/.test(events);
  const complexKey = /^\?(\s|$)/m.test(yaml);
  const workflowStyle = !invalid && docs === 1 && !hasTag && !complexKey && (usesAnchorsOrTags || !hasAnchorOrAlias);

  let parsed: unknown, threw: string | null = null;
  try { parsed = Bun.YAML.parse(yaml); } catch (e) { threw = (e as Error).message.slice(0, 80); }

  let status: Status, detail: string | undefined;
  if (invalid) status = threw ? "invalid-rejected" : "invalid-accepted";
  else {
    const json = read("in.json");
    if (json === null) status = threw ? "no-expected-threw" : "no-expected-parsed";
    else if (threw) { status = "unexpected-error"; detail = threw; }
    else {
      const expected = splitJsonStream(json);
      const want = expected.length === 0 ? null : docs <= 1 ? expected[0] : expected;
      const got = parsed;
      status = JSON.stringify(sortKeys(got)) === JSON.stringify(sortKeys(want)) ? "pass" : "wrong-result";
      if (status === "wrong-result") detail = `got ${JSON.stringify(got).slice(0, 60)} want ${JSON.stringify(want).slice(0, 60)}`;
    }
  }
  results.push({ id: c.id, name, status, workflowStyle, detail });
}
function sortKeys(v: unknown): unknown {
  if (Array.isArray(v)) return v.map(sortKeys);
  if (v && typeof v === "object") return Object.fromEntries(Object.entries(v as object).sort(([a], [b]) => a.localeCompare(b)).map(([k, x]) => [k, sortKeys(x)]));
  return v;
}

const count = (rs: typeof results, s: Status) => rs.filter((r) => r.status === s).length;
const sub = results.filter((r) => r.workflowStyle);
console.log(`bun ${Bun.version}; suite cases: ${results.length}`);
console.log("real workflow features (files using each, of " + real.size + "):", JSON.stringify(used));
console.log(`ALL    valid with expected JSON: pass ${count(results, "pass")}, wrong ${count(results, "wrong-result")}, error ${count(results, "unexpected-error")}; invalid: rejected ${count(results, "invalid-rejected")}, accepted ${count(results, "invalid-accepted")}; no expected JSON: parsed ${count(results, "no-expected-parsed")}, threw ${count(results, "no-expected-threw")}`);
console.log(`SUBSET workflow-style valid cases: ${sub.length}: pass ${count(sub, "pass")}, wrong ${count(sub, "wrong-result")}, error ${count(sub, "unexpected-error")}, no expected JSON parsed ${count(sub, "no-expected-parsed")}, threw ${count(sub, "no-expected-threw")}`);
for (const r of results.filter((x) => ["wrong-result", "unexpected-error", "invalid-accepted"].includes(x.status))) console.log(`  ${r.status.padEnd(16)} ${r.workflowStyle ? "[subset]" : "        "} ${r.id.padEnd(8)} ${r.name.slice(0, 50)} ${r.detail ?? ""}`);
writeFileSync(Bun.argv[3] ?? "results.json", JSON.stringify({ bun: Bun.version, used, results }, null, 2));
