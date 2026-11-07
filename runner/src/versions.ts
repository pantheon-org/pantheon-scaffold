// Appends "# <version>" to pinned `uses: owner/repo[/path]@<40 hex>` lines in rendered workflow text.
import type { Pin } from "./contract";

const USES = /^(?<lead>\s*(?:-\s+)?uses:\s+)(?<q>['"]?)(?<action>[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(?:\/[^@\s'"]+)?)@(?<sha>[0-9a-f]{40})\k<q>(?<tail>\s*(?:#.*)?)$/;
const UNPINNED = /^\s*(?:-\s+)?uses:\s+['"]?(?<action>[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+(?:\/[^@\s'"]+)?)@(?<ref>[^\s'"#]+)/;
const BLOCK_START = /^(?<indent>\s*)(?:-\s+)?[^#\n]*:\s*[|>][-+0-9]*\s*(?:#.*)?$/;

export type Line = { index: number; text: string; match: RegExpMatchArray | null };

// Walks the lines, skipping the content of block scalars (run: | ...), which can hold text that looks like a uses line.
export function scan(text: string): Line[] {
  const lines = text.split("\n");
  const out: Line[] = [];
  let blockKeyColumn = -1;
  lines.forEach((raw, index) => {
    const indent = raw.length - raw.trimStart().length;
    if (blockKeyColumn >= 0) {
      if (raw.trim() === "" || indent > blockKeyColumn) {
        out.push({ index, text: raw, match: null });
        return;
      }
      blockKeyColumn = -1;
    }
    const block = raw.match(BLOCK_START);
    if (block) {
      const dash = raw.match(/^(\s*)-\s+/);
      blockKeyColumn = dash ? dash[0].length : (block.groups?.indent.length ?? 0);
    }
    out.push({ index, text: raw, match: raw.match(USES) });
  });
  return out;
}

export function appendVersions(text: string, pins: Pin[]): string {
  const lines = scan(text);
  const result: string[] = [];
  for (const line of lines) {
    const g = line.match?.groups;
    if (!g) {
      result.push(line.text);
      continue;
    }
    const pin = pins.find((p) => p.action === g.action && p.sha === g.sha);
    if (!pin) throw new Error(`line ${line.index + 1}: no version known for ${g.action}@${g.sha}`);
    const comment = g.tail.trim();
    if (comment === "") {
      result.push(`${g.lead}${g.q}${g.action}@${g.sha}${g.q} # ${pin.version}`);
    } else if (comment === `# ${pin.version}`) {
      result.push(line.text);
    } else {
      throw new Error(`line ${line.index + 1}: comment "${comment}" differs from "# ${pin.version}" for ${g.action}`);
    }
  }
  return result.join("\n");
}

export function findMissingComments(text: string): { line: number; text: string }[] {
  return scan(text)
    .filter((l) => l.match?.groups && l.match.groups.tail.trim() === "")
    .map((l) => ({ line: l.index + 1, text: l.text.trim() }));
}

export function findUnpinned(text: string): { line: number; text: string }[] {
  return scan(text)
    .filter((l) => !l.match && UNPINNED.test(l.text) && !/@[0-9a-f]{40}/.test(l.text))
    .map((l) => ({ line: l.index + 1, text: l.text.trim() }));
}
