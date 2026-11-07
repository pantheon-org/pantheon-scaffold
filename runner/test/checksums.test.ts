import { expect, test } from "bun:test";
import { checksumAssetUrl, findChecksum, parseChecksums } from "../src/checksums";

const h = (c: string) => c.repeat(64);

test("parses sha256sum lines, with or without the binary marker, and ignores blank lines", () => {
  const text = `${h("a")}  pantheon-scaffold@0.2.0\n\n${h("b")} *runner-linux-x64\n`;
  expect(parseChecksums(text)).toEqual([
    { sha256: h("a"), name: "pantheon-scaffold@0.2.0" },
    { sha256: h("b"), name: "runner-linux-x64" },
  ]);
});

test("a line that is not a sha256sum line is an error, never skipped", () => {
  expect(() => parseChecksums("nonsense\n")).toThrow("not a checksum line");
  expect(() => parseChecksums(`${"a".repeat(63)}  x\n`)).toThrow("not a checksum line");
  expect(() => parseChecksums(`${"G".repeat(64)}  x\n`)).toThrow("not a checksum line");
});

test("an empty file has no checksums", () => expect(parseChecksums("\n\n")).toEqual([]));

test("finds the checksum of one named file", () => {
  expect(findChecksum(`${h("a")}  one\n${h("b")}  two\n`, "two")).toBe(h("b"));
});

test("a missing name and a repeated name are both errors", () => {
  expect(() => findChecksum(`${h("a")}  one\n`, "two")).toThrow("no checksum for two");
  expect(() => findChecksum(`${h("a")}  one\n${h("b")}  one\n`, "one")).toThrow("more than one checksum for one");
});

test("the asset lives next to the release it describes", () => {
  expect(checksumAssetUrl("https://github.com/o/r/releases/download", "0.2.0")).toBe("https://github.com/o/r/releases/download/v0.2.0/checksums.txt");
  expect(checksumAssetUrl("https://x/", "0.2.0-rc.1")).toBe("https://x/v0.2.0-rc.1/checksums.txt");
});

test("formatChecksums writes sha256sum lines that parseChecksums reads back, sorted by name", async () => {
  const { formatChecksums } = await import("../src/checksums");
  const text = formatChecksums([
    { sha256: h("b"), name: "z-file" },
    { sha256: h("a"), name: "a-file" },
  ]);
  expect(text).toBe(`${h("a")}  a-file\n${h("b")}  z-file\n`);
  expect(parseChecksums(text)).toEqual([
    { sha256: h("a"), name: "a-file" },
    { sha256: h("b"), name: "z-file" },
  ]);
});

test("formatChecksums refuses a checksum or a name that parseChecksums could not read back", async () => {
  const { formatChecksums } = await import("../src/checksums");
  expect(() => formatChecksums([{ sha256: "abc", name: "x" }])).toThrow("not a sha256");
  expect(() => formatChecksums([{ sha256: h("a"), name: "two\nlines" }])).toThrow("not a valid file name");
  expect(() => formatChecksums([{ sha256: h("a"), name: "" }])).toThrow("not a valid file name");
});
