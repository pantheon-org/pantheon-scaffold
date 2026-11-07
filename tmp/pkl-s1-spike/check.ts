// Local half of S.1: package build, consumption through --http-rewrite, inline checksum, offline cache,
// pre-release exact pin and the allowed-modules flags. Run from this folder: bun check.ts
import { $ } from "bun";
import { mkdtempSync, cpSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, basename } from "node:path";

const here = import.meta.dir;
const work = mkdtempSync(join(tmpdir(), "s1-"));
if (!work.includes("s1-")) throw new Error("no temp dir");
const BASE = "package://github.com/pantheon-org/pantheon-scaffold-spike/releases/download";

async function build(version: string) {
  const dir = join(work, `pkg-${version}`);
  cpSync(join(here, "pkg"), dir, { recursive: true });
  const p = join(dir, "PklProject");
  writeFileSync(p, readFileSync(p, "utf8").replace('version = "0.0.1"', `version = "${version}"`));
  const out = join(work, `out-${version}`);
  await $`pkl project package ${dir} --output-path ${out}`.quiet();
  const meta = readFileSync(join(out, `pantheon-scaffold-spike@${version}`));
  const sha = new Bun.CryptoHasher("sha256").update(meta).digest("hex");
  return { out, sha };
}

const stable = await build("0.0.1");
const rc = await build("0.0.2-rc.1");
const dirs = [stable.out, rc.out];

let server: ReturnType<typeof Bun.serve> | undefined;
const startServer = (port = 0) => Bun.serve({
  port,
  fetch(req) {
    const name = decodeURIComponent(basename(new URL(req.url).pathname));
    for (const d of dirs) {
      const f = Bun.file(join(d, name));
      if (f.size) return new Response(f);
    }
    return new Response("not found", { status: 404 });
  },
});
server = startServer();
const port = server.port;
const rewrite = (p: number) => `https://github.com/=http://localhost:${p}/`;

function consumer(version: string, sha?: string, toolchain = "1.91.0") {
  const dir = mkdtempSync(join(work, "consumer-"));
  const hash = sha ? `::sha256:${sha}` : "";
  writeFileSync(join(dir, "pantheon.pkl"),
    `amends "${BASE}/v${version}/pantheon-scaffold-spike@${version}${hash}#/Config.pkl"\ntoolchain = "${toolchain}"\n`);
  return join(dir, "pantheon.pkl");
}

let failures = 0;
const check = (name: string, ok: boolean, detail = "") => {
  console.log(`${ok ? "PASS" : "FAIL"}  ${name}${detail ? "  " + detail : ""}`);
  if (!ok) failures++;
};
const DEFAULT_RES = "env:,prop:,package:,projectpackage:,https:,http:,file:";
const run = async (file: string, cache: string, opts: { modules?: string; resources?: string } = {}, p = port) => {
  const flags = ["--allowed-resources", opts.resources ?? DEFAULT_RES, ...(opts.modules ? ["--allowed-modules", opts.modules] : [])];
  const r = await $`pkl eval --cache-dir ${cache} --http-rewrite ${rewrite(p)} ${flags} -x output.text ${file}`.quiet().nothrow();
  return { code: r.exitCode, out: r.stdout.toString(), err: r.stderr.toString() };
};

const metaJson = JSON.parse(readFileSync(join(stable.out, "pantheon-scaffold-spike@0.0.1"), "utf8"));
check("asset names are <name>@<version>, <name>@<version>.zip and a .sha256 beside each",
  ["pantheon-scaffold-spike@0.0.1", "pantheon-scaffold-spike@0.0.1.zip", "pantheon-scaffold-spike@0.0.1.sha256", "pantheon-scaffold-spike@0.0.1.zip.sha256"]
    .every((n) => Bun.file(join(stable.out, n)).size > 0));
