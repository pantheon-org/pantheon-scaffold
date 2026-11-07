// P.6 spike: the same file set as tmp/pkl-spike, rendered with projen. Run with: bun .projenrc.ts
// STYLE=verbatim|typed picks how cargo-deny.yml is rendered. AGNIX=on|off is the toggle. TOOLCHAIN is the one source.
import { readFileSync } from "node:fs";
import { Project, SampleFile, TextFile } from "projen";
import { GitHub, GithubWorkflow } from "projen/lib/github";

const style = process.env.STYLE ?? "verbatim";
const agnix = (process.env.AGNIX ?? "off") === "on";
const toolchain = process.env.TOOLCHAIN ?? "1.96.1";
const asset = (name: string) => readFileSync(`${__dirname}/assets/${name}`, "utf8");
const tmpl = (name: string) => asset(name).replaceAll("@@TOOLCHAIN@@", toolchain);
// projen joins lines without a trailing newline, so an empty last line is added to end the file with one.
const lines = (s: string) => [...s.replace(/\n$/, "").split("\n"), ""];

const project = new Project({ name: "p6-projen-spike" });

new TextFile(project, "rust-toolchain.toml", { lines: lines(tmpl("rust-toolchain.toml.tmpl")), marker: false });
new TextFile(project, "deny.toml", { lines: lines(asset("deny.toml")), marker: false });
new TextFile(project, ".github/scripts/cargo-deny.sh", { lines: lines(asset("cargo-deny.sh")), marker: false, executable: true });
new SampleFile(project, "README.md", { contents: "# Seeded once\n\nEdit freely; the generator never touches this again.\n" });
if (agnix) {
  new TextFile(project, ".github/scripts/agnix-introduced-errors.sh", { lines: lines(asset("agnix-introduced-errors.sh")), marker: false, executable: true });
}

if (style === "verbatim") {
  new TextFile(project, ".github/workflows/cargo-deny.yml", { lines: lines(tmpl("cargo-deny.yml.tmpl")), marker: false });
} else {
  // Best effort with the typed API. Escape hatches (file.addOverride) are counted in the README.
  const github = new GitHub(project, { mergify: false, pullRequestLint: false, projenCredentials: undefined });
  const wf = new GithubWorkflow(github, "Cargo Deny", {
    fileName: "cargo-deny.yml",
    limitConcurrency: true,
    concurrencyOptions: { group: "${{ github.workflow }}-${{ github.ref }}", cancelInProgress: true },
  });
  wf.on({
    pullRequest: { types: ["opened", "synchronize", "reopened"], branches: ["main"] },
    push: { branches: ["main"] },
    schedule: [{ cron: "17 5 * * 1" }],
    workflowDispatch: {},
  });
  wf.addJob("cargo-deny", {
    name: "Cargo Deny",
    runsOn: ["ubuntu-latest"],
    permissions: {},
    timeoutMinutes: 10,
    steps: [
      { uses: "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1", with: { "persist-credentials": false } },
      { name: "Install Rust toolchain", run: `rustup toolchain install ${toolchain} --profile minimal\nrustup default ${toolchain}\n` },
      { name: "Check dependencies", run: "bash .github/scripts/cargo-deny.sh" },
    ],
  });
  // Escape hatches: the typed API has no workflow-level permissions and no null for workflow_dispatch.
  wf.file?.addOverride("permissions", { contents: "read" });
  wf.file?.addOverride("on.workflow_dispatch", null);
}

project.synth();
