import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// release-bump.yml and release-bump.sh: after a verified release they open and merge the version bump. The workflow properties are read from
// the file; the script runs against a stand-in `gh` that records its calls.
const root = join(import.meta.dir, "../..");
const workflowText = readFileSync(join(root, ".github/workflows/release-bump.yml"), "utf8");
const script = join(root, ".github/scripts/release-bump.sh");
type Step = { name?: string; uses?: string; run?: string; env?: Record<string, string>; with?: Record<string, unknown> };
const wf = Bun.YAML.parse(workflowText) as { on: Record<string, any>; permissions: unknown; jobs: Record<string, { if?: string; permissions: Record<string, string>; steps: Step[] }> };
const job = wf.jobs.bump;

test("it runs only after the release workflow completed, and only for a successful tag push", () => {
  expect(Object.keys(wf.on)).toEqual(["workflow_run"]);
  expect(wf.on.workflow_run).toEqual({ workflows: ["release"], types: ["completed"] });
  expect(job.if).toContain("workflow_run.conclusion == 'success'");
  expect(job.if).toContain("workflow_run.event == 'push'");
  expect(workflowText).not.toContain("pull_request");
});

test("nothing has permissions by default; the job reads contents, and the App token can write only contents and pull requests", () => {
  expect(wf.permissions).toEqual({});
  expect(job.permissions).toEqual({ contents: "read" });
  const mint = job.steps.find((s) => s.uses?.startsWith("actions/create-github-app-token@"))!;
  expect(Object.keys(mint.with ?? {}).filter((k) => k.startsWith("permission-")).sort()).toEqual(["permission-contents", "permission-pull-requests"]);
});

test("it checks out main without credentials, pins every action by SHA, and passes event text through env only", () => {
  const checkout = job.steps.find((s) => s.uses?.startsWith("actions/checkout@"))!;
  expect(checkout.with).toMatchObject({ ref: "main", "persist-credentials": false });
  for (const s of job.steps.filter((s) => s.uses)) expect(s.uses, s.uses).toMatch(/@[0-9a-f]{40}( |$)/);
  for (const s of job.steps.filter((s) => s.run)) expect(s.run).not.toMatch(/\$\{\{/);
  const run = job.steps.find((s) => s.run)!;
  expect(run.env?.TAG).toBe("${{ github.event.workflow_run.head_branch }}");
  expect(run.env?.TAG_SHA).toBe("${{ github.event.workflow_run.head_sha }}");
});

type Outcome = { code: number; calls: string; stderr: string };

function runScript(o: { tag: string; tagSha?: string; headSha?: string; fileVersion: string; checks?: string; openPr?: boolean; path?: string; fileText?: string; failPrCreate?: boolean }): Outcome {
  const dir = mkdtempSync(join(tmpdir(), "release-bump-"));
  const path = o.path ?? "pkl/PklProject";
  mkdirSync(join(dir, path, ".."), { recursive: true });
  const file = o.fileText ?? `@ModuleInfo { minPklVersion = "0.32.0" }\npackage {\n  version = "${o.fileVersion}" // x-release-please-version\n}\n`;
  writeFileSync(join(dir, path), file);
  // The script is rendered for pkl/PklProject; a test of another path renders it again with that path.
  const scriptPath = o.path ? join(dir, "release-bump.sh") : script;
  if (o.path) writeFileSync(scriptPath, readFileSync(script, "utf8").replace("version_file='pkl/PklProject'", `version_file='${o.path}'`));
  const log = join(dir, "calls.log");
  const b64 = Buffer.from(file).toString("base64");
  const gh = join(dir, "gh");
  writeFileSync(
    gh,
    `#!/usr/bin/env bash
echo "$*" >> "${log}"
case "$1 $2" in
  "api repos/o/r/git/ref/tags/"*) echo '{"object":{"sha":"${o.tagSha ?? "abc123"}","type":"commit"}}' ;;
  "api repos/o/r/git/ref/heads/main") echo "mainsha" ;;
  "api repos/o/r/contents/"*"?ref="*) echo '{"sha":"blob1","content":"${b64}"}' ;;
  "pr list") ${o.openPr ? "echo 41" : "true"} ;;
  "pr create") ${o.failPrCreate ? "exit 1" : 'echo "https://github.com/o/r/pull/50"'} ;;
  "pr checks") echo '${o.checks ?? '["pass","pass"]'}' ;;
esac
exit 0
`,
  );
  chmodSync(gh, 0o755);
  const p = Bun.spawnSync(["bash", scriptPath], {
    env: { PATH: `${dir}:${process.env.PATH}`, GH_REPO: "o/r", TAG: o.tag, TAG_SHA: o.headSha ?? "abc123", POLL_SECONDS: "0", MAX_POLLS: "3", GITHUB_STEP_SUMMARY: join(dir, "summary") },
    cwd: dir,
  });
  let calls = "";
  try {
    calls = readFileSync(log, "utf8");
  } catch {}
  return { code: p.exitCode ?? -1, calls, stderr: p.stderr.toString() };
}

test("v0.3.0 released and main still at 0.3.0: the next patch is put in a branch, a pull request is opened, and it is merged when checks pass", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0" });
  expect(r.code).toBe(0);
  expect(r.calls).toContain("refs/heads/chore/start-0.3.1");
  const put = r.calls.split("\n").find((c) => c.includes("--method PUT"))!;
  expect(put).toContain("branch=chore/start-0.3.1");
  const content = put.match(/content=(\S+)/)![1]!;
  expect(Buffer.from(content, "base64").toString()).toContain('version = "0.3.1" // x-release-please-version');
  expect(r.calls).toContain("pr create");
  expect(r.calls).toContain("--title chore(pkl): start 0.3.1");
  expect(r.calls).toContain("pr merge https://github.com/o/r/pull/50 --squash --delete-branch");
});

test("the patch carries over two digits: v1.2.9 gives 1.2.10", () => {
  const r = runScript({ tag: "v1.2.9", fileVersion: "1.2.9" });
  expect(r.code).toBe(0);
  expect(r.calls).toContain("chore/start-1.2.10");
});

test("a failing check leaves the pull request open and fails the run", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", checks: '["pass","fail"]' });
  expect(r.code).toBe(1);
  expect(r.stderr).toContain("stays open");
  expect(r.calls).not.toContain("pr merge");
});

