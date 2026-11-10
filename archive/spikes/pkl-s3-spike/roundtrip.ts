// Reads the real workflows of a repo from GitHub (read-only), strips the comments on pinned uses lines,
// rebuilds them from a pin table taken from the original comments, and compares byte for byte.
// Usage: bun roundtrip.ts [owner/repo]
import { $ } from "bun";
import { appendVersions, findMissingComments, findUnpinned, scan, type Pin } from "./versions";

const repo = Bun.argv[2] ?? "pantheon-org/pantheon-adr";
const names = (await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows --jq '.[].name'`.text()).trim().split("\n");
const files = new Map<string, string>();
for (const n of names) {
  const b64 = (await $`env -u GH_TOKEN gh api repos/${repo}/contents/.github/workflows/${n} --jq .content`.text()).trim();
  files.set(n, Buffer.from(b64, "base64").toString("utf8"));
}

// Pin table from the commented lines.
const pins: Pin[] = [];
for (const text of files.values()) {
  for (const l of scan(text)) {
    const g = l.match?.groups;
    const c = g?.tail.trim();
    if (g && c) {
      const version = c.replace(/^#\s*/, "");
      if (!pins.some((p) => p.action === g.action && p.sha === g.sha)) pins.push({ action: g.action, sha: g.sha, version });
    }
  }
}

let linesWithComment = 0;
let identical = 0;
const uncommented: string[] = [];
const unknown: string[] = [];
for (const [name, original] of files) {
  const stripped = scan(original).map((l) => (l.match?.groups?.tail.trim() ? l.text.replace(/\s+#.*$/, "") : l.text)).join("\n");
  for (const m of findMissingComments(original)) uncommented.push(`${name}:${m.line}  ${m.text}`);
  const known = pins.filter((p) => true);
  // Rebuild only for lines that have a pin; lines whose action is not in the table are reported, not guessed.
  let rebuilt: string;
  try {
    rebuilt = appendVersions(stripped, known);
  } catch (e) {
    unknown.push(`${name}: ${(e as Error).message}`);
    continue;
  }
  const a = original.split("\n");
  const b = rebuilt.split("\n");
  a.forEach((line, i) => {
    const m = scan(line)[0].match?.groups;
    if (m && m.tail.trim()) {
      linesWithComment++;
      if (line === b[i]) identical++;
      else console.log(`DIFF ${name}:${i + 1}\n  orig:    ${line}\n  rebuilt: ${b[i]}`);
    }
  });
}
console.log(`workflows read: ${files.size}, distinct pins with a comment: ${pins.length}`);
console.log(`lines with a comment: ${linesWithComment}, rebuilt byte-identical: ${identical}`);
console.log(`pinned lines with no comment: ${uncommented.length}`);
for (const u of uncommented) console.log(`  MISSING ${u}`);
console.log(`unpinned uses lines: ${files.size ? [...files.entries()].flatMap(([n, t]) => findUnpinned(t).map((m) => `${n}:${m.line}`)).length : 0}`);
for (const u of unknown) console.log(`  UNKNOWN ${u}`);
process.exit(identical === linesWithComment ? 0 : 1);
