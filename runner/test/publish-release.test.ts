import { expect, test } from "bun:test";
import { chmodSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Runs .github/scripts/publish-release.sh with a stand-in `gh` that records its calls, so each path of the script is seen to do what the
// release process says: adopt one draft, create when nothing exists, and refuse a published release or two drafts.
const script = join(import.meta.dir, "../../.github/scripts/publish-release.sh");

type Outcome = { code: number; calls: string[]; stderr: string };

function run(opts: { draftIds: string[]; publishedExists: boolean }): Outcome {
  const dir = mkdtempSync(join(tmpdir(), "publish-release-"));
  const log = join(dir, "calls.log");
  const gh = join(dir, "gh");
  writeFileSync(
    gh,
    `#!/usr/bin/env bash
echo "$*" >> "${log}"
case "$1 $2" in
  "api repos/o/r/releases") printf '%b' "${opts.draftIds.join("\\n")}" ;;
  "api repos/o/r/releases/"*) [ "$3" = "--jq" ] && echo "changelog body" ;;
  "release view") exit ${opts.publishedExists ? 0 : 1} ;;
esac
exit 0
`,
  );
  chmodSync(gh, 0o755);
  const p = Bun.spawnSync(["bash", script], {
    env: { PATH: `${dir}:${process.env.PATH}`, GH_REPO: "o/r", TAG: "v1.2.3", GITHUB_STEP_SUMMARY: join(dir, "summary") },
    cwd: dir,
  });
  let calls: string[] = [];
  try {
    calls = readFileSync(log, "utf8").trim().split("\n");
  } catch {}
  return { code: p.exitCode ?? -1, calls, stderr: p.stderr.toString() };
}

test("one draft for the tag: assets are uploaded to it and it is published as a pre-release by id", () => {
  const r = run({ draftIds: ["777"], publishedExists: false });
  expect(r.code).toBe(0);
  expect(r.calls.some((c) => c.startsWith("release upload v1.2.3"))).toBe(true);
  const patch = r.calls.find((c) => c.includes("--method PATCH"))!;
  expect(patch).toContain("repos/o/r/releases/777");
  expect(patch).toContain("draft=false");
  expect(patch).toContain("prerelease=true");
  expect(patch).toContain("v1.2.3 (pre-release)");
  expect(r.calls).toContain("changelog body");
  expect(r.calls.some((c) => c.startsWith("release create"))).toBe(false);
});

test("no release for the tag: it is created as a pre-release, as for a tag pushed by hand", () => {
  const r = run({ draftIds: [], publishedExists: false });
  expect(r.code).toBe(0);
  const create = r.calls.find((c) => c.startsWith("release create v1.2.3"))!;
  expect(create).toContain("--verify-tag");
  expect(create).toContain("--prerelease");
  expect(r.calls.some((c) => c.includes("--method PATCH"))).toBe(false);
});

test("a published release for the tag stops it, and nothing is created or edited", () => {
  const r = run({ draftIds: [], publishedExists: true });
  expect(r.code).toBe(1);
  expect(r.stderr).toContain("never re-published");
  expect(r.calls.some((c) => c.startsWith("release create") || c.startsWith("release upload") || c.includes("PATCH"))).toBe(false);
});

test("two drafts for the tag stop it before anything is uploaded", () => {
  const r = run({ draftIds: ["1", "2"], publishedExists: false });
  expect(r.code).toBe(1);
  expect(r.stderr).toContain("More than one draft");
  expect(r.calls.some((c) => c.startsWith("release upload") || c.includes("PATCH"))).toBe(false);
});
