// The P.1 equality rule (decision 73): two workflows are equal when both parse to equal data with Bun.YAML, comments and
// formatting are ignored, `run:` scripts are compared byte for byte (a parsed string is its exact text), null and {} are
// equal inside the `on:` map, and a duplicate mapping key in either file is a failure.
export type Comparison = { equal: boolean; differences: string[]; duplicateKeys: string[] };

type Json = unknown;

function normalise(doc: Json): Json {
  if (!doc || typeof doc !== "object" || Array.isArray(doc)) return doc;
  const out: Record<string, Json> = { ...(doc as Record<string, Json>) };
  const on = out["on"];
  if (on && typeof on === "object" && !Array.isArray(on)) {
    out["on"] = Object.fromEntries(Object.entries(on as Record<string, Json>).map(([k, v]) => [k, v === null ? {} : v]));
  }
  return out;
}

function diff(a: Json, b: Json, path: string, out: string[]): void {
  if (JSON.stringify(a) === JSON.stringify(b)) return;
  if (a && b && typeof a === "object" && typeof b === "object" && !Array.isArray(a) && !Array.isArray(b)) {
    const keys = new Set([...Object.keys(a as object), ...Object.keys(b as object)]);
    for (const k of keys) diff((a as Record<string, Json>)[k], (b as Record<string, Json>)[k], `${path}.${k}`, out);
    return;
  }
  if (Array.isArray(a) && Array.isArray(b) && a.length === b.length) {
    a.forEach((v, i) => diff(v, b[i], `${path}[${i}]`, out));
    return;
  }
  out.push(`${path}: ${JSON.stringify(a)} vs ${JSON.stringify(b)}`);
}

const BLOCK_START = /^(?<indent>\s*)(?:-\s+)?[^#\n]*:\s*[|>][-+0-9]*\s*(?:#.*)?$/;
const KEY = /^(?:- +)*(?<key>"[^"]*"|'[^']*'|[^\s#:'"{}\[\],&*!|>%@`-][^:#]*?|-[^\s:][^:#]*?):(?:\s|$)/;

/** Block-style duplicate key detection. Flow mappings on one line and block scalar contents are not inspected. */
export function findDuplicateKeys(text: string): string[] {
  const dups: string[] = [];
  const stack: { indent: number; keys: Set<string> }[] = [];
  let blockKeyColumn = -1;
  text.split("\n").forEach((raw, i) => {
    const indent = raw.length - raw.trimStart().length;
    if (blockKeyColumn >= 0) {
      if (raw.trim() === "" || indent > blockKeyColumn) return;
      blockKeyColumn = -1;
    }
    const trimmed = raw.trim();
    if (trimmed === "" || trimmed.startsWith("#") || trimmed === "---") return;
    const block = raw.match(BLOCK_START);
    if (block) {
      const dash = raw.match(/^(\s*)(?:- +)+/);
      blockKeyColumn = dash ? dash[0].length : (block.groups?.indent.length ?? 0);
    }
    const seq = raw.match(/^(\s*)((?:- +)+)/);
    const keyIndent = seq ? seq[1].length + seq[2].length : indent;
    const m = trimmed.match(KEY);
    if (seq) while (stack.length && stack[stack.length - 1].indent >= keyIndent) stack.pop();
    else while (stack.length && stack[stack.length - 1].indent > keyIndent) stack.pop();
    if (!m) return;
    if (!stack.length || stack[stack.length - 1].indent < keyIndent) stack.push({ indent: keyIndent, keys: new Set() });
    const key = m.groups!.key.trim();
    const top = stack[stack.length - 1];
    if (top.keys.has(key)) dups.push(`line ${i + 1}: ${key}`);
    top.keys.add(key);
  });
  return dups;
}

export function compareWorkflows(canonical: string, rendered: string): Comparison {
  const differences: string[] = [];
  diff(normalise(Bun.YAML.parse(canonical)), normalise(Bun.YAML.parse(rendered)), "$", differences);
  const duplicateKeys = [
    ...findDuplicateKeys(canonical).map((d) => `canonical ${d}`),
    ...findDuplicateKeys(rendered).map((d) => `rendered ${d}`),
  ];
  return { equal: differences.length === 0 && duplicateKeys.length === 0, differences, duplicateKeys };
}

// Comments a tool acts on. P.1 ignores comments, so a render that dropped one would pass P.1 and then fail the tool in the real repository.
// Only zizmor's `ignore[...]` is a YAML comment that matters in these workflows. A directive inside a `run:` script is part of the script,
// which P.1 already compares byte for byte, so block scalar contents are skipped here.
const DIRECTIVE = /#\s*zizmor:\s*ignore\[[^\]]*\]/;

/** The lines that carry a directive comment, trimmed and with runs of whitespace collapsed. */
export function directiveComments(text: string): string[] {
  const out: string[] = [];
  let blockKeyColumn = -1;
  for (const raw of text.split("\n")) {
    const indent = raw.length - raw.trimStart().length;
    if (blockKeyColumn >= 0) {
      if (raw.trim() === "" || indent > blockKeyColumn) continue;
      blockKeyColumn = -1;
    }
    const block = raw.match(BLOCK_START);
    if (block) {
      const dash = raw.match(/^(\s*)(?:- +)+/);
      blockKeyColumn = dash ? dash[0].length : (block.groups?.indent.length ?? 0);
    }
    // The renderer quotes the key `on` (it is a YAML 1.1 boolean), so a quoted key is the same line as a bare one.
    if (DIRECTIVE.test(raw)) out.push(raw.trim().replace(/^(['"])([^'"]+)\1:/, "$2:").replace(/\s+/g, " "));
  }
  return out;
}

/** Differences between the directive comments of the canonical file and of the render, as a multiset. */
export function compareDirectives(canonical: string, rendered: string): string[] {
  const want = directiveComments(canonical);
  const got = directiveComments(rendered);
  const left = [...got];
  const out: string[] = [];
  for (const d of want) {
    const i = left.indexOf(d);
    if (i === -1) out.push(`missing in the render: ${d}`);
    else left.splice(i, 1);
  }
  for (const d of left) out.push(`not in the canonical file: ${d}`);
  return out;
}
