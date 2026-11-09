// Phase 8 wave E: the adoption guide (docs/adopting.md) followed literally, on a scratch clone of pantheon-adr, against the PUBLISHED release.
// Each step is the guide's own command. The clone, the mise folders and everything downloaded live in a temporary folder that is deleted
// afterwards, and nothing is pushed. It prints a markdown record of every step: the command, the exit code and the tail of the output.
// usage: bun runner/scripts/proof-adoption.ts [version]   (default: the newest published release)
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const REPO = "pantheon-org/pantheon-scaffold";
const work = mkdtempSync(join(tmpdir(), "proof-adoption-"));
const site = join(work, "site");
const token = (await Bun.$`env -u GH_TOKEN gh auth token`.quiet().text()).trim();
const baseEnv = Object.fromEntries(Object.entries(process.env).filter(([k, v]) => k !== "GH_TOKEN" && v !== undefined)) as Record<string, string>;
const env: Record<string, string> = {
  ...baseEnv,
  GITHUB_TOKEN: token,
  MISE_DATA_DIR: join(work, "mise-data"),
  MISE_CACHE_DIR: join(work, "mise-cache"),
  MISE_STATE_DIR: join(work, "mise-state"),
  MISE_CONFIG_DIR: join(work, "mise-config"),
  MISE_TRUSTED_CONFIG_PATHS: work,
  MISE_YES: "1",
  NO_COLOR: "1",
};
for (const d of ["mise-data", "mise-cache", "mise-state", "mise-config"]) mkdirSync(join(work, d));

type Rec = { step: string; command: string; exit: number; tail: string };
const record: Rec[] = [];
const sh = async (step: string, command: string, cwd = site, expect: number | number[] = 0): Promise<{ out: string; code: number }> => {
  const p = Bun.spawn(["bash", "-c", command], { cwd, stdout: "pipe", stderr: "pipe", env });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  const all = `${out}\n${err}`.trim();
  const tail = all.split("\n").slice(-5).join("\n").slice(-700);
  record.push({ step, command, exit: code, tail });
  const ok = (Array.isArray(expect) ? expect : [expect]).includes(code);
  if (!ok) throw new Error(`${step}: \`${command}\` exited ${code}, expected ${[expect].flat().join(" or ")}\n${all.slice(-1500)}`);
  return { out: all, code };
};

/** "Add this to your mise.toml": the guide's three settings and two tools go under the file's own [settings] and [tools]. */
function addToMise(text: string, version: string): string {
  const settings = ["lockfile = true", "locked_verify_provenance = true", 'lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]'];
  const tools = ['pkl = "0.32.2"', `"github:pantheon-org/pantheon-scaffold" = "${version}"`];
  const put = (t: string, header: string, lines: string[]): string => {
    const add = lines.filter((l) => !new RegExp(`^${l.split("=")[0].trim().replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*=`, "m").test(t));
    if (!add.length) return t;
    return t.includes(`${header}\n`) ? t.replace(`${header}\n`, `${header}\n${add.join("\n")}\n`) : `${header}\n${add.join("\n")}\n\n${t}`;
  };
  return put(put(text, "[settings]", settings), "[tools]", tools);
}

