// Builds the package from a tree's pkl/ folder, serves it locally in place of GitHub (as runner/test/package-uri.test.ts does), renders the full
// composition through the package URI for a few option sets, and compares each with the folder render of the same tree.
//
//   command bun packaged.ts <tree root> <binary> <work dir>
//
// Checks: (1) every file under pkl/ is in the package zip, (2) the packaged contract equals the folder contract (packageUri and requires aside),
// so `read()` of a submodule's assets and the types of Config.* work from a package URI. This is a local zip, not the published package:
// the true package:// path is proven only by verify-release at the next release.
import { mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, join, relative, resolve } from "node:path";

const [treeArg, binaryArg, workArg] = Bun.argv.slice(2);
if (!treeArg || !binaryArg || !workArg) {
  console.error("usage: bun packaged.ts <tree root> <binary> <work dir>");
  process.exit(2);
}
const tree = resolve(treeArg);
const binary = resolve(binaryArg);
const work = resolve(workArg);
rmSync(work, { recursive: true, force: true });
mkdirSync(work, { recursive: true });

const build = Bun.spawnSync(["pkl", "project", "package", join(tree, "pkl"), "--output-path", join(work, "pkg")], { stdout: "pipe", stderr: "pipe" });
if (build.exitCode !== 0) {
  console.error(build.stderr.toString());
  process.exit(1);
}
const project = readFileSync(join(tree, "pkl/PklProject"), "utf8");
const version = /version = "([^"]+)"/.exec(project)![1]!;
const zip = join(work, "pkg", `pantheon-scaffold@${version}.zip`);

// (1) The zip lists every file of pkl/.
const walk = (dir: string): string[] => readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? walk(join(dir, e.name)) : [relative(join(tree, "pkl"), join(dir, e.name))]));
const onDisk = walk(join(tree, "pkl")).filter((p) => p !== "PklProject" && p !== "PklProject.deps.json").sort();
const listed = Bun.spawnSync(["unzip", "-Z1", zip]).stdout.toString().split("\n").filter((l) => l && !l.endsWith("/")).sort();
const missing = onDisk.filter((p) => !listed.includes(p));
const extra = listed.filter((p) => !onDisk.includes(p));
console.log(`zip: ${listed.length} entries, ${onDisk.length} files on disk, ${missing.length} missing from the zip, ${extra.length} extra`);
if (missing.length) console.log(`  missing: ${missing.join(", ")}`);

// (2) Packaged contract equals folder contract.
const base = `package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${version}/pantheon-scaffold@${version}`;
const server = Bun.serve({ port: 0, fetch: (req) => new Response(Bun.file(join(work, "pkg", decodeURIComponent(basename(new URL(req.url).pathname))))) });
const sets: Record<string, string> = {
  "skills-binary-composite-all": `skillName = "demo-skill"\n  crateName = "demo"\n  skills = true\n  binary = true\n  compositeAction = true\n  features = "all"`,
  "library": `skills = false\n  binary = false`,
};
let problems = 0;
try {
  for (const [name, body] of Object.entries(sets)) {
    const dir = join(work, name);
    mkdirSync(dir, { recursive: true });
    const common = `toolchain = "1.96.1"\n  runnerVersion = "0.1.9"\n  ${body}`;
    const composition = `workflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles()\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n`;
    writeFileSync(join(dir, "pantheon.pkl"), `amends "${base}#/Config.pkl"\nimport "${base}#/Builtins.pkl"\noptions {\n  ${common}\n}\n${composition}`);
    writeFileSync(join(dir, "folder.pkl"), `amends "file://${tree}/pkl/Config.pkl"\nimport "file://${tree}/pkl/Builtins.pkl"\noptions {\n  ${common}\n}\n${composition}`);
    const p = Bun.spawn(
      ["pkl", "eval", "-f", "json", "--cache-dir", join(dir, "cache"), "--http-rewrite", `https://github.com/=http://localhost:${server.port}/`, "--allowed-resources", "package:,https:,http:", join(dir, "pantheon.pkl")],
      { stdout: "pipe", stderr: "pipe" },
    );
    const [packaged, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
    if (code !== 0) {
      console.log(`${name}: packaged render FAILED\n${err}`);
      problems++;
      continue;
    }
    const f = Bun.spawnSync(["pkl", "eval", "-f", "json", "--allowed-resources", `file:`, join(dir, "folder.pkl")], { stdout: "pipe", stderr: "pipe" });
    if (f.exitCode !== 0) {
      console.log(`${name}: folder render FAILED\n${f.stderr.toString()}`);
      problems++;
      continue;
    }
    const a = JSON.parse(packaged), b = JSON.parse(f.stdout.toString());
    delete a.packageUri;
    delete b.packageUri;
    const same = JSON.stringify(a) === JSON.stringify(b);
    console.log(`${name}: packaged contract ${same ? "equals" : "DIFFERS from"} the folder contract (${a.files?.length} files, ${Object.keys(a.workflows ?? {}).length} workflows)`);
    if (!same) problems++;
  }
} finally {
  server.stop(true);
}
if (missing.length || problems) process.exit(1);
