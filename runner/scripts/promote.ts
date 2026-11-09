// Promotion of a pre-release to a stable release (Phase 8, decisions 46, 56, 60 and 90). The logic and its tests are in runner/src/promote.ts.
// usage: bun runner/scripts/promote.ts check <tag>     runs every guard and edits nothing
//        bun runner/scripts/promote.ts promote <tag>   runs every guard, removes the pre-release flag, and checks the result
// env:   GH_TOKEN (the workflow's token), GH_REPO (default pantheon-org/pantheon-scaffold), GITHUB_REF (must be refs/heads/main)
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { parseKnownBad } from "../src/knownbad";
import { promote, PromoteError, type PromoteDeps } from "../src/promote";

const [cmd, tag] = Bun.argv.slice(2);
if ((cmd !== "check" && cmd !== "promote") || !tag) {
  console.error("usage: promote.ts check <tag> | promote <tag>");
  process.exit(2);
}
const repo = process.env.GH_REPO ?? "pantheon-org/pantheon-scaffold";
const work = mkdtempSync(join(tmpdir(), "promote-"));

const gh = async (args: string[]): Promise<{ out: string; err: string; code: number }> => {
  const p = Bun.spawn(["gh", ...args], { stdout: "pipe", stderr: "pipe" });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
};
const need = async (args: string[]): Promise<string> => {
  const r = await gh(args);
  if (r.code !== 0) throw new Error(`gh ${args[0]} ${args[1] ?? ""} failed (exit ${r.code}): ${r.err}`);
  return r.out;
};

const deps: PromoteDeps = {
  async viewRelease(t) {
    const j = JSON.parse(await need(["release", "view", t, "--repo", repo, "--json", "tagName,name,body,isPrerelease,isDraft,assets"])) as {
      tagName: string; name: string; body: string; isPrerelease: boolean; isDraft: boolean; assets: { name: string }[];
    };
    return { ...j, assets: j.assets.map((a) => a.name) };
  },
  async downloadAsset(t, name) {
    const dir = join(work, `dl-${Math.random().toString(36).slice(2)}`);
    mkdirSync(dir);
    await need(["release", "download", t, "--repo", repo, "--pattern", name, "--dir", dir]);
    return new Uint8Array(readFileSync(join(dir, name)));
  },
  async verifyAttestation(name, bytes) {
    const dir = join(work, `at-${Math.random().toString(36).slice(2)}`);
    mkdirSync(dir);
    writeFileSync(join(dir, name), bytes);
    await need(["attestation", "verify", join(dir, name), "--repo", repo]);
  },
  async knownBad() {
    const list = parseKnownBad(readFileSync(join(import.meta.dir, "../../known-bad.json"), "utf8"));
    if (!list) throw new Error("known-bad.json is not a valid known-bad list");
    return list;
  },
  async releaseRun(t) {
    const runs = JSON.parse(await need(["run", "list", "--repo", repo, "--workflow", "release.yml", "--event", "push", "--branch", t, "--limit", "10", "--json", "databaseId,status,conclusion"])) as { databaseId: number; status: string; conclusion: string }[];
    const done = runs.find((r) => r.status === "completed");
    if (!done) return undefined;
    const view = JSON.parse(await need(["run", "view", String(done.databaseId), "--repo", repo, "--json", "jobs"])) as { jobs: { name: string; conclusion: string }[] };
    return { conclusion: done.conclusion, jobs: view.jobs.map((j) => ({ name: j.name, conclusion: j.conclusion })) };
  },
  async latestTag() {
    const r = await gh(["api", `repos/${repo}/releases/latest`, "--jq", ".tag_name"]);
    return r.code === 0 ? r.out : undefined;
  },
  async edit(t, e) {
    await need(["release", "edit", t, "--repo", repo, "--prerelease=false", "--latest", "--title", e.title, "--notes", e.notes]);
  },
};

let code = 0;
try {
  const report = await promote(tag, process.env.GITHUB_REF ?? "", deps, { dryRun: cmd === "check" });
  for (const s of report.steps) console.log(`ok    ${s.guard}: ${s.detail}`);
  console.log(report.promoted ? `${tag} is now a stable release.` : `dry run: every guard passed for ${tag}, nothing was edited.`);
} catch (e) {
  if (e instanceof PromoteError) for (const s of e.report.steps) console.log(`${s.ok ? "ok   " : "FAIL "} ${s.guard}: ${s.detail}`);
  console.error(e instanceof PromoteError ? `refused: ${e.message}` : (e as Error).message);
  code = 1;
} finally {
  rmSync(work, { recursive: true, force: true });
}
process.exit(code);
