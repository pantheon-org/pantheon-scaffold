import { afterEach, expect, test } from "bun:test";
import { ManifestError, emptyManifest, readManifest, serialiseManifest, sha256 } from "../src/manifest";
import { cleanupRepos, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

test("serialisation is sorted, indented and ends with one newline", () => {
  const m = emptyManifest();
  m.files["b"] = { class: "owned", hash: "2" };
  m.files["a"] = { class: "external" };
  expect(serialiseManifest(m)).toBe(`{\n  "manifestVersion": 1,\n  "files": {\n    "a": {\n      "class": "external"\n    },\n    "b": {\n      "class": "owned",\n      "hash": "2"\n    }\n  }\n}\n`);
});
test("serialising twice gives the same text", () => {
  const m = emptyManifest();
  m.files["x"] = { class: "seed", hash: "h" };
  expect(serialiseManifest(m)).toBe(serialiseManifest(JSON.parse(serialiseManifest(m))));
});
test("a missing manifest reads as empty", async () => expect(await readManifest(tempRepo())).toEqual(emptyManifest()));
test("a manifest round-trips from disk", async () => {
  const root = tempRepo();
  const m = emptyManifest();
  m.files["a"] = { class: "owned", hash: sha256("x") };
  put(root, ".pkl-gen/manifest.json", serialiseManifest(m));
  expect(await readManifest(root)).toEqual(m);
});
test("an unknown manifest version is refused", async () => {
  const root = tempRepo();
  put(root, ".pkl-gen/manifest.json", '{"manifestVersion":2,"files":{}}');
  await expect(readManifest(root)).rejects.toThrow(/manifestVersion 2/);
});
test("a manifest with merge conflict markers points at --repair", async () => {
  const root = tempRepo();
  put(root, ".pkl-gen/manifest.json", "<<<<<<< HEAD\n{}\n=======\n{}\n>>>>>>> x\n");
  await expect(readManifest(root)).rejects.toThrow(/--repair/);
});
test("sha256 matches a known value", () => expect(sha256("abc")).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
test("the ManifestError type is exported", () => expect(new ManifestError("x")).toBeInstanceOf(Error));

test("the package version and the detach flag are kept, and the package version is written right after the manifest version", () => {
  const m = emptyManifest();
  m.packageVersion = "0.2.0";
  m.files["w.yml"] = { class: "owned", hash: "h", removeOnDetach: true };
  const text = serialiseManifest(m);
  expect(text).toBe(`{\n  "manifestVersion": 1,\n  "packageVersion": "0.2.0",\n  "files": {\n    "w.yml": {\n      "class": "owned",\n      "hash": "h",\n      "removeOnDetach": true\n    }\n  }\n}\n`);
  expect(JSON.parse(text)).toEqual(m);
});
test("a manifest without a package version serialises exactly as before", () => {
  expect(serialiseManifest(emptyManifest())).toBe(`{\n  "manifestVersion": 1,\n  "files": {}\n}\n`);
});
test("a package version that is not a string is refused", async () => {
  const root = tempRepo();
  put(root, ".pkl-gen/manifest.json", '{"manifestVersion":1,"packageVersion":3,"files":{}}');
  await expect(readManifest(root)).rejects.toThrow(/packageVersion/);
});