test("no check yet is not green: it waits, then gives up without merging", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", checks: "[]" });
  expect(r.code).toBe(1);
  expect(r.stderr).toContain("did not finish");
  expect(r.calls).not.toContain("pr merge");
});

test("main already above the released version: a visible notice and no branch", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.1" });
  expect(r.code).toBe(0);
  expect(r.calls).not.toContain("git/refs");
  expect(r.calls).not.toContain("pr create");
});

test("main below the released version, a tag that is not the run's commit, and a pre-release tag are each stopped or skipped without a branch", () => {
  const below = runScript({ tag: "v0.3.0", fileVersion: "0.2.9" });
  expect(below.code).toBe(1);
  const spoof = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", headSha: "other" });
  expect(spoof.code).toBe(1);
  expect(spoof.stderr).toContain("No bump");
  const rc = runScript({ tag: "v0.4.0-rc1", fileVersion: "0.3.0" });
  expect(rc.code).toBe(0);
  for (const r of [below, spoof, rc]) expect(r.calls).not.toContain("git/refs");
});

test("an open pull request for the same branch is not duplicated", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", openPr: true });
  expect(r.code).toBe(0);
  expect(r.calls).not.toContain("pr create");
});

test("a version file with a space and regex characters in its path is read, fetched and written with the path encoded", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", path: "a b/v[1].(x)+.toml" });
  expect(r.stderr).toBe("");
  expect(r.code).toBe(0);
  expect(r.calls).toContain("contents/a%20b/v%5B1%5D.%28x%29%2B.toml?ref=chore/start-0.3.1");
  const put = r.calls.split("\n").find((c) => c.includes("--method PUT"))!;
  expect(Buffer.from(put.match(/content=(\S+)/)![1]!, "base64").toString()).toContain('version = "0.3.1"');
});

test("a version file with no version line, or with two, is refused before anything is created", () => {
  for (const fileText of ["package {\n  name = \"x\"\n}\n", 'a {\n  version = "0.3.0"\n}\nb {\n  version = "0.3.0"\n}\n']) {
    const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", fileText });
    expect(r.code, fileText).toBe(1);
    expect(r.stderr).toContain("exactly one line");
    expect(r.calls, fileText).not.toContain("git/refs");
  }
});

test("a failure after the branch is made and before the pull request is open deletes the branch again", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", failPrCreate: true });
  expect(r.code).not.toBe(0);
  expect(r.calls).toContain("--method DELETE repos/o/r/git/refs/heads/chore/start-0.3.1");
});

test("a failure after the pull request is open leaves the branch alone", () => {
  const r = runScript({ tag: "v0.3.0", fileVersion: "0.3.0", checks: '["fail"]' });
  expect(r.code).toBe(1);
  expect(r.calls).not.toContain("--method DELETE");
});
