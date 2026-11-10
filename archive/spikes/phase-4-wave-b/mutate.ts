// Mutation check for Phase 4 wave B: each entry breaks one property of the wave B builtins and the wave B tests must then fail.
// Run from the repository root: bun tmp/phase-4-wave-b/mutate.ts [text]  (with text, only the entries whose description contains it)
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const M: [string, string, string, string][] = [
  [B, `["advanced-security"] = false`, `["advanced-security"] = true`, "zizmor does not upload to code scanning"],
  [B, `when (opts.binary) { ["config"] = ".github/zizmor.yml" }`, `["config"] = ".github/zizmor.yml"`, "zizmor reads a config only when there is a binary"],
  [B, `when (opts.features == "all") { new { name = "Build with default features"`, `when (false) { new { name = "Build with default features"`, "all features adds the default-features build"],
  [B, `when (!opts.gitDependencyIgnore.isEmpty) {\n              ignore =`, `when (true) {\n              ignore =`, "dependabot ignores git dependencies only when listed"],
  [B, `          when (opts.binary) {\n            bareCheckout("Checkout")`, `          when (true) {\n            bareCheckout("Checkout")`, "release-please tags the crate only with a binary"],
  [B, `["cancel-in-progress"] = "\${{ github.event_name == 'pull_request' }}"`, `["cancel-in-progress"] = true`, "agnix cancels in-flight runs only for pull requests"],
  ["pkl/Tools.pkl", `const agnixSha256: String = "3228e882`, `const agnixSha256: String = "4228e882`, "agnix is checked against its pinned digest"],
  ["pkl/Tools.pkl", `const aislop: String = "0.17.0"`, `const aislop: String = "0.18.0"`, "aislop is pinned at the version the repositories use"],
  ["pkl/Pins.pkl", `sha = "949feb2413d6458794dcd2491c4babbbce0c15c1"`, `sha = "949feb2413d6458794dcd2491c4babbbce0c15c2"`, "setup-node is the commit of its release tag"],
  [B, `read(if (opts.compositeAction) "assets/zizmor-composite.yml" else "assets/zizmor.yml")`, `read("assets/zizmor.yml")`, "a composite action gets the zizmor config with its extra entry"],
  [B, `replaceFirst("# agnix lints the bundled skill against`, `replaceFirst("# agnix lints the bundle skill against`, "a repository without a skill gets the other first line"],
  [B, `let (inputs = if (opts.compositeAction) ".github action.yml" else ".github")`, `let (inputs = ".github")`, "zizmor scans action.yml for a composite action"],
  [B, `\`if\` = "github.event_name == 'push' || github.event.pull_request.head.repo.fork == false"`, `\`if\` = "github.event_name == 'push'"`, "agnix skips pull requests from forks"],
  ["pkl/assets/agnix.toml", `target = "ClaudeCode"`, `target = "Claude"`, "the agnix config is byte for byte the repositories' file"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
const only = Bun.argv[2];
for (const [file, from, to, what] of M) {
  if (only && !what.includes(only)) continue;
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, () => to));
  const r = await $`cd runner && bun test test/workflows-b.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
