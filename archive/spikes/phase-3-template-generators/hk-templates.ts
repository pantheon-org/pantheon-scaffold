const up = (await Bun.file("runner/test/upstream/hk.pkl").text()).split("\n");
const body = (start: string): string[] => {
  const s = up.findIndex((l) => l.startsWith(start));
  if (s < 0) throw new Error("missing " + start);
  const e = up.findIndex((l, i) => i > s && l === "}");
  return up.slice(s + 1, e);
};
const pre = body("local preCommit");
let push = body("local prePush").join("\n");
for (const [a, b] of [["--all-targets --locked -- -D warnings", "--all-targets @@FEATURES@@--locked -- -D warnings"], ["cargo test --workspace --locked", "cargo test --workspace @@FEATURES@@--locked"]] as const) {
  if (!push.includes(a)) throw new Error("missing " + a);
  push = push.replace(a, () => b);
}
const shared = `// Rendered by pantheon-scaffold from pantheon.pkl. Do not edit: the runner owns this file and refuses to overwrite a hand edit.
// The steps of the shared hooks. hk.pkl imports this file and adds the repository's own steps.
// Tools are pinned in mise.toml; HK_MISE=1 runs each hook under \`mise x\`.
module pantheon.hk.shared

import "package://github.com/jdx/hk/releases/download/v@@HK_VERSION@@/hk@@@HK_VERSION@@#/Config.pkl" as hk

/// The steps of the pre-commit hook and of \`hk check\` and \`hk fix\`: fast, staged files only. \`check\` verifies, \`fix\` mutates.
function preCommit(): Mapping<String, hk.Step> = new Mapping<String, hk.Step> {
${pre.join("\n")}
}

/// The steps of the pre-push hook, matching the rust-ci workflow.
function prePush(): Mapping<String, hk.Step> = new Mapping<String, hk.Step> {
${push}
}
`;
await Bun.write("pkl/assets/hk.shared.pkl.tmpl", shared);

const hooksAt = up.findIndex((l) => l.startsWith("hooks {"));
const hooks = up.slice(hooksAt).join("\n").replace("steps = prePush", () => "steps = shared.prePush()").replaceAll("steps = preCommit", () => "steps = shared.preCommit()");
const seed = `amends "package://github.com/jdx/hk/releases/download/v@@HK_VERSION@@/hk@@@HK_VERSION@@#/Config.pkl"
import "package://github.com/jdx/hk/releases/download/v@@HK_VERSION@@/hk@@@HK_VERSION@@#/Builtins.pkl"
import "hk.shared.pkl" as shared

// This file is seeded once by pantheon-scaffold and is yours to edit. The steps every repository shares are in
// hk.shared.pkl, which the runner renders and owns: do not edit that file. To add a step of your own, edit a hook below (a module can
// define \`hooks\` only once), for example in the check hook
//
//   steps { ...shared.preCommit(); ["my-step"] { glob = List("*.txt"); check = "my-check {{files}}" } }
//
// Tools are pinned in mise.toml; HK_MISE=1 runs each hook under \`mise x\`.
// \`check\` is the non-mutating verification (used by \`hk check\` and CI); \`fix\`
// is the mutating variant (used by the pre-commit hook, which stages results).

${hooks}`;
await Bun.write("pkl/assets/hk.pkl.tmpl", seed);
