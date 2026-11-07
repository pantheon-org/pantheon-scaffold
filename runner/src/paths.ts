import { existsSync, lstatSync, readlinkSync, realpathSync } from "node:fs";
import { dirname, isAbsolute, join, normalize, relative, resolve, sep } from "node:path";

export class PathError extends Error {}

const outside = (rootReal: string, p: string): boolean => {
  const r = relative(rootReal, p);
  return r === ".." || r.startsWith(`..${sep}`) || isAbsolute(r);
};

function isLink(p: string): boolean {
  try {
    return lstatSync(p).isSymbolicLink();
  } catch {
    return false;
  }
}

/** Where a path really points. A dangling symlink resolves to its target as written. */
function realTarget(p: string): string {
  try {
    return realpathSync(p);
  } catch {
    return isLink(p) ? resolve(dirname(p), readlinkSync(p)) : p;
  }
}

/** Resolves a contract path to an absolute path inside `root`, or throws. The root must exist. */
export function resolveInside(root: string, rel: string): string {
  if (rel === "" || rel.includes("\0")) throw new PathError(`${JSON.stringify(rel)}: empty or invalid path`);
  if (isAbsolute(rel) || rel.startsWith("~")) throw new PathError(`${rel}: absolute paths are not allowed`);
  if (rel.includes("\\")) throw new PathError(`${rel}: backslashes are not allowed`);
  const clean = normalize(rel);
  if (clean === ".." || clean.split(sep).includes("..")) throw new PathError(`${rel}: escapes the repository`);
  const rootReal = realpathSync(root);
  const abs = join(rootReal, clean);
  // The deepest existing ancestor (or the link itself) must resolve, through symlinks, to somewhere inside the root.
  let probe = abs;
  while (!existsSync(probe) && !isLink(probe)) probe = dirname(probe);
  if (outside(rootReal, realTarget(probe))) {
    throw new PathError(isLink(probe) && probe === abs ? `${rel}: is a symlink that points outside the repository` : `${rel}: resolves outside the repository through a symlink`);
  }
  return abs;
}

/** Two paths that differ only by case would collide on a case-insensitive filesystem. */
export function findCaseCollisions(paths: string[]): string[] {
  const seen = new Map<string, string>();
  const out: string[] = [];
  for (const p of paths) {
    const key = p.toLowerCase();
    const prev = seen.get(key);
    if (prev !== undefined && prev !== p) out.push(`${prev} and ${p} differ only by case`);
    else seen.set(key, p);
  }
  return out;
}
