// Deep-compares two YAML files as parsed data (Bun.YAML) and prints each difference path. Usage: bun compare.ts a.yml b.yml
const [a, b] = Bun.argv.slice(2).map((p) => Bun.YAML.parse(require("node:fs").readFileSync(p, "utf8")));
const diffs: string[] = [];
const walk = (x: unknown, y: unknown, path: string) => {
  if (JSON.stringify(x) === JSON.stringify(y)) return;
  if (x && y && typeof x === "object" && typeof y === "object" && !Array.isArray(x) && !Array.isArray(y)) {
    for (const k of new Set([...Object.keys(x as object), ...Object.keys(y as object)])) walk((x as any)[k], (y as any)[k], `${path}.${k}`);
    return;
  }
  if (Array.isArray(x) && Array.isArray(y) && x.length === y.length) {
    x.forEach((v, i) => walk(v, y[i], `${path}[${i}]`));
    return;
  }
  diffs.push(`${path}: ${JSON.stringify(x)} vs ${JSON.stringify(y)}`);
};
walk(a, b, "$");
console.log(diffs.length ? diffs.join("\n") : "parsed data identical");
process.exit(diffs.length ? 1 : 0);
