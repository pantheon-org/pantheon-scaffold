import { assetPath, expectPins, unpin } from "./helpers";
import { expect, test } from "bun:test";
import { join } from "node:path";
import { parseContract } from "../src/contract";
import { compareWorkflows } from "./support/equality";
import { evalContractJson } from "./support/contract";
import type { Schema } from "../src/schema";

const root = join(import.meta.dir, "../..");
const read = (p: string) => Bun.file(join(root, p)).text();
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const contract = parseContract(await evalContractJson(join(import.meta.dir, "fixtures/slice/pantheon.pkl"), root, { allowFilesUnder: [join(root, "pkl")] }), schema);
const file = (path: string) => contract.files.find((f) => f.path === path)!;

test("the slice renders the three workflows, five verbatim files and no external file", () => {
  expect(contract.files.map((f) => f.path)).toEqual([
    ".github/scripts/cargo-deny.sh",
    ".github/scripts/check-inline-scripts.sh",
    ".github/scripts/check-toolchain-version.sh",
    ".github/workflows/cargo-deny.yml",
    ".github/workflows/inline-scripts.yml",
    ".github/workflows/rust-ci.yml",
    "deny.toml",
    "rust-toolchain.toml",
  ]);
});

for (const name of ["cargo-deny", "inline-scripts", "rust-ci"]) {
  test(`${name}.yml equals the pinned pantheon-adr copy under the P.1 rule`, async () => {
    const upstream = await read(`runner/test/upstream/workflows/${name}.yml`);
    const result = compareWorkflows(unpin(upstream), unpin(file(`.github/workflows/${name}.yml`).content!));
    expect(result).toEqual({ equal: true, differences: [], duplicateKeys: [] });
  });
}

for (const name of ["cargo-deny", "check-inline-scripts", "check-toolchain-version"]) {
  test(`${name}.sh is byte for byte the asset and executable`, async () => {
    const f = file(`.github/scripts/${name}.sh`);
    expect(f.content).toBe(await read(assetPath(`${name}.sh`)));
    expect(f.mode).toBe("0755");
  });
}

test("deny.toml is byte for byte the asset and not executable", async () => {
  expect(file("deny.toml").content).toBe(await read(assetPath("deny.toml")));
  expect(file("deny.toml").mode).toBe("0644");
});

test("rust-toolchain.toml carries the one toolchain option", async () => {
  expect(file("rust-toolchain.toml").content).toBe((await read(assetPath("rust-toolchain.toml.tmpl"))).replace("@@TOOLCHAIN@@", "1.96.1"));
});

test("the toolchain option reaches both workflows that install it", () => {
  for (const name of ["cargo-deny", "rust-ci"]) expect(file(`.github/workflows/${name}.yml`).content).toContain("rustup toolchain install 1.96.1");
});

test("the slice uses no escape hatch", () => expect(contract.escapeHatch.uses).toBe(0));
test("the only pin is checkout at its release tag", () => expectPins(contract.pins, ["actions/checkout"]));

// Non-circular byte fidelity: the git blob hash of what pkl renders must equal the blob hash of the pinned upstream file.
const gitBlobSha = (text: string): string => {
  const body = new TextEncoder().encode(text);
  return new Bun.CryptoHasher("sha1").update(`blob ${body.length}\0`).update(body).digest("hex");
};
const blobs = (await Bun.file(join(import.meta.dir, "upstream/blobs.json")).json()) as Record<string, string>;
for (const [path, blob] of Object.entries(blobs)) {
  test(`${path} renders byte for byte as the pinned pantheon-adr file (git blob ${blob.slice(0, 7)})`, () => {
    expect(gitBlobSha(file(path).content!)).toBe(blob);
  });
}
