import { expect, test } from "bun:test";
import { TARGETS, binaryName } from "../src/release";
import { TRIPLES, hostTarget, isStatic, parseArgs, plan } from "../scripts/build-rust";

test("every release target has a Rust target, and the Linux ones are static musl builds", () => {
  expect(Object.keys(TRIPLES).sort()).toEqual([...TARGETS].sort());
  expect(TRIPLES).toEqual({ "linux-x64": "x86_64-unknown-linux-musl", "linux-arm64": "aarch64-unknown-linux-musl", "darwin-arm64": "aarch64-apple-darwin" });
});

test("the plan builds the command line crate, release, locked, for the target, with the version set", () => {
  const p = plan("linux-arm64", "0.2.0", "/out", "/repo");
  expect(p.triple).toBe("aarch64-unknown-linux-musl");
  expect(p.cargo).toEqual(["build", "--release", "--locked", "-p", "scaffold-cli", "--target", "aarch64-unknown-linux-musl"]);
  expect(p.env.SCAFFOLD_VERSION).toBe("0.2.0");
  expect(p.built).toBe("/repo/target/aarch64-unknown-linux-musl/release/pantheon-scaffold");
  expect(p.asset).toBe(`/out/${binaryName("linux-arm64")}`);
});

test("the asset names are the ones the release has always used, so mise picks the same files", () => {
  for (const t of TARGETS) expect(plan(t, "0.2.0", "/o", "/r").asset).toBe(`/o/pantheon-scaffold-${t}`);
});

test("a Linux build compiles its C code with musl-gcc, a macOS build is left alone", () => {
  expect(plan("linux-x64", "0.2.0", "/o", "/r").env.CC_x86_64_unknown_linux_musl).toBe("musl-gcc");
  expect(plan("linux-arm64", "0.2.0", "/o", "/r").env.CC_aarch64_unknown_linux_musl).toBe("musl-gcc");
  expect(Object.keys(plan("darwin-arm64", "0.2.0", "/o", "/r").env)).toEqual(["SCAFFOLD_VERSION"]);
});

test("only the musl builds must be static", () => {
  expect(plan("linux-x64", "0.2.0", "/o", "/r").mustBeStatic).toBe(true);
  expect(plan("linux-arm64", "0.2.0", "/o", "/r").mustBeStatic).toBe(true);
  expect(plan("darwin-arm64", "0.2.0", "/o", "/r").mustBeStatic).toBe(false);
});

test("a leading v is dropped from the version, and a pre-release is a version", () => {
  expect(plan("linux-x64", "v0.2.0-rc.1", "/o", "/r").env.SCAFFOLD_VERSION).toBe("0.2.0-rc.1");
});

test("an unknown target or a version that is not one is refused", () => {
  expect(() => plan("windows-x64", "0.2.0", "/o", "/r")).toThrow("unknown target windows-x64, expected one of linux-x64, linux-arm64, darwin-arm64");
  expect(() => plan("linux-x64", "latest", "/o", "/r")).toThrow('"latest" is not a version');
});

test("ldd output says whether a binary is static", () => {
  expect(isStatic("\tnot a dynamic executable\n")).toBe(true);
  expect(isStatic("\tstatically linked\n")).toBe(true);
  expect(isStatic("\tlinux-vdso.so.1 (0x00007ffd)\n\tlibc.so.6 => /lib/libc.so.6\n")).toBe(false);
  expect(isStatic("")).toBe(false);
});

test("the host decides which target can be built here", () => {
  expect(hostTarget("linux", "x64")).toBe("linux-x64");
  expect(hostTarget("linux", "arm64")).toBe("linux-arm64");
  expect(hostTarget("darwin", "arm64")).toBe("darwin-arm64");
  expect(hostTarget("darwin", "x64")).toBeNull();
  expect(hostTarget("win32", "x64")).toBeNull();
});

test("the arguments are --target, --version and --out, and the target is required", () => {
  expect(parseArgs(["--target", "linux-x64", "--version", "1.2.3", "--out", "dist"])).toMatchObject({ target: "linux-x64", version: "1.2.3" });
  expect(parseArgs(["--target", "linux-x64"]).version).toBe("0.0.0");
  expect(() => parseArgs([])).toThrow("--target needs a value");
  expect(() => parseArgs(["--target"])).toThrow("--target needs a value");
  expect(() => parseArgs(["--target", "linux-x64", "--version"])).toThrow("--version needs a value");
});
