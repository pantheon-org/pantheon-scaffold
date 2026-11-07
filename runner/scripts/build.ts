// Builds the runner into one standalone binary per target, with the schema embedded and the version set, and writes checksums.txt.
// usage: bun runner/scripts/build.ts [--version <x.y.z>] [--out <dir>] [--targets linux-x64,linux-arm64,darwin-arm64]
// The release pipeline adds the package's checksum to checksums.txt and attests the file (wave D).
import { mkdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { formatChecksums } from "../src/checksums";
import { parseSemver } from "../src/semver";

export const TARGETS = ["linux-x64", "linux-arm64", "darwin-arm64"] as const;
export const binaryName = (target: string): string => `pantheon-scaffold-${target}`;

function args(argv: string[]): { version: string; out: string; targets: string[] } {
  const get = (flag: string, fallback: string): string => {
    const i = argv.indexOf(flag);
    return i === -1 ? fallback : (argv[i + 1] ?? fail(`${flag} needs a value`));
  };
  const version = get("--version", "0.0.0").replace(/^v/, "");
  if (!parseSemver(version)) fail(`${JSON.stringify(version)} is not a version`);
  const targets = get("--targets", TARGETS.join(",")).split(",");
  for (const t of targets) if (!(TARGETS as readonly string[]).includes(t)) fail(`unknown target ${t}, expected one of ${TARGETS.join(", ")}`);
  return { version, out: resolve(get("--out", "dist")), targets };
}

function fail(message: string): never {
  console.error(message);
  process.exit(2);
}

if (import.meta.main) {
  const { version, out, targets } = args(Bun.argv.slice(2));
  mkdirSync(out, { recursive: true });
  const entry = join(import.meta.dir, "../src/cli.ts");
  const sums = [];
  for (const target of targets) {
    const outfile = join(out, binaryName(target));
    const p = Bun.spawnSync(["bun", "build", "--compile", `--target=bun-${target}`, "--define", `BUILD_VERSION=${JSON.stringify(version)}`, entry, "--outfile", outfile], { stdout: "inherit", stderr: "inherit" });
    if (p.exitCode !== 0) fail(`building ${target} failed`);
    sums.push({ sha256: new Bun.CryptoHasher("sha256").update(readFileSync(outfile)).digest("hex"), name: binaryName(target) });
    console.log(`built ${outfile}`);
  }
  await Bun.write(join(out, "checksums.txt"), formatChecksums(sums));
  console.log(`wrote ${join(out, "checksums.txt")}`);
}
