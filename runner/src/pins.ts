import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseSemver } from "./semver";

/** The mise tool key the runner binary is installed under. Confirmed against the published binary in wave C. */
export const RUNNER_TOOL = "github:pantheon-org/pantheon-scaffold";

export class PinError extends Error {}

export type Pins = { packageVersion: string; checksum?: string; runnerVersion: string };
export type NewPins = { version: string; checksum: string };

// All pin locations live in this file, so a consumer that keeps the pin somewhere else (a PklProject) changes one place (decision 45).
const AMENDS = /(amends "package:\/\/github\.com\/pantheon-org\/pantheon-scaffold\/releases\/download\/v)([^/"]+)(\/pantheon-scaffold@)([^:#"]+)(?:::sha256:([0-9a-f]{64}))?(#\/Config\.pkl")/g;
const TOOL_STRING = new RegExp(`^(\\s*"${RUNNER_TOOL}"\\s*=\\s*")([^"]+)(")`, "gm");
// The section form mise writes when a tool has options: [tools."..."] followed by lines that do not open another section, one of them the version.
const TOOL_SECTION = new RegExp(`^(\\[tools\\."${RUNNER_TOOL}"\\][ \\t]*\\n(?:(?!\\[)[^\\n]*\\n)*?[ \\t]*version[ \\t]*=[ \\t]*")([^"]+)(")`, "gm");
const TOOL_TABLE = new RegExp(`^(\\s*"${RUNNER_TOOL}"\\s*=\\s*\\{[^}\\n]*?\\bversion\\s*=\\s*")([^"]+)(")`, "gm");

const readText = (root: string, file: string): string => {
  const path = join(root, file);
  if (!existsSync(path)) throw new PinError(`${file} not found in ${root}`);
  return readFileSync(path, "utf8");
};

function only(text: string, pattern: RegExp, what: string, file: string): RegExpExecArray {
  const hits = [...text.matchAll(pattern)];
  if (hits.length === 0) throw new PinError(`${file}: ${what}`);
  if (hits.length > 1) throw new PinError(`${file}: more than one ${what.replace(/^no /, "")}, the runner will not guess which to change`);
  return hits[0];
}

function toolMatch(mise: string): RegExpExecArray {
  const hits = [...mise.matchAll(TOOL_STRING), ...mise.matchAll(TOOL_TABLE), ...mise.matchAll(TOOL_SECTION)];
  if (hits.length === 0) throw new PinError(`mise.toml: no ${RUNNER_TOOL} entry with a version`);
  if (hits.length > 1) throw new PinError(`mise.toml: more than one ${RUNNER_TOOL} entry, the runner will not guess which to change`);
  return hits[0];
}

export function readPins(root: string, config: string): Pins {
  const a = only(readText(root, config), AMENDS, "no amends line for the pantheon-scaffold package", config);
  const tool = toolMatch(readText(root, "mise.toml"));
  return { packageVersion: a[4], ...(a[5] ? { checksum: a[5] } : {}), runnerVersion: tool[2] };
}

/** Rewrites both files in memory first, and writes either only when both rewrites worked. */
export function writePins(root: string, config: string, next: NewPins): void {
  if (!parseSemver(next.version)) throw new PinError(`${JSON.stringify(next.version)} is not a version`);
  if (!/^[0-9a-f]{64}$/.test(next.checksum)) throw new PinError("the checksum is not 64 hex characters");
  const pkl = readText(root, config);
  const mise = readText(root, "mise.toml");
  only(pkl, AMENDS, "no amends line for the pantheon-scaffold package", config);
  const tool = toolMatch(mise);
  const newPkl = pkl.replace(AMENDS, (_m, p1, _v, p3, _v2, _s, p6) => `${p1}${next.version}${p3}${next.version}::sha256:${next.checksum}${p6}`);
  const newMise = mise.slice(0, tool.index) + tool[0].replace(tool[2], next.version) + mise.slice(tool.index + tool[0].length);
  writeFileSync(join(root, config), newPkl);
  writeFileSync(join(root, "mise.toml"), newMise);
}
