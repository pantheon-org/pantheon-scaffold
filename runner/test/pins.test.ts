import { afterEach, expect, test } from "bun:test";
import { PinError, RUNNER_TOOL, readPins, writePins } from "../src/pins";
import { cleanupRepos, get, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const h = (c: string) => c.repeat(64);
const amends = (v: string, sum?: string) =>
  `amends "package://github.com/pantheon-org/pantheon-scaffold/releases/download/v${v}/pantheon-scaffold@${v}${sum ? `::sha256:${sum}` : ""}#/Config.pkl"`;
const pkl = (v: string, sum?: string) => `${amends(v, sum)}\n\noptions { toolchain = "1.96.1" }\n`;
const mise = (v: string) => `[tools]\nbun = "1.4.2"\n"${RUNNER_TOOL}" = "${v}"\npkl = "0.32.2"\n`;

const repo = (v = "0.1.0", sum: string | null = h("a")) => {
  const root = tempRepo();
  put(root, "pantheon.pkl", pkl(v, sum ?? undefined));
  put(root, "mise.toml", mise(v));
  return root;
};

test("reads the package version, its checksum and the runner pin", () => {
  expect(readPins(repo(), "pantheon.pkl")).toEqual({ packageVersion: "0.1.0", checksum: h("a"), runnerVersion: "0.1.0" });
});
test("a file with no checksum yet reads as none", () => expect(readPins(repo("0.1.0", null), "pantheon.pkl").checksum).toBeUndefined());

test("writes the new version into the tag, the file name and the runner pin, and the new checksum, touching nothing else", async () => {
  const root = repo();
  writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: h("c") });
  expect(get(root, "pantheon.pkl")).toBe(pkl("0.2.0", h("c")));
  expect(get(root, "mise.toml")).toBe(mise("0.2.0"));
});
test("a checksum is added when there was none", () => {
  const root = repo("0.1.0", null);
  writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: h("c") });
  expect(get(root, "pantheon.pkl")).toContain(`pantheon-scaffold@0.2.0::sha256:${h("c")}#/Config.pkl`);
});
test("a pre-release version is written as given", () => {
  const root = repo();
  writePins(root, "pantheon.pkl", { version: "0.2.0-rc.1", checksum: h("c") });
  expect(readPins(root, "pantheon.pkl").packageVersion).toBe("0.2.0-rc.1");
});
test("the table form of the runner pin is rewritten too", () => {
  const root = repo();
  put(root, "mise.toml", `[tools]\n"${RUNNER_TOOL}" = { version = "0.1.0", version_prefix = "v" }\n`);
  writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: h("c") });
  expect(get(root, "mise.toml")).toBe(`[tools]\n"${RUNNER_TOOL}" = { version = "0.2.0", version_prefix = "v" }\n`);
});

test("a pantheon.pkl with no amends line for the package, or two, is an error that names the file", () => {
  const none = repo();
  put(none, "pantheon.pkl", `amends "../local/Config.pkl"\n`);
  expect(() => readPins(none, "pantheon.pkl")).toThrow(PinError);
  expect(() => readPins(none, "pantheon.pkl")).toThrow("pantheon.pkl");
  const two = repo();
  put(two, "pantheon.pkl", `${amends("0.1.0")}\n${amends("0.1.0")}\n`);
  expect(() => readPins(two, "pantheon.pkl")).toThrow("more than one");
});
test("a mise.toml without the runner tool, or with two, is an error", () => {
  const none = repo();
  put(none, "mise.toml", `[tools]\nbun = "1.4.2"\n`);
  expect(() => readPins(none, "pantheon.pkl")).toThrow(`no ${RUNNER_TOOL}`);
  const two = repo();
  put(two, "mise.toml", `${mise("0.1.0")}"${RUNNER_TOOL}" = "0.1.0"\n`);
  expect(() => readPins(two, "pantheon.pkl")).toThrow("more than one");
});
test("a missing file is an error, not a crash", () => {
  expect(() => readPins(tempRepo(), "pantheon.pkl")).toThrow(PinError);
});
test("nothing is written when one of the two files cannot be rewritten", () => {
  const root = repo();
  put(root, "mise.toml", `[tools]\nbun = "1.4.2"\n`);
  const before = get(root, "pantheon.pkl");
  expect(() => writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: h("c") })).toThrow(PinError);
  expect(get(root, "pantheon.pkl")).toBe(before);
});
test("a version or checksum that is not valid is refused before anything is written", () => {
  const root = repo();
  expect(() => writePins(root, "pantheon.pkl", { version: "latest", checksum: h("c") })).toThrow("not a version");
  expect(() => writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: "abc" })).toThrow("checksum");
});

// The form mise itself writes when a tool has options: a section with a version line.
const section = (v: string, extra = "") => `[settings]\nlockfile = true\n\n[tools]\npkl = "0.32.2"\n\n[tools."${RUNNER_TOOL}"]\n${extra}version = "${v}"\n\n[env]\nX = "1"\nversion = "9.9.9"\n`;

test("the section form of the runner pin is read and rewritten, and nothing else with a version line is touched", () => {
  const root = repo();
  put(root, "mise.toml", section("0.1.0"));
  expect(readPins(root, "pantheon.pkl").runnerVersion).toBe("0.1.0");
  writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: h("c") });
  expect(get(root, "mise.toml")).toBe(section("0.2.0"));
});
test("a section with other options before the version is handled", () => {
  const root = repo();
  put(root, "mise.toml", section("0.1.0", 'version_prefix = "v"\n'));
  writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: h("c") });
  expect(get(root, "mise.toml")).toBe(section("0.2.0", 'version_prefix = "v"\n'));
});
test("the pin in a section that is the last thing in the file is handled", () => {
  const root = repo();
  put(root, "mise.toml", `[tools."${RUNNER_TOOL}"]\nversion = "0.1.0"`);
  writePins(root, "pantheon.pkl", { version: "0.2.0", checksum: h("c") });
  expect(get(root, "mise.toml")).toBe(`[tools."${RUNNER_TOOL}"]\nversion = "0.2.0"`);
});
test("a section with no version line is an error, and so is a section plus a one-line entry", () => {
  const none = repo();
  put(none, "mise.toml", `[tools."${RUNNER_TOOL}"]\nversion_prefix = "v"\n\n[env]\nversion = "9.9.9"\n`);
  expect(() => readPins(none, "pantheon.pkl")).toThrow(`no ${RUNNER_TOOL}`);
  const two = repo();
  put(two, "mise.toml", `${section("0.1.0")}\n[tools]\n"${RUNNER_TOOL}" = "0.1.0"\n`);
  expect(() => readPins(two, "pantheon.pkl")).toThrow("more than one");
});
test("a section for a different tool is not mistaken for the runner", () => {
  const root = repo();
  put(root, "mise.toml", `[tools."github:other/tool"]\nversion = "1.0.0"\n`);
  expect(() => readPins(root, "pantheon.pkl")).toThrow(`no ${RUNNER_TOOL}`);
});
