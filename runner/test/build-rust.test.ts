import { expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
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

test("a Linux build compiles its C code with a zig cc wrapper for its own target, a macOS build is left alone", () => {
  const x64 = plan("linux-x64", "0.2.0", "/o", "/r");
  expect(x64.env.CC_x86_64_unknown_linux_musl).toBe("/r/target/zig-cc/cc-x86_64-unknown-linux-musl");
  expect(x64.wrapper?.path).toBe("/r/target/zig-cc/cc-x86_64-unknown-linux-musl");
  expect(x64.wrapper?.content).toContain("exec zig cc -target x86_64-linux-musl ");
  const arm = plan("linux-arm64", "0.2.0", "/o", "/r");
  expect(arm.env.CC_aarch64_unknown_linux_musl).toBe("/r/target/zig-cc/cc-aarch64-unknown-linux-musl");
  expect(arm.wrapper?.content).toContain("exec zig cc -target aarch64-linux-musl ");
  const mac = plan("darwin-arm64", "0.2.0", "/o", "/r");
  expect(Object.keys(mac.env)).toEqual(["SCAFFOLD_VERSION"]);
  expect(mac.wrapper).toBeNull();
});

test("nothing in a plan needs musl-gcc or the Ubuntu archive", () => {
  for (const t of TARGETS) expect(JSON.stringify(plan(t, "0.2.0", "/o", "/r"))).not.toContain("musl-gcc");
});

test("the wrapper hands zig the other arguments untouched, with spaces kept, and drops the --target the cc crate adds", () => {
  const dir = mkdtempSync(join(tmpdir(), "zig-wrapper-"));
  const p = plan("linux-x64", "0.2.0", "/o", dir);
  mkdirSync(dirname(p.wrapper!.path), { recursive: true });
  writeFileSync(p.wrapper!.path, p.wrapper!.content, { mode: 0o755 });
  const bin = join(dir, "bin");
  mkdirSync(bin);
  // A stand-in for zig that prints each argument on its own line.
  writeFileSync(join(bin, "zig"), '#!/bin/sh\nfor a in "$@"; do printf "%s\\n" "$a"; done\n', { mode: 0o755 });
  const out = Bun.spawnSync([p.wrapper!.path, "-O3", "--target=x86_64-unknown-linux-musl", "-I", "a dir/with space", "-c", "x.c"], { env: { ...process.env, PATH: `${bin}:${process.env.PATH}` } });
  expect(out.stdout.toString().split("\n").filter(Boolean)).toEqual(["cc", "-target", "x86_64-linux-musl", "-O3", "-I", "a dir/with space", "-c", "x.c"]);
  rmSync(dir, { recursive: true, force: true });
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
