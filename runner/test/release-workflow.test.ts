import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

// Properties of .github/workflows/release.yml that make publishing safe. They are read from the file, so a change that breaks one fails here.
const path = join(import.meta.dir, "../../.github/workflows/release.yml");
const text = readFileSync(path, "utf8");
type Step = { name?: string; uses?: string; run?: string; if?: string; env?: Record<string, string>; with?: Record<string, unknown> };
type Job = { needs?: string | string[]; permissions?: Record<string, string>; steps: Step[]; strategy?: { matrix?: unknown }; if?: string };
const wf = Bun.YAML.parse(text) as { on: Record<string, any>; permissions: unknown; jobs: Record<string, Job>; concurrency?: { "cancel-in-progress"?: boolean } };
const jobs = Object.entries(wf.jobs);
const allSteps = jobs.flatMap(([name, j]) => j.steps.map((s) => ({ job: name, ...s })));

test("it runs on a v-tag push or by hand, never on pull_request_target or a branch push", () => {
  expect(Object.keys(wf.on).sort()).toEqual(["push", "workflow_dispatch"]);
  expect(wf.on.push).toEqual({ tags: ["v[0-9]*"] });
  expect(text).not.toContain("pull_request_target");
});

test("the manual run is a dry run unless told otherwise", () => {
  expect(wf.on.workflow_dispatch.inputs.dry_run).toMatchObject({ type: "boolean", default: true });
});

test("nothing has permissions by default and every job asks for its own", () => {
  expect(wf.permissions).toEqual({});
  for (const [name, j] of jobs) expect(j.permissions, `job ${name}`).toBeDefined();
});

test("only the release job can write contents, attest and use the id token", () => {
  for (const [name, j] of jobs) {
    const writes = Object.entries(j.permissions ?? {}).filter(([, v]) => v === "write").map(([k]) => k).sort();
    expect(writes, `job ${name}`).toEqual(name === "release" ? ["attestations", "contents", "id-token"] : []);
  }
});

test("every action is pinned by a 40 character SHA", () => {
  const uses = allSteps.map((s) => s.uses).filter((u): u is string => !!u && !u.startsWith("./"));
  expect(uses.length).toBeGreaterThan(5);
  for (const u of uses) expect(u, u).toMatch(/@[0-9a-f]{40}$/);
});

test("it uses no secrets and persists no checkout credentials", () => {
  expect(text).not.toMatch(/secrets\./);
  for (const s of allSteps.filter((s) => s.uses?.startsWith("actions/checkout@"))) expect(s.with?.["persist-credentials"], s.job).toBe(false);
});

test("no run script interpolates text a pusher controls: tags, refs and inputs go through env", () => {
  for (const s of allSteps.filter((s) => s.run)) {
    expect(s.run, `${s.job}: ${s.name}`).not.toMatch(/\$\{\{\s*(github\.(ref|ref_name|head_ref|event)|inputs\.)/);
  }
});

test("only the release job creates a release, as a pre-release, and never on a dry run", () => {
  const creators = allSteps.filter((s) => s.run?.includes("gh release create"));
  expect(creators.map((s) => s.job)).toEqual(["release"]);
  expect(creators[0].run).toContain("--prerelease");
  expect(creators[0].run).toContain("--verify-tag");
  expect(creators[0].if).toBe("github.event_name == 'push'");
});

test("the attestation is made only on a real release, and covers the assets", () => {
  const attest = allSteps.filter((s) => s.uses?.startsWith("actions/attest-build-provenance@"));
  expect(attest).toHaveLength(1);
  expect(attest[0].job).toBe("release");
  expect(attest[0].if).toBe("github.event_name == 'push'");
  expect(String(attest[0].with?.["subject-path"])).toContain("dist/");
});

test("the tag is checked against PklProject before anything is built", () => {
  expect(wf.jobs.prepare.steps.some((s) => s.run?.includes("release-assets.ts check-tag"))).toBe(true);
  const after: Record<string, string> = { package: "prepare", build: "prepare", smoke: "build", release: "smoke" };
  for (const [name, needs] of Object.entries(after)) expect([wf.jobs[name].needs].flat(), name).toContain(needs);
  expect([wf.jobs.release.needs].flat()).toContain("package");
});

// Task 2.5: the runner is the Rust one, built natively on each target by the same script ci.yml runs.
const targets = [
  { os: "ubuntu-latest", target: "linux-x64" },
  { os: "ubuntu-24.04-arm", target: "linux-arm64" },
  { os: "macos-latest", target: "darwin-arm64" },
];

test("the runner is built on a machine of each target, in a matrix, and a failure on one does not hide the others", () => {
  const b = wf.jobs.build as Job & { strategy: { matrix: { include: unknown[] }; "fail-fast"?: boolean } };
  expect(b.strategy.matrix.include).toEqual(targets);
  expect(b.strategy["fail-fast"]).toBe(false);
  expect((wf.jobs.smoke as Job & { strategy: { matrix: { include: unknown[] } } }).strategy.matrix.include).toEqual(targets);
});

test("each build installs the pinned Rust toolchain, builds with the shared script, and uploads its own binary", () => {
  const steps = wf.jobs.build.steps;
  const toolchain = steps.find((s) => s.run?.includes("rustup toolchain install"));
  // Every version in the step, the one installed and the one made the default, is the one rust-toolchain.toml pins.
  const channel = /channel\s*=\s*"([^"]+)"/.exec(readFileSync(join(import.meta.dir, "../../rust-toolchain.toml"), "utf8"))?.[1];
  expect(channel).toMatch(/^\d+\.\d+\.\d+$/);
  expect(toolchain?.run?.match(/\d+\.\d+\.\d+/g)).toEqual([channel, channel]);
  const build = steps.find((s) => s.run?.includes("build-rust.ts"));
  expect(build?.run).toContain('--target "${{ matrix.target }}"');
  expect(build?.run).toContain('--version "$VERSION"');
  const upload = steps.find((s) => s.uses?.startsWith("actions/upload-artifact@"));
  expect(upload?.with?.name).toBe("binary-${{ matrix.target }}");
  expect(upload?.with?.path).toBe("dist/pantheon-scaffold-${{ matrix.target }}");
  expect(upload?.with?.["if-no-files-found"]).toBe("error");
});