let version = Bun.argv[2] ?? "";
let failure = "";
try {
  if (!version) version = (await sh("pick a version", `gh release list --repo ${REPO} --exclude-pre-releases --limit 1 --json tagName --jq '.[0].tagName'`, work)).out.replace(/^v/, "");
  const known = (await sh("known-bad list", "curl -s https://raw.githubusercontent.com/pantheon-org/pantheon-scaffold/main/known-bad.json", work)).out;
  if (known.includes(`"version": "${version}"`)) throw new Error(`${version} is on the known-bad list`);

  await sh("clone a repository to adopt in", `git clone --quiet --depth 1 https://github.com/pantheon-org/pantheon-adr.git ${site} && git -C ${site} switch -q -c adopt-scaffold`, work);

  // First-time setup, step 1: install the runner with mise.
  const miseToml = join(site, "mise.toml");
  writeFileSync(miseToml, addToMise(readFileSync(miseToml, "utf8"), version));
  await sh("1. mise lock", "mise lock");
  await sh("1. mise install --locked", "mise install --locked");
  const reported = (await sh("1. the runner's version", "mise x -- pantheon-scaffold --version")).out.split("\n").pop();
  if (reported !== version) throw new Error(`the runner reports ${reported}, expected ${version}`);

  // Step 2: the package checksum, from a release verified first.
  await sh("2. download checksums.txt", `gh release download v${version} --repo ${REPO} -p checksums.txt -D ${work} --clobber`, work);
  await sh("2. verify its attestation", `gh attestation verify ${join(work, "checksums.txt")} --repo ${REPO}`, work);
  const sum = (await sh("2. the package checksum", `grep ' pantheon-scaffold@${version}$' ${join(work, "checksums.txt")}`, work)).out.split(/\s+/)[0];

  // Step 3: pantheon.pkl, exactly as the guide shows it.
  const uri = `package://github.com/${REPO}/releases/download/v${version}/pantheon-scaffold@${version}::sha256:${sum}`;
  writeFileSync(
    join(site, "pantheon.pkl"),
    `amends "${uri}#/Config.pkl"\nimport "${uri}#/Builtins.pkl"\n\noptions {\n  toolchain = "1.96.1"\n  crateName = "adr"\n  skillName = "adr-creator"\n}\n\nworkflows { ...Builtins.standardWorkflows(options) }\nfiles {\n  ...Builtins.standardFiles(options)\n  ...Builtins.baseFiles(options)\n  ...Builtins.toolingFiles()\n  ...Builtins.miseFiles(options)\n  ...Builtins.hkFiles(options)\n}\n`,
  );

  // Step 4: look before you write, then render.
  const dry = await sh("4. render --dry-run", "mise x -- pantheon-scaffold render pantheon.pkl --dry-run", site, [0, 3]);
  const refused = dry.out.split("\n").filter((l) => /was edited by hand|already exists|refused/i.test(l)).length;
  await sh("4. render --force", "mise x -- pantheon-scaffold render pantheon.pkl --force");
  await sh("4. check", "mise x -- pantheon-scaffold check pantheon.pkl");
  const again = await sh("4. a second render", "mise x -- pantheon-scaffold render pantheon.pkl");
  if (!again.out.includes("written 0 file(s)")) throw new Error(`a second render did not write 0 files: ${again.out.slice(-300)}`);

  // The guide's one-off: hk.pkl. The seeded one is written once the old one is out of the way.
  await sh("hk.pkl: remove the old one, render the seed", "rm hk.pkl && mise x -- pantheon-scaffold render pantheon.pkl && grep -c 'hk.shared.pkl' hk.pkl");
  await sh("hk.pkl: check still passes", "mise x -- pantheon-scaffold check pantheon.pkl");

  // The rendered tree builds and passes its tests, with the toolchain the repository pins.
  await sh("cargo build", "mise x -- cargo build --workspace --locked 2>&1 | tail -3 && exit ${PIPESTATUS[0]}");
  // Success is zero failures on every suite: the summary counts the suites and the failed ones, not just the last suite's line.
  await sh(
    "cargo test",
    "mise x -- cargo test --workspace --locked > test.log 2>&1; code=$?; awk '/^test result:/ {s++; if ($3 == \"ok.\") ok++; p += $4; f += $6} END {print \"suites: \" s \", ok: \" ok \", tests passed: \" p \", tests failed: \" f}' test.log; grep -c 'FAILED' test.log; exit $code",
  );
  record.push({ step: "note", command: "", exit: 0, tail: `the dry run listed ${refused} refusal line(s)` });
} catch (e) {
  failure = (e as Error).message;
}

const rows = record.map((r) => `| ${r.step} | \`${r.command.replace(/\|/g, "\\|").replace(work, "<scratch>")}\` | ${r.exit} |`);
console.log(`## Phase 8 wave E: the adoption guide followed literally\n\nRelease: v${version}. Scratch clone of pantheon-adr, isolated mise folders, nothing pushed.\n\n| Step | Command | Exit |\n|---|---|---|\n${rows.join("\n")}\n`);
for (const r of record.filter((r) => r.tail && r.step !== "note")) console.log(`### ${r.step}\n\n\`\`\`\n${r.tail.replace(new RegExp(work, "g"), "<scratch>")}\n\`\`\`\n`);
console.log(failure ? `FAILED: ${failure.replace(new RegExp(work, "g"), "<scratch>")}` : "Verdict: PASS");
rmSync(work, { recursive: true, force: true });
process.exit(failure ? 1 : 0);