check("metadata packageUri carries the tag path and the version", metaJson.packageUri === `${BASE}/v0.0.1/pantheon-scaffold-spike@0.0.1`, metaJson.packageUri);
check("metadata packageZipUrl points at the release asset", metaJson.packageZipUrl.endsWith("/v0.0.1/pantheon-scaffold-spike@0.0.1.zip"));
check("metadata holds the zip sha256", /^[0-9a-f]{64}$/.test(metaJson.packageZipChecksums.sha256));
check("the .sha256 file is the checksum of the metadata file", readFileSync(join(stable.out, "pantheon-scaffold-spike@0.0.1.sha256"), "utf8").trim().startsWith(stable.sha));

const cache = mkdtempSync(join(work, "cache-"));
const noSum = await run(consumer("0.0.1"), cache);
check("amends without a checksum renders (control)", noSum.code === 0 && noSum.out.includes("toolchain = 1.91.0"), noSum.err.slice(0, 120));

const cache2 = mkdtempSync(join(work, "cache-"));
const good = await run(consumer("0.0.1", stable.sha), cache2);
check("inline ::sha256 of the metadata file works", good.code === 0 && good.out.includes("toolchain = 1.91.0"), good.err.slice(0, 160));

const cache3 = mkdtempSync(join(work, "cache-"));
const bad = await run(consumer("0.0.1", "0".repeat(64)), cache3);
check("a wrong inline checksum is rejected", bad.code !== 0, bad.err.split("\n").find((l) => /checksum/i.test(l)) ?? bad.err.slice(0, 120));

const sumOfZip = await run(consumer("0.0.1", metaJson.packageZipChecksums.sha256), mkdtempSync(join(work, "cache-")));
check("the zip checksum is not accepted in place of the metadata checksum", sumOfZip.code !== 0);

// Offline: warm cache, then stop the server.
const warm = mkdtempSync(join(work, "cache-"));
await run(consumer("0.0.1", stable.sha), warm);
await server.stop(true);
const offlineWarm = await run(consumer("0.0.1", stable.sha), warm);
check("offline with a warm cache renders", offlineWarm.code === 0 && offlineWarm.out.includes("toolchain = 1.91.0"), offlineWarm.err.slice(0, 160));
const offlineCold = await run(consumer("0.0.1", stable.sha), mkdtempSync(join(work, "cache-")));
check("offline with a cold cache fails", offlineCold.code !== 0);
server = startServer(port);

// Pre-release: pkl has no latest or range resolution for package URIs, so an exact version is the only form.
const rcRun = await run(consumer("0.0.2-rc.1", rc.sha), mkdtempSync(join(work, "cache-")));
check("a pre-release version resolves by exact pin like any other version", rcRun.code === 0 && rcRun.out.includes("toolchain = 1.91.0"), rcRun.err.slice(0, 160));

// Allowed flags. The package fetch is a resource read of the https URL (checked before the rewrite), and the
// local rewrite adds a read of an http URL, which is only for this test.
const f = consumer("0.0.1", stable.sha);
const okFlags = await run(f, mkdtempSync(join(work, "cache-")), { modules: "pkl:,file:,package:", resources: "package:,https:,http:" });
check("--allowed-modules pkl:,file:,package: with --allowed-resources package:,https: allows the fetch", okFlags.code === 0, okFlags.err.slice(0, 200));
const noPkg = await run(f, mkdtempSync(join(work, "cache-")), { modules: "pkl:,file:" });
check("--allowed-modules without package: blocks the package", noPkg.code !== 0);
const noHttps = await run(f, mkdtempSync(join(work, "cache-")), { resources: "package:,http:,file:,prop:" });
check("--allowed-resources without https: blocks the fetch, so the runner flags must include https:", noHttps.code !== 0, (noHttps.err.split("\n").find((l) => /Refusing/.test(l)) ?? "").slice(0, 110));
const noPkgRes = await run(f, mkdtempSync(join(work, "cache-")), { resources: "https:,http:,file:,prop:" });
check("--allowed-resources without package: still allows the fetch", noPkgRes.code === 0, noPkgRes.err.slice(0, 160));

await server.stop(true);
rmSync(work, { recursive: true, force: true });
if (failures) { console.error(`${failures} failing`); process.exit(1); }
console.log("all passed");
