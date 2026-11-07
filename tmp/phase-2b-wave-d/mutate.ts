// Mutation check for wave D: each entry breaks one property of the release logic or of .github/workflows/release.yml, and the release
// and pin tests must then fail. Run from the repository root: bun tmp/phase-2b-wave-d/mutate.ts
import { $ } from "bun";

const WF = ".github/workflows/release.yml";
const REL = "runner/src/release.ts";
const M: [string, string, string, string][] = [
  [REL, `if (declared !== m[1]) throw`, `if (false) throw`, "a tag must match PklProject"],
  [REL, `if (!m || !parseSemver(m[1])) throw`, `if (!m) throw`, "a tag must be v plus a version"],
  [REL, `if (meta.packageZipChecksums?.sha256 !== sha256(zipBytes)) throw`, `if (false) throw`, "the zip must match the metadata"],
  [REL, `if (bare(join(dir, \`\${pkg}.sha256\`)) !== sha256(metaBytes)) throw`, `if (false) throw`, "the .sha256 file must match the metadata"],
  [REL, `if (bare(join(dir, \`\${pkg}.zip.sha256\`)) !== sha256(zipBytes)) throw`, `if (false) throw`, "the .zip.sha256 file must match the zip"],
  [REL, `if (meta.version !== version) throw`, `if (false) throw`, "the metadata must be for this version"],
  [REL, `if (missing.length) throw`, `if (false) throw`, "every release file must exist"],
  [WF, `            --prerelease \\\n`, ``, "the release is a pre-release"],
  [WF, `            --verify-tag \\\n`, ``, "the tag must exist"],
  [WF, `      - name: Publish the pre-release\n        if: github.event_name == 'push'`, `      - name: Publish the pre-release`, "a dry run publishes nothing"],
  [WF, `        if: github.event_name == 'push'\n        with:\n          subject-path`, `        with:\n          subject-path`, "a dry run attests nothing"],
  [WF, `    name: Test and build\n    needs: prepare\n    runs-on: ubuntu-latest\n    timeout-minutes: 15\n    permissions:\n      contents: read`, `    name: Test and build\n    needs: prepare\n    runs-on: ubuntu-latest\n    timeout-minutes: 15\n    permissions:\n      contents: write`, "only the release job writes"],
  [WF, `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2`, `actions/upload-artifact@v7`, "every action is pinned by SHA"],
  [WF, `persist-credentials: false`, `persist-credentials: true`, "checkout keeps no credentials"],
  [WF, `check-tag "$REF_NAME"`, `check-tag "\${{ github.ref_name }}"`, "the tag name goes through env"],
  [WF, `cancel-in-progress: false`, `cancel-in-progress: true`, "a running release is not cancelled"],
  [WF, `gh attestation verify checksums.txt --repo "$REPO"`, `true`, "the published release is verified"],
  [WF, `tags: ["v[0-9]*"]`, `tags: ["v*"]`, "only version tags start a release"],
  [WF, `    needs: [prepare, release]\n    if: github.event_name == 'push'\n`, `    needs: [prepare, release]\n`, "the verify job is a real-release job"],
];

const files = [...new Set(M.map((m) => m[0]))];
const before = await Promise.all(files.map((f) => Bun.file(f).text()));
let missed = 0;
for (const [file, from, to, what] of M) {
  const original = await Bun.file(file).text();
  if (!original.includes(from)) { console.log(`SKIP   ${what}: pattern not found`); missed++; continue; }
  await Bun.write(file, original.replace(from, to));
  const r = await $`cd runner && bun test test/release.test.ts test/release-workflow.test.ts`.quiet().nothrow();
  await Bun.write(file, original);
  const caught = r.exitCode !== 0;
  if (!caught) missed++;
  console.log(`${caught ? "CAUGHT" : "MISSED"} ${what}`);
}
const after = await Promise.all(files.map((f) => Bun.file(f).text()));
const restored = before.every((text, i) => text === after[i]);
console.log(restored ? "restored: every mutated file is as it was" : "RESTORE FAILED: a file differs from before the run");
process.exit(missed || !restored ? 1 : 0);
