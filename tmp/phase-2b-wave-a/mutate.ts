// Mutation check for wave A: each entry breaks one behaviour, and the suite must then fail. Run from runner/: bun ../tmp/phase-2b-wave-a/mutate.ts
import { $ } from "bun";
const M = [
  ["src/guard.ts", `!k.startsWith("GIT_")`, `true`, "git environment stripped"],
  ["src/guard.ts", `if (!tracked.ok)`, `if (false)`, "untracked file never deleted"],
  ["src/guard.ts", `else if (status.out.trim() !== "")`, `else if (false)`, "dirty tree refused"],
  ["src/guard.ts", `if (sparse || partial)`, `if (false)`, "sparse or partial refused"],
  ["src/lock.ts", `const gone = !ci && pid`, `const gone = pid`, "CI ignores the process id"],
  ["src/lock.ts", `const old = Date.now() - startedMs > staleMs;`, `const old = false;`, "age-based recovery"],
  ["src/apply.ts", `if (deletes.length) refusals.push(...deleteGuard(root, deletes));`, ``, "guard called before writes"],
  ["src/apply.ts", `for (const path of deletes) rmSync(resolveInside(root, path));`, ``, "orphan deleted"],
  ["src/apply.ts", `for (const f of chmods) chmodSync(`, `for (const f of [] as ContractFile[]) chmodSync(`, "exec bit applied"],
  ["src/apply.ts", `const release = opts.dryRun ? () => {} : acquireLock(root, opts.lock);`, `const release = () => {};`, "lock taken"],
  ["src/apply.ts", `else if (entry.hash !== undefined && sha256(current) !== entry.hash && !opts.force)`, `else if (false)`, "edited orphan refused"],
  ["src/check.ts", `if (wantExec !== isExec)`, `if (false)`, "check reports mode drift"],
] as const;
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP  ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, to));
  const r = await $`bun test`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await $`bun test`.quiet().nothrow();
console.log(after.exitCode === 0 ? "restored: suite green" : "RESTORE FAILED");
process.exit(missed || after.exitCode ? 1 : 0);
