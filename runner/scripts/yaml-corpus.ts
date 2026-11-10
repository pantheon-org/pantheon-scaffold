// Writes the shared YAML corpus the Rust core is held to (plan task 2.2: the YAML crate and `Bun.YAML` must give the same
// equality verdict). Every workflow the fixtures render, this repository's own workflows and a list of tricky snippets are
// parsed with Bun.YAML; the Rust tests (crates/scaffold-core/tests/yaml_corpus.rs) must reproduce each result.
// usage: bun runner/scripts/yaml-corpus.ts [--check]
// With --check the file is not written, and the exit code is 1 when it is out of date.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { compareWorkflows } from "../src/equality";
import { evalContractJson } from "../src/pkl";

const root = resolve(import.meta.dir, "../..");
const out = join(root, "crates/scaffold-core/tests/data/yaml-corpus.json");

type Doc = { name: string; text: string };
const docs: Doc[] = [];
const seen = new Set<string>();
const add = (name: string, text: string) => {
  if (seen.has(text)) return;
  seen.add(text);
  docs.push({ name, text });
};

// Tricky snippets: YAML 1.1 and 1.2 differences, scalars, anchors, block scalars, multiple documents.
const SNIPPETS: Record<string, string> = {
  "on-key-bare": "on:\n  push:\n",
  "on-key-quoted": "'on':\n  push: {}\n",
  "yes-no": "a: yes\nb: no\nc: on\nd: off\ne: y\nf: n\n",
  "booleans": "a: true\nb: True\nc: TRUE\nd: false\n",
  "null-forms": "a: ~\nb: null\nc: Null\nd:\ne: ''\n",
  "numbers": "a: 12\nb: -3\nc: +4\nd: 0o17\ne: 0x1f\nf: 017\ng: 1e3\nh: 1.5\ni: .5\nj: 1_000\nk: 0b101\n",
  "special-floats": "a: .inf\nb: -.inf\nc: .nan\n",
  "quoted-numbers": "a: '12'\nb: \"1.5\"\nc: '007'\n",
  "timestamps": "a: 2001-12-14\nb: 2001-12-14t21:59:43.10-05:00\n",
  "version-like": "a: 1.10\nb: 3.12\nc: 20.04\n",
  "anchors": "a: &x\n  k: 1\nb: *x\n",
  "merge-key": "base: &b\n  k: 1\n  j: 2\nd:\n  <<: *b\n  j: 3\n",
  "block-literal-keep": "run: |+\n  one\n\n",
  "block-literal-strip": "run: |-\n  one\n  two\n",
  "block-folded": "run: >\n  one\n  two\n\n  three\n",
  "block-indent-indicator": "run: |2\n    indented\n",
  "multi-document": "a: 1\n---\nb: 2\n",
  "empty": "",
  "only-comment": "# nothing\n",
  "flow": "a: {b: 1, c: [1, 2, {d: e}]}\n",
  "flow-trailing-comma": "a: [1, 2, ]\n",
  "unicode": "name: café ünï ☃ \U0001F600\n",
  "escapes": 'a: "\\u00e9\\t\\n\\x41"\nb: \'it\'\'s\'\n',
  "multiline-plain": "a: one\n  two\n  three\n",
  "multiline-quoted": 'a: "one\n  two"\n',
  "complex-key": "? [a, b]\n: c\n",
  "tags": "a: !!str 12\nb: !!int '12'\nc: !!float 1\n",
  "sequences": "- a\n- - b\n  - c\n- d: e\n  f: g\n",
  "colon-in-value": "a: b: c\n",
  "url": "a: https://example.com/x?y=1#z\n",
  "hash-in-value": "a: b # comment\nc: 'd # not comment'\n",
  "duplicate-key": "a: 1\na: 2\n",
  "tab-after-colon": "a:\t1\n",
  "crlf": "a: 1\r\nb: |\r\n  x\r\n",
  "invalid-flow": "a: [1\n",
  "invalid-indent": "a:\n  b: 1\n c: 2\n",
  "directive": "%YAML 1.2\n---\na: 1\n",
  "document-end": "a: 1\n...\n",
  "large-int": "a: 12345678901234567890\nb: 9007199254740993\n",
  "negative-zero": "a: -0\nb: -0.0\n",
  "on-null-vs-empty": "on:\n  workflow_dispatch:\n  push: {}\n",
  "key-with-spaces": "a key: 1\n'b key': 2\n",
  "empty-values-in-seq": "- \n- ''\n- ~\n",
};
for (const [name, text] of Object.entries(SNIPPETS)) add(`snippet:${name}`, text);