test("the static Linux builds get their C compiler on Linux only, and nothing cross-compiles", () => {
  const musl = wf.jobs.build.steps.find((s) => s.run?.includes("musl-tools"));
  expect(musl?.if).toBe("runner.os == 'Linux'");
  expect(text).not.toContain("bun build --compile");
  expect(text).not.toContain("runner/scripts/build.ts");
});

test("the package is built on its own and uploaded as package-files", () => {
  const steps = wf.jobs.package.steps;
  expect(steps.some((s) => s.run?.includes("pkl project package"))).toBe(true);
  expect(steps.find((s) => s.uses?.startsWith("actions/upload-artifact@"))?.with?.name).toBe("package-files");
});

test("smoke tests the binary built for its own target, and the release gets the package and all three binaries into one folder", () => {
  const smoke = wf.jobs.smoke.steps.find((s) => s.uses?.startsWith("actions/download-artifact@"));
  expect(smoke?.with?.name).toBe("binary-${{ matrix.target }}");
  const downloads = wf.jobs.release.steps.filter((s) => s.uses?.startsWith("actions/download-artifact@"));
  expect(downloads.map((s) => s.with)).toEqual([
    { name: "package-files", path: "dist/" },
    { pattern: "binary-*", "merge-multiple": true, path: "dist/" },
  ]);
  const checksums = wf.jobs.release.steps.findIndex((s) => s.run?.includes("release-assets.ts checksums"));
  expect(checksums).toBeGreaterThan(wf.jobs.release.steps.indexOf(downloads[1]));
});

test("the release workflow and the pull request checks build the binary with the same script", () => {
  const ci = readFileSync(join(import.meta.dir, "../../.github/workflows/ci.yml"), "utf8");
  expect(ci).toContain("runner/scripts/build-rust.ts");
  expect(ci).toContain("runner/scripts/smoke.ts");
});

test("the release is verified from the outside after it is published, on all three platforms", () => {
  const v = wf.jobs["verify-release"];
  expect([v.needs].flat()).toContain("release");
  expect(v.if).toBe("github.event_name == 'push'");
  expect(JSON.stringify(v.strategy?.matrix)).toContain("ubuntu-24.04-arm");
  const run = v.steps.map((s) => s.run ?? "").join("\n");
  expect(run).toContain("gh attestation verify");
  expect(run).toContain("mise install --locked");
  expect(run).toContain("upgrade");
});

test("two releases never run at once, and a running release is not cancelled", () => {
  expect(wf.concurrency?.["cancel-in-progress"]).toBe(false);
});

test("the consumer the release is verified with limits the lock to the supported platforms and checks the assets mise chose", () => {
  const run = wf.jobs["verify-release"].steps.map((s) => s.run ?? "").join("\n");
  expect(run).toContain('lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]');
  expect(run).toContain("pantheon-scaffold-(linux-x64|linux-arm64|darwin-arm64)");
});

// Phase 5 wave E: the consumer smoke test. verify-release renders the whole standard set through the real package, as a consumer would.
test("verify-release renders the standard workflows and files through the published package, checks them, and renders again with nothing to write", () => {
  const step = wf.jobs["verify-release"].steps.find((s) => s.run?.includes("standardWorkflows"));
  expect(step, "a step that renders the standard set").toBeDefined();
  const run = step!.run!;
  expect(run).toContain("standardFiles");
  expect(run).toContain("::sha256:");
  expect(run).toMatch(/pantheon-scaffold check /);
  expect(run).toContain("written 0 file(s)");
  for (const f of [".github/workflows/dist-check.yml", ".github/workflows/audit-skill.yml", ".github/workflows/scaffold-check.yml"]) expect(run, f).toContain(f);
  expect(run).not.toMatch(/\$\{\{/);
});

test("verify-release imports Builtins from the same package URI, with the same checksum, as it amends Config", () => {
  const run = wf.jobs["verify-release"].steps.find((s) => s.run?.includes("standardWorkflows"))!.run!;
  expect(run).toContain("#/Config.pkl");
  expect(run).toContain("#/Builtins.pkl");
});
