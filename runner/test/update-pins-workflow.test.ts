import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Properties of .github/workflows/update-pins.yml, which keeps pkl/Pins.pkl current and publishes the freshness record (decision 83).
// The App steps are unverified until the workflow has run once with the App installed.
const text = readFileSync(join(import.meta.dir, "../../.github/workflows/update-pins.yml"), "utf8");
type Step = { name?: string; id?: string; uses?: string; run?: string; env?: Record<string, string>; with?: Record<string, unknown>; if?: string };
type Job = { needs?: string | string[]; permissions: Record<string, string>; env?: Record<string, string>; steps: Step[] };
const wf = Bun.YAML.parse(text) as { on: Record<string, any>; permissions: unknown; jobs: Record<string, Job> };
const steps = Object.values(wf.jobs).flatMap((j) => j.steps);
const runs = steps.filter((s) => s.run);
const idx = (needle: string) => runs.findIndex((s) => s.run!.includes(needle));

// The condition of everything that writes: not a manual dry run. A schedule event has no inputs, so it is not a dry run.
const NOT_DRY = "${{ !(github.event_name == 'workflow_dispatch' && inputs.dry_run) }}";
const DRY = "${{ github.event_name == 'workflow_dispatch' && inputs.dry_run }}";
const RUST_WRITE = "tool/scaffold-release update-pins";
const TS_WRITE = "mise x -- bun runner/scripts/update-pins.ts";
const rustWrite = () => runs.find((s) => s.run!.includes(RUST_WRITE) && s.run!.includes("--freshness-out"))!;
const tsWrite = () => runs.find((s) => s.run!.includes(TS_WRITE) && s.run!.includes("--freshness-out"))!;

test("it runs on a schedule or by hand, and on no push: merging a change to the tool must not start a real run", () => {
  expect(Object.keys(wf.on).sort()).toEqual(["schedule", "workflow_dispatch"]);
  expect(wf.on.push).toBeUndefined();
  expect(text).toContain("There is no push trigger on purpose");
  expect(text).not.toContain("pull_request");
});

test("a manual run is a dry run unless told otherwise, and says which tool writes in a real run, the Rust one by default", () => {
  const inputs = wf.on.workflow_dispatch.inputs;
  expect(inputs.dry_run).toMatchObject({ type: "boolean", default: true });
  expect(inputs.implementation).toMatchObject({ type: "choice", options: ["rust", "typescript"], default: "rust" });
});

test("nothing has permissions by default and every job reads contents only: writes use the App token", () => {
  expect(wf.permissions).toEqual({});
  for (const j of Object.values(wf.jobs)) expect(j.permissions).toEqual({ contents: "read" });
  const mint = steps.find((s) => s.id === "app-token")!;
  expect(mint.with).toMatchObject({ "permission-contents": "write", "permission-pull-requests": "write" });
  expect(mint.with).not.toHaveProperty("permission-workflows");
});

test("the only secret is the App key, and the default token is never used to write", () => {
  expect([...text.matchAll(/secrets\.(\w+)/g)].map((m) => m[1])).toEqual(["GH_APP_PRIVATE_KEY"]);
  expect(text).not.toContain("secrets.GITHUB_TOKEN");
});

