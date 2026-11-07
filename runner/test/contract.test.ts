import { expect, test } from "bun:test";
import { join } from "node:path";
import { parseContract, ContractError } from "../src/contract";
import { evalContractJson } from "../src/pkl";
import type { Schema } from "../src/schema";

const root = join(import.meta.dir, "../..");
const schema = (await Bun.file(join(root, "contract.schema.json")).json()) as Schema;
const fixture = (name: string) => join(import.meta.dir, "fixtures", name, "pantheon.pkl");

const base = () => ({
  schemaVersion: 1,
  files: [{ path: "a.txt", class: "owned", mode: "0644", content: "x\n" }],
  pins: [],
  escapeHatch: { uses: 0, constructs: 0 },
});
const rejects = (mutate: (c: any) => void, message: RegExp) => {
  const c = base();
  mutate(c);
  expect(() => parseContract(c, schema)).toThrow(message);
};

test("the pkl output of the minimal fixture satisfies the schema (contract test, both sides)", async () => {
  const contract = parseContract(await evalContractJson(fixture("minimal"), root), schema);
  expect(contract.files.map((f) => f.path)).toEqual([".github/scripts/hello.sh", ".github/workflows/hello.yml", "mise.lock", "rust-toolchain.toml"]);
  expect(contract.files.find((f) => f.path === "mise.lock")).toEqual({ path: "mise.lock", class: "external", mode: "0644" });
  expect(contract.files.find((f) => f.path === ".github/scripts/hello.sh")?.mode).toBe("0755");
  expect(contract.pins).toEqual([{ action: "actions/checkout", sha: "3d3c42e5aac5ba805825da76410c181273ba90b1", version: "v7.0.1" }]);
  expect(contract.escapeHatch).toEqual({ uses: 0, constructs: 4 });
});

test("the accepted example is valid", () => expect(parseContract(base(), schema).files).toHaveLength(1));
test("rejects an unknown schema version", () => rejects((c) => (c.schemaVersion = 2), /unsupported contract schemaVersion 2/));
test("rejects a missing schema version", () => rejects((c) => delete c.schemaVersion, /unsupported contract schemaVersion undefined/));
test("rejects an external file with content", () => rejects((c) => (c.files[0] = { path: "m.lock", class: "external", mode: "0644", content: "x" }), /external file must not carry content/));
test("rejects an owned file without content", () => rejects((c) => delete c.files[0].content, /owned file needs content/));
test("rejects a duplicate path", () => rejects((c) => c.files.push({ ...c.files[0] }), /a\.txt: listed twice/));
test("rejects a bad mode", () => rejects((c) => (c.files[0].mode = "0777"), /#\/files\/0\/mode: must be one of/));
test("rejects a bad pin sha", () => rejects((c) => c.pins.push({ action: "a/b", sha: "abc", version: "v1" }), /#\/pins\/0\/sha: does not match/));
test("rejects an unexpected top-level key", () => rejects((c) => (c.extra = 1), /#: unexpected extra/));
test("the ContractError type is exported", () => expect(new ContractError("x")).toBeInstanceOf(Error));

test("a pantheon.pkl cannot read arbitrary files (restricted resources)", async () => {
  await expect(evalContractJson(fixture("read-file"), root)).rejects.toThrow(/Refusing to read resource/);
});
