// Mutation check for wave B2: each entry breaks one behaviour, and the suite must then fail. Run from the repo root: bun tmp/phase-2b-wave-b2/mutate.ts
import { $ } from "bun";
const M = [
  ["runner/src/checksums.ts", `if (hits.length > 1) throw`, `if (false) throw`, "a repeated checksum name is an error"],
  ["runner/src/checksums.ts", `if (!m) throw new Error(`, `if (false) throw new Error(`, "a malformed checksum line is an error"],
  ["runner/src/pins.ts", `if (!parseSemver(next.version)) throw`, `if (false) throw`, "writePins refuses a bad version"],
  ["runner/src/pins.ts", `if (hits.length > 1) throw new PinError(\`\${file}: more than one`, `if (false) throw new PinError(\`\${file}: more than one`, "two amends lines are an error"],
  ["runner/src/pins.ts", `if (!/^[0-9a-f]{64}$/.test(next.checksum)) throw`, `if (false) throw`, "writePins refuses a bad checksum"],
  ["runner/src/upgrade.ts", `await deps.verifyAttestation(file, REPO);`, ``, "the attestation is verified"],
  ["runner/src/upgrade.ts", `restore(saved);`, ``, "files are put back when mise fails"],
  ["runner/src/upgrade.ts", `if (text === null) rmSync(path, { force: true });`, `if (text === null) {}`, "a lock file that did not exist is removed again"],
  ["runner/src/upgrade.ts", `if (opts.dryRun) return`, `if (false) return`, "--dry-run writes nothing"],
  ["runner/src/upgrade.ts", `compareSemver(to, from) < 0 && !opts.force`, `compareSemver(to, from) < 0 && false`, "an older version needs --force"],
  ["runner/src/upgrade.ts", `if (compareSemver(to, from) === 0) throw`, `if (false) throw`, "the same version is refused"],
  ["runner/src/upgrade.ts", `} finally {
    release();
  }
  if (opts.noRender)`, `} finally {
  }
  if (opts.noRender)`, "the lock is released before the render"],
  ["runner/src/upgrade.ts", `if (opts.noRender) return`, `if (false) return`, "--no-render stops after mise"],
  ["runner/src/cli.ts", `if (r.renderExit) {`, `if (false) {`, "the render's exit code is upgrade's exit code"],
  ["runner/src/cli.ts", `e instanceof PinError || e instanceof UpgradeError`, `false`, "upgrade errors print a clean message"],
] as const;
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, to));
  const r = await $`cd runner && bun test`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await $`cd runner && bun test`.quiet().nothrow();
console.log(after.exitCode === 0 ? "restored: suite green" : "RESTORE FAILED");
process.exit(missed || after.exitCode ? 1 : 0);