test("every action is pinned by a 40 character SHA and checkout keeps no credentials", () => {
  for (const s of steps.filter((s) => s.uses)) expect(s.uses, s.uses).toMatch(/@[0-9a-f]{40}$/);
  for (const s of steps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"]).toBe(false);
});

test("no run script interpolates an expression, and each is 15 lines or fewer", () => {
  for (const s of runs) {
    expect(s.run, s.name).not.toContain("${{");
    expect(s.run!.trimEnd().split("\n").length, s.name).toBeLessThanOrEqual(15);
  }
});

test("the freshness record is written only after the pins were checked and the pull request step ran", () => {
  const update = runs.indexOf(rustWrite());
  const pr = idx("gh pr create");
  const publish = idx("refs/heads/freshness");
  expect(update).toBeGreaterThanOrEqual(0);
  expect(update).toBeLessThan(pr);
  expect(pr).toBeLessThan(publish);
  expect(runs.indexOf(tsWrite())).toBeLessThan(pr);
});

test("each writing step asks for the record and the summary, and a failing check fails the job", () => {
  for (const update of [rustWrite(), tsWrite()]) {
    expect(update.run).toContain("--freshness-out");
    expect(update.run).toContain("--summary");
    expect(update.run).not.toContain("--no-write");
    expect(update.run).not.toMatch(/\|\|\s*true/);
    expect(update["continue-on-error" as keyof Step]).toBeUndefined();
  }
});

test("the record goes to the freshness branch, never main", () => {
  const publish = runs[idx("refs/heads/freshness")].run!;
  expect(publish).not.toContain("refs/heads/main");
  expect(publish).toContain("freshness.json");
});

test("the pull request step does nothing when no pin changed", () => {
  expect(runs[idx("gh pr create")].run).toMatch(/git diff --quiet/);
});

test("the pull request is a conventional commit and is opened with the App token", () => {
  const pr = runs[idx("gh pr create")];
  expect(pr.run).toMatch(/--title "chore\(deps\): /);
  expect(pr.env?.GH_TOKEN).toBe("${{ steps.app-token.outputs.token }}");
});

// Phase 3 of docs/ts-tooling-rust-port.md: the update tool is the Rust crate crates/scaffold-release. Building it runs the build scripts of
// every crate it depends on, so it is built in a job that cannot write and has no token, and the update job, which holds the App token,
// only downloads the finished binary and checks its checksum before it runs it.
test("the release tool is built in a job that holds no token", () => {
  const tool = wf.jobs.tool;
  expect(tool.permissions).toEqual({ contents: "read" });
  expect(tool.env).toBeUndefined();
  for (const s of tool.steps) expect(JSON.stringify(s), s.name ?? s.uses).not.toMatch(/github\.token|secrets\.|app-token/);
  expect(tool.steps.some((s) => s.run?.includes("cargo build --release --locked -p scaffold-release"))).toBe(true);
  expect(tool.steps.find((s) => s.uses?.startsWith("actions/upload-artifact@"))?.with?.name).toBe("scaffold-release");
});

test("the update job, which holds the App token, never runs cargo and downloads the tool and checks its checksum before it runs it", () => {
  const job = wf.jobs.update;
  expect([job.needs].flat()).toContain("tool");
  expect(job.steps.some((s) => /\bcargo\b/.test(s.run ?? ""))).toBe(false);
  const download = job.steps.findIndex((s) => s.uses?.startsWith("actions/download-artifact@") && s.with?.name === "scaffold-release");
  expect(download).toBeGreaterThanOrEqual(0);
  const verify = job.steps.findIndex((s) => s.run?.includes("sha256sum --check --strict"));
  expect(verify).toBeGreaterThan(download);
  expect(job.steps[verify].env?.EXPECTED).toBe("${{ needs.tool.outputs.sha256 }}");
  const firstRun = job.steps.findIndex((s) => s.run?.includes("tool/scaffold-release update-pins"));
  expect(firstRun).toBeGreaterThan(verify);
});

test("no job has a job-wide token in its environment: each step that needs one names it", () => {
  for (const [name, j] of Object.entries(wf.jobs)) expect(j.env?.GITHUB_TOKEN, name).toBeUndefined();
});

test("a dry run reaches nothing that writes: no App token, no write step, no pull request, no freshness record", () => {
  const mint = steps.find((s) => s.id === "app-token")!;
  expect(mint.if).toBe(NOT_DRY);
  const writers = [rustWrite(), tsWrite(), runs[idx("gh pr create")], runs[idx("refs/heads/freshness")]];
  for (const step of writers) expect(step.if, step.name).toContain("!(github.event_name == 'workflow_dispatch' && inputs.dry_run)");
  const dry = runs.find((s) => s.run!.includes("--no-write") && s.run!.includes(RUST_WRITE))!;
  expect(dry.if).toBe(DRY);
  expect(dry.env?.GITHUB_TOKEN).toBe("${{ github.token }}");
});

test("a dry run runs both tools and fails when they disagree, whatever the Rust one says", () => {
  const dry = runs.find((s) => s.run!.includes("--no-write") && s.run!.includes(RUST_WRITE))!;
  expect(dry.run).toContain("tool/scaffold-release update-pins --no-write");
  expect(dry.run).toContain("mise x -- bun runner/scripts/update-pins.ts --no-write");
  expect(dry.run).toContain('[ "$rust_code" != "$ts_code" ]');
  expect(dry.run).toContain("cmp -s");
  expect(dry.run).toContain("disagree");
  expect(dry.run!.trimEnd().endsWith('exit "$rust_code"')).toBe(true);
});

test("a real run uses the Rust tool unless asked for the TypeScript one, with the App token, and mise is installed only for the TypeScript tool", () => {
  expect(rustWrite().if).toBe("${{ !(github.event_name == 'workflow_dispatch' && inputs.dry_run) && inputs.implementation != 'typescript' }}");
  expect(tsWrite().if).toBe("${{ !(github.event_name == 'workflow_dispatch' && inputs.dry_run) && inputs.implementation == 'typescript' }}");
  for (const s of [rustWrite(), tsWrite()]) expect(s.env?.GITHUB_TOKEN).toBe("${{ steps.app-token.outputs.token }}");
  const toolchain = wf.jobs.update.steps.filter((s) => s.uses?.startsWith("jdx/mise-action@") || s.run === "mise install --locked");
  expect(toolchain).toHaveLength(2);
  for (const s of toolchain) expect(s.if, s.name ?? s.uses).toBe("${{ github.event_name == 'workflow_dispatch' && (inputs.dry_run || inputs.implementation == 'typescript') }}");
});