for (const file of readdirSync(join(root, ".github/workflows")).sort()) add(`repo:${file}`, readFileSync(join(root, ".github/workflows", file), "utf8"));
add("fixture:consumer-check.yml", readFileSync(join(root, "runner/test/fixtures/consumer-check.yml"), "utf8"));

const fixtures = readdirSync(join(root, "runner/test/fixtures"), { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort();
for (const name of fixtures) {
  const config = join(root, "runner/test/fixtures", name, "pantheon.pkl");
  if (!existsSync(config)) continue;
  let contract: { files: { path: string; content?: string }[] };
  try {
    contract = (await evalContractJson(config, root, { attempts: 1, allowFilesUnder: [root] })) as typeof contract;
  } catch {
    console.error(`skipped fixture ${name}: pkl could not evaluate it`);
    continue;
  }
  for (const f of contract.files) if (f.content !== undefined && /\.ya?ml$/.test(f.path)) add(`render:${name}:${f.path}`, f.content);
}

const parse = (text: string): { parsed: unknown } | { error: true } => {
  try {
    return { parsed: JSON.parse(JSON.stringify(Bun.YAML.parse(text) ?? null)) };
  } catch {
    return { error: true };
  }
};

// The last letter of a document, changed: a small real difference, usually inside a run script.
const mutate = (text: string): string => text.replace(/[A-Za-z](?=[^A-Za-z]*$)/, (c) => (c === "Z" ? "Y" : "Z"));

type Pair = { a: number; b: number; expected: ReturnType<typeof verdict> };
const verdict = (a: string, b: string) => {
  try {
    return compareWorkflows(a, b);
  } catch {
    return { error: true as const };
  }
};

const entries = docs.map((d) => ({ ...d, ...parse(d.text) }));
const pairs: Pair[] = [];
const extra: Doc[] = [];
const index = (text: string, name: string): number => {
  let i = docs.findIndex((d) => d.text === text);
  if (i === -1) {
    i = docs.length + extra.length;
    extra.push({ name, text });
  }
  return i;
};
docs.forEach((d, i) => {
  pairs.push({ a: i, b: i, expected: verdict(d.text, d.text) });
  const next = docs[(i + 1) % docs.length];
  pairs.push({ a: i, b: index(next.text, next.name), expected: verdict(d.text, next.text) });
  if (d.name.startsWith("render:") || d.name.startsWith("repo:")) {
    const changed = mutate(d.text);
    pairs.push({ a: i, b: index(changed, `${d.name} (last letter changed)`), expected: verdict(d.text, changed) });
  }
});
const all = [...entries, ...extra.map((d) => ({ ...d, ...parse(d.text) }))];

const text = JSON.stringify({ docs: all, pairs }, null, 1) + "\n";
if (Bun.argv.includes("--check")) {
  const current = existsSync(out) ? readFileSync(out, "utf8") : "";
  if (current !== text) {
    console.error("crates/scaffold-core/tests/data/yaml-corpus.json is out of date: run bun runner/scripts/yaml-corpus.ts");
    process.exit(1);
  }
} else {
  writeFileSync(out, text);
  console.log(`${all.length} documents, ${pairs.length} pairs, ${text.length} bytes -> ${out}`);
}
