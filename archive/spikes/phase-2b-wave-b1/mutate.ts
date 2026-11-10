// Mutation check for wave B1: each entry breaks one behaviour, and the suite must then fail. Run from the repo root: bun tmp/phase-2b-wave-b1/mutate.ts
import { $ } from "bun";
const M = [
  ["runner/src/semver.ts", `if (!x.pre.length || !y.pre.length)`, `if (false)`, "a pre-release is lower than its release"],
  ["runner/src/preflight.ts", `if (!uri || !uri.startsWith("package://")) return undefined;`, `if (!uri) return undefined;`, "a local package has no version"],
  ["runner/src/preflight.ts", `if (opts.mode === "render" && !opts.force)`, `if (false)`, "an older package is refused"],
  ["runner/src/preflight.ts", `compareSemver(opts.runnerVersion, need) < 0`, `compareSemver(opts.runnerVersion, need) > 100`, "a runner that is too old is an error"],
  ["runner/src/preflight.ts", `if (opts.strict) out.refusals`, `if (false) out.refusals`, "--strict refuses a known-bad version"],
  ["runner/src/preflight.ts", `if (!b.enabled)`, `if (false)`, "a disabled Security builtin warns"],
  ["runner/src/knownbad.ts", `writeFileSync(cachePath, JSON.stringify(list, null, 2) + "\\n");`, ``, "the fetched list is cached"],
  ["runner/src/knownbad.ts", `if (!res.ok) throw new Error(\`HTTP \${res.status}\`);`, ``, "a bad HTTP status is a failed fetch"],
  ["runner/src/knownbad.ts", `|| !parseSemver(v.version)) return null;`, `) return null;`, "a list entry must be a version"],
  ["runner/src/detach.ts", `if (deletes.length) refusals.push(...deleteGuard(root, deletes));`, ``, "detach uses the delete guard"],
  ["runner/src/detach.ts", `!== entry.hash && !opts.force)`, `!== entry.hash && false)`, "detach refuses a hand-edited workflow"],
  ["runner/src/detach.ts", `["manifest.json", "stage", "cache"]`, `["stage", "cache"]`, "detach removes the manifest"],
  ["runner/src/pkl.ts", `...(opts.cacheDir ? ["--cache-dir", opts.cacheDir] : []),`, ``, "pkl gets the cache directory"],
  ["runner/src/cli.ts", `flags.repair ? emptyManifest() : await readManifest(flags.root)`, `await readManifest(flags.root)`, "--repair does not read the broken manifest"],
  ["runner/src/cli.ts", `if (packageVersionOf(contract.packageUri)) {`, `if (true) {`, "a local package never fetches the list"],
  ["runner/src/apply.ts", `const packageVersion = opts.packageVersion ?? manifest.packageVersion;`, `const packageVersion = opts.packageVersion;`, "the recorded version is kept"],
  ["runner/src/apply.ts", `...(f.removeOnDetach ? { removeOnDetach: true as const } : {}) };
    if (prev?.class`, `};
    if (prev?.class`, "the detach flag is recorded"],
  ["pkl/Config.pkl", `when (definingUri.startsWith("package://")) { packageUri = definingUri }`, `packageUri = definingUri`, "a local package has no packageUri"],
  ["pkl/Config.pkl", `when (f.removeOnDetach) { removeOnDetach = true }`, `removeOnDetach = f.removeOnDetach`, "an unmarked file carries no flag"],
] as const;
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, to));
  const r = await $`cd runner && bun test`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await $`cd runner && bun test`.quiet().nothrow();
console.log(after.exitCode === 0 ? "restored: suite green" : "RESTORE FAILED");
process.exit(missed || after.exitCode ? 1 : 0);
