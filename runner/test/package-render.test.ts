import { afterEach, expect, test } from "bun:test";
import { join } from "node:path";
import { cleanupRepos, get, tempRepo, unpin } from "./helpers";
import { runScaffold } from "./support/contract";
import { compareWorkflows } from "./support/equality";

// Ported from runner/test/cli.test.ts when the TypeScript runner was retired: the workflows the Rust runner writes carry the version comment of
// their pin and still equal the pinned upstream copies under the P.1 rule (docs/ts-runner-test-mapping.md).
afterEach(cleanupRepos);
const root = join(import.meta.dir, "../..");

test("the rendered workflows carry version comments and still equal the pinned upstream copies", async () => {
  const repo = tempRepo();
  const r = await runScaffold(["render", join(root, "runner/test/fixtures/slice/pantheon.pkl"), "--root", repo, "--allow-local-package", join(root, "pkl")], root);
  expect(r.code).toBe(0);
  for (const name of ["cargo-deny", "inline-scripts", "rust-ci"]) {
    const text = get(repo, `.github/workflows/${name}.yml`);
    expect(text).toMatch(/actions\/checkout@[0-9a-f]{40} # v\d/);
    const upstream = await Bun.file(join(root, `runner/test/upstream/workflows/${name}.yml`)).text();
    expect(compareWorkflows(unpin(upstream), unpin(text))).toEqual({ equal: true, differences: [], duplicateKeys: [] });
  }
});
