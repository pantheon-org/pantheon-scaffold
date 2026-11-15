import { afterEach, expect, test } from "bun:test";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { evalContractJson } from "./support/contract";
import { cleanupRepos, tempRepo } from "./helpers";

// The release options: releaseShape, releaseAutomerge, versionFile, versionCommitScope and seedReleaseConfig, and what the release-please,
// release-automerge and release-bump builtins render for them. Every error case is reached through Presence.forOptions, which every aggregator reads.
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");

const COMPOSITION = `workflows { ...Builtins.standardWorkflows(options) }
files {
  ...Builtins.standardFiles(options)
  ...Builtins.baseFiles(options)
  ...Builtins.toolingFiles(options)
}`;

let n = 0;
type Contract = { files: { path: string; class: string; mode: string; content?: string }[] };
async function render(extra: string, without: string[] = []): Promise<Map<string, { class: string; mode: string; content: string }>> {
  const list = without.map((w) => `"${w}"`).join("; ");
  const f = join(tempRepo(), `release-${n++}.pkl`);
  writeFileSync(
    f,
    `amends "file://${root}/pkl/Config.pkl"\nimport "file://${root}/pkl/Builtins.pkl"\noptions {\n  toolchain = "1.96.1"\n  runnerVersion = "0.1.9"\n  skills = false\n  ${extra.includes("crateName") ? "" : 'crateName = "demo"'}\n  ${extra}\n  without { ${list} }\n}\n${COMPOSITION}\n`,
  );
  const c = (await evalContractJson(f, root, { allowFilesUnder: [join(root, "pkl")] })) as Contract;
  return new Map(c.files.map((x) => [x.path, { class: x.class, mode: x.mode, content: x.content ?? "" }]));
}
const text = (m: Map<string, { content: string }>, path: string): string => {
  const f = m.get(path);
  if (!f) throw new Error(`no file ${path}; files: ${[...m.keys()].sort().join(", ")}`);
  return f.content;
};

const DRAFT = `binary = false; releaseShape = "draft"; versionFile = "pkl/PklProject"; versionCommitScope = "pkl"`;

test("the shape follows binary unless it is set, and each shape has its own header", async () => {
  const dist = await render("binary = true");
  expect(text(dist, ".github/workflows/release-please.yml")).toContain("starts release.yml (cargo-dist)");
  const published = await render("binary = false");
  expect(text(published, ".github/workflows/release-please.yml")).toContain("creates the vX.Y.Z tag and the GitHub Release");
  const draft = await render(DRAFT);
  const header = text(draft, ".github/workflows/release-please.yml");
  expect(header).toContain("a DRAFT release");
  expect(header).toContain("The version line of pkl/PklProject is updated through the x-release-please-version");
  expect(header).not.toContain("tekhne");
  expect(text(await render('binary = false; releaseShape = "draft"'), ".github/workflows/release-please.yml")).not.toContain("version line of");
});

test("the draft shape renders the publishing script, the others do not", async () => {
  const draft = await render(DRAFT);
  expect(draft.get(".github/scripts/publish-release.sh")).toMatchObject({ class: "owned", mode: "0755" });
  expect(text(draft, ".github/scripts/publish-release.sh")).toContain('DIST_DIR:-dist');
  expect((await render("binary = false")).has(".github/scripts/publish-release.sh")).toBe(false);
  expect((await render("binary = true")).has(".github/scripts/publish-release.sh")).toBe(false);
});

test("the seeded release-please files are seeds, differ by shape, and are skipped by seedReleaseConfig = false", async () => {
  for (const [extra, expected] of [
    ["binary = true", { type: "rust", path: "crates/demo" }],
    ["binary = false", { type: "simple", path: "." }],
    [DRAFT, { type: "simple", path: "." }],
  ] as const) {
    const m = await render(extra);
    expect(m.get("release-please-config.json")?.class, extra).toBe("seed");
    expect(m.get(".release-please-manifest.json")?.class, extra).toBe("seed");
    const cfg = JSON.parse(text(m, "release-please-config.json"));
    const pkg = cfg.packages[expected.path];
    expect(pkg, extra).toBeDefined();
    expect(pkg["release-type"] ?? cfg["release-type"], extra).toBe(expected.type);
    expect(JSON.parse(text(m, ".release-please-manifest.json"))).toEqual({ ".": "0.0.0" });
    const off = await render(`${extra}; seedReleaseConfig = false`);
    expect(off.has("release-please-config.json") || off.has(".release-please-manifest.json"), extra).toBe(false);
  }
  const dist = JSON.parse(text(await render("binary = true"), "release-please-config.json"));
  expect(dist["skip-github-release"]).toBe(true);
  expect(dist.draft).toBeUndefined();
  const draft = JSON.parse(text(await render(DRAFT), "release-please-config.json"));
  expect(draft).toMatchObject({ draft: true, "force-tag-creation": true, "include-component-in-tag": false });
  expect(draft.packages["."]["extra-files"]).toEqual([{ type: "generic", path: "pkl/PklProject" }]);
});

