// Mutation check for Phase 5 wave B: each entry breaks one property of the parameterised files and phase5-parameterised.test.ts must then fail.
// Run from the repository root: bun tmp/phase-5-wave-b/mutate.ts [text]
import { $ } from "bun";

const B = "pkl/Builtins.pkl";
const T = "pkl/Tools.pkl";
const M: [string, string, string, string][] = [
  [B, `    when (opts.compositeAction) { "action.yml" }\n`, ``, "the composite variant watches action.yml"],
  [B, `read("assets/tag-release.sh.tmpl").text.replaceAll("@@CRATE@@", opts.crateName!!)\n      executable = true`, `read("assets/tag-release.sh.tmpl").text.replaceAll("@@CRATE@@", opts.crateName!!)\n      executable = false`, "tag-release.sh is executable"],
  [B, `read("assets/tag-release.sh.tmpl").text.replaceAll("@@CRATE@@", opts.crateName!!)`, `read("assets/tag-release.sh.tmpl").text.replaceAll("@@CRATE@@", "adr")`, "tag-release.sh names the repository's crate"],
  [B, `    \`class\` = "seed"\n    content =\n      if (opts.binary)`, `    content =\n      if (opts.binary)`, "SECURITY.md is seeded once"],
  [B, `.replaceAll("@@SKILL_LINE@@\\n", if (opts.skills) "- The bundled \`\\(opts.skillName!!)\` skill and its scripts.\\n" else "")`, `.replaceAll("@@SKILL_LINE@@\\n", "")`, "SECURITY.md names the bundled skill when skills is on"],
  [B, `      else read("assets/security-library.md").text`, `      else read("assets/security-binary.md.tmpl").text.replaceAll("@@CRATE@@", "x").replaceAll("@@SKILL_LINE@@\\n", "")`, "the library repository gets the library SECURITY.md"],
  [B, `text.replaceAll("@@SKILL@@", opts.skillName!!)`, `text.replaceAll("@@SKILL@@", "adr-creator")`, ".coderabbit.yaml names the repository's skill"],
  [B, `        text.replaceAll("    # Covered by the Audit Skill workflow; avoid a second opinion on the same diff.\\n    - \\"!skills/@@SKILL@@/**\\"\\n", "")`, `        text.replaceAll("@@SKILL@@", "none")`, ".coderabbit.yaml has no skill line without skills"],
  [T, `const skillAuditorSha256: String = "538863c9`, `const skillAuditorSha256: String = "638863c9`, "audit-skill.yml checks the auditor binary against its digest"],
  [T, `const skillAuditorAction: String = "a71472ec078f54ff13b5b74b7e3786779ef28df9"`, `const skillAuditorAction: String = "a71472ec078f54ff13b5b74b7e3786779ef28df8"`, "audit-skill.yml pins the v0.3.1 commit"],
  [T, `-v0.3.0/pantheon-skill-auditor-x86_64`, `-v0.3.1/pantheon-skill-auditor-x86_64`, "the bootstrap binary is the tekhne v0.3.0 one"],
  [B, `                A|A+) ;;`, `                A) ;;`, "the grade gate passes A and A+"],
  [B, `\`timeout-minutes\` = 5\n        steps {\n          checkout\n          new {\n            name = "Install pantheon-skill-auditor"`, `\`timeout-minutes\` = 6\n        steps {\n          checkout\n          new {\n            name = "Install pantheon-skill-auditor"`, "the audit job has its five minute timeout"],
  ["pkl/assets/coderabbit.yaml.tmpl", `    - "!target/**"\n`, ``, ".coderabbit.yaml is the repositories' file"],
  ["pkl/assets/security-library.md", `Acknowledgement of your report within 5 working days.`, `Acknowledgement of your report within 7 working days.`, "the library SECURITY.md is the repository's file"],
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
  const r = await $`cd runner && bun test test/phase5-parameterised.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
