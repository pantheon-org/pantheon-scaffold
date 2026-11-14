import { expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const builtins = join(import.meta.dir, "../../pkl/Builtins");
const assets = readdirSync(builtins, { withFileTypes: true })
  .filter((e) => e.isDirectory() && existsSync(join(builtins, e.name, "assets")))
  .flatMap((e) => readdirSync(join(builtins, e.name, "assets")).map((f) => ({ builtin: e.name, file: f, path: join(builtins, e.name, "assets", f) })));

// The one named exception: a seed with comments and an empty [tools] table, so there is no value to pass (docs/templates-variables.md, question 2).
const STATIC_STRUCTURED = ["mise/mise-repo.toml"];

test("every toml, yaml and json asset a builtin ships is a Jinja template, apart from the one named exception", () => {
  const plain = assets
    .filter((a) => /\.(toml|ya?ml|json)$/.test(a.file))
    .map((a) => `${a.builtin}/${a.file}`)
    .filter((name) => !STATIC_STRUCTURED.includes(name));
  expect(plain).toEqual([]);
});

test("no asset still uses an @@NAME@@ placeholder", () => {
  const found = assets.filter((a) => /@@[A-Z_]+@@/.test(readFileSync(a.path, "utf8"))).map((a) => `${a.builtin}/${a.file}`);
  expect(found).toEqual([]);
});

test("no builtin patches rendered text with replaceAll or replaceFirst: a variable and a conditional do that in the template", () => {
  const modules = readdirSync(builtins, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => join(builtins, e.name, "Builtin.pkl")).filter(existsSync);
  const found = modules.filter((p) => /\.replace(All|First)\(/.test(readFileSync(p, "utf8")));
  expect(found).toEqual([]);
});