test("the seed for this repository's options is the committed release-please-config.json", async () => {
  const m = await render(`binary = false; crateName = "pantheon-scaffold"; releaseShape = "draft"; versionFile = "pkl/PklProject"`);
  const committed = await Bun.file(join(root, "release-please-config.json")).text();
  expect(text(m, "release-please-config.json")).toBe(committed);
});

test("release-automerge: manual by default with a notice when the schedule is off, today's schedule with always", async () => {
  const gated = text(await render("binary = false"), ".github/workflows/release-automerge.yml");
  expect(gated).toContain("RELEASE_AUTOMERGE_SCHEDULE");
  expect(gated).toContain("steps.gate.outputs.run == 'true'");
  expect(gated).toContain("Scheduled run skipped");
  expect(gated).toContain("workflow_dispatch");
  const always = text(await render('binary = false; releaseAutomerge = "always"'), ".github/workflows/release-automerge.yml");
  expect(always).not.toContain("RELEASE_AUTOMERGE_SCHEDULE");
  expect(always).not.toContain("gate");
  expect(always).toContain("cron: 37 8 * * 1");
});

test("release-bump renders only with versionFile, as a workflow and a script that name the file and the scope", async () => {
  const none = await render("binary = false");
  expect(none.has(".github/workflows/release-bump.yml") || none.has(".github/scripts/release-bump.sh")).toBe(false);
  const m = await render(DRAFT);
  const wf = text(m, ".github/workflows/release-bump.yml");
  expect(wf).toContain("workflows:\n    - release");
  expect(wf).toContain("zizmor: ignore[dangerous-triggers]");
  expect(wf).toContain("timeout-minutes: 45");
  const script = text(m, ".github/scripts/release-bump.sh");
  expect(script).toContain("version_file='pkl/PklProject'");
  expect(script).toContain("chore(pkl): start $next");
  expect(script).toContain("max_polls=\"${MAX_POLLS:-60}\"");
  expect(m.get(".github/scripts/release-bump.sh")?.mode).toBe("0755");
  expect(text(m, ".github/dependabot.yml")).toContain(".github/workflows/release-bump.yml");
  const unscoped = text(await render(`binary = false; releaseShape = "draft"; versionFile = "version.toml"`), ".github/scripts/release-bump.sh");
  expect(unscoped).toContain('"chore: start $next"');
});

test("the release options are refused, with a message that names what is wrong, whatever is rendered", async () => {
  await expect(render('binary = true; releaseShape = "draft"')).rejects.toThrow(/releaseShape = "draft" cannot be used with options\.binary = true/);
  await expect(render('versionCommitScope = "pkl"')).rejects.toThrow(/versionCommitScope is set but options\.versionFile is not/);
  await expect(render('binary = false; versionFile = "pkl/PklProject"')).rejects.toThrow(/versionFile needs options\.releaseShape = "draft".*"published"/s);
  await expect(render(DRAFT, ["release-bump"])).rejects.toThrow(/leaves out "release-bump"/);
  for (const bad of ["/etc/passwd", "../up", "it's", "a/../b"]) {
    await expect(render(`binary = false; releaseShape = "draft"; versionFile = "${bad}"`), bad).rejects.toThrow(/versionFile must be a plain path/);
  }
});

test("release-bump waits for the workflow that the release shape names", async () => {
  const release = await Bun.file(join(root, ".github/workflows/release.yml")).text();
  expect(release).toMatch(/^name: release$/m);
});
