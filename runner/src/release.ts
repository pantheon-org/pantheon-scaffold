/** The package version, from the one `version = "..."` line of pkl/PklProject. The rest of the release tooling is `scaffold-release`. */
export function pklProjectVersion(text: string): string {
  const hits = [...text.matchAll(/^[ \t]*version[ \t]*=[ \t]*"([^"]+)"/gm)];
  if (hits.length === 0) throw new Error("PklProject has no version line");
  if (hits.length > 1) throw new Error("PklProject has more than one version line");
  return hits[0][1];
}
