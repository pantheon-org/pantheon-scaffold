// Builds the Rust runner for one release target, on a machine of that target, and puts it where the release expects it: one file
// named pantheon-scaffold-<target>. The release workflow and the pull request checks both run this script, so a pull request builds
// the release binary exactly as a tag will.
//
// The Linux builds are static musl binaries, which run on any Linux distribution whatever its glibc; the macOS build is a normal one.
// After the build the script checks what it made: the binary reports the version it was built for, and a Linux binary is static.
// usage: bun runner/scripts/build-rust.ts --target <linux-x64|linux-arm64|darwin-arm64> [--version <x.y.z>] [--out <dir>]
import { chmodSync, copyFileSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { TARGETS, binaryName } from "../src/release";
import { parseSemver } from "../src/semver";

export const TRIPLES = {
  "linux-x64": "x86_64-unknown-linux-musl",
  "linux-arm64": "aarch64-unknown-linux-musl",
  "darwin-arm64": "aarch64-apple-darwin",
} as const satisfies Record<(typeof TARGETS)[number], string>;

export type Plan = {
  triple: string;
  cargo: string[];
  env: Record<string, string>;
  /** Where cargo leaves the binary. */
  built: string;
  /** Where the release expects it. */
  asset: string;
  mustBeStatic: boolean;
};

/** What to run for a target. Nothing is run here, so it can be checked without a toolchain. */
export function plan(target: string, version: string, out: string, root: string): Plan {
  if (!(TARGETS as readonly string[]).includes(target)) throw new Error(`unknown target ${target}, expected one of ${TARGETS.join(", ")}`);
  const bare = version.replace(/^v/, "");
  if (!parseSemver(bare)) throw new Error(`${JSON.stringify(version)} is not a version`);
  const triple = TRIPLES[target as keyof typeof TRIPLES];
  const musl = triple.endsWith("-musl");
  return {
    triple,
    cargo: ["build", "--release", "--locked", "-p", "scaffold-cli", "--target", triple],
    // The version is baked into the binary at compile time. ring, which rustls uses, has C code: musl-gcc compiles it against musl.
    env: { SCAFFOLD_VERSION: bare, ...(musl ? { [`CC_${triple.replaceAll("-", "_")}`]: "musl-gcc" } : {}) },
    built: join(root, "target", triple, "release", "pantheon-scaffold"),
    asset: join(out, binaryName(target)),
    mustBeStatic: musl,
  };
}

/** `ldd` on a static binary says so, in one of two words depending on how it was linked. */
export const isStatic = (ldd: string): boolean => /not a dynamic executable|statically linked/.test(ldd);

/** The target this machine can build, or null. Only native builds are made: nothing here cross-compiles. */
export function hostTarget(platform: string, arch: string): (typeof TARGETS)[number] | null {
  if (platform === "linux" && arch === "x64") return "linux-x64";
  if (platform === "linux" && arch === "arm64") return "linux-arm64";
  if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
  return null;
}

export function parseArgs(argv: string[]): { target: string; version: string; out: string } {
  const get = (flag: string, fallback?: string): string => {
    const i = argv.indexOf(flag);
    const value = i === -1 ? fallback : argv[i + 1];
    if (value === undefined) throw new Error(`${flag} needs a value`);
    return value;
  };
  return { target: get("--target"), version: get("--version", "0.0.0"), out: resolve(get("--out", "dist")) };
}

function run(cmd: string[], env: Record<string, string> = {}): void {
  const p = Bun.spawnSync(cmd, { stdout: "inherit", stderr: "inherit", env: { ...process.env, ...env } });
  if (p.exitCode !== 0) throw new Error(`${cmd.join(" ")} failed (exit ${p.exitCode})`);
}

if (import.meta.main) {
  try {
    const { target, version, out } = parseArgs(Bun.argv.slice(2));
    const root = resolve(import.meta.dir, "../..");
    const p = plan(target, version, out, root);
    const here = hostTarget(process.platform, process.arch);
    if (here !== target) throw new Error(`this machine builds ${here ?? "no release target"}, not ${target}: build each target on a machine of its own`);
    run(["rustup", "target", "add", p.triple]);
    run(["cargo", ...p.cargo], p.env);
    mkdirSync(out, { recursive: true });
    copyFileSync(p.built, p.asset);
    chmodSync(p.asset, 0o755);

    const reported = Bun.spawnSync([p.asset, "--version"], { stdout: "pipe", stderr: "pipe" }).stdout.toString().trim();
    if (reported !== version.replace(/^v/, "")) throw new Error(`the binary reports version ${JSON.stringify(reported)}, expected ${version.replace(/^v/, "")}`);
    if (p.mustBeStatic && process.platform === "linux") {
      const ldd = Bun.spawnSync(["ldd", p.asset], { stdout: "pipe", stderr: "pipe" });
      const said = ldd.stdout.toString() + ldd.stderr.toString();
      if (!isStatic(said)) throw new Error(`${p.asset} is not a static binary:\n${said.trim()}`);
    }
    console.log(`built ${p.asset} (${p.triple}, version ${reported})`);
  } catch (e) {
    console.error((e as Error).message);
    process.exit(1);
  }
}
