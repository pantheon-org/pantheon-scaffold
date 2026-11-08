import { afterEach, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cleanupRepos, has, put, tempRepo } from "./helpers";

afterEach(cleanupRepos);

const script = join(import.meta.dir, "../scripts/update-pins.ts");
const sha = (c: string) => c.repeat(40);
const PINS = `module x\nclass Pin { action: String }\ncheckout: Pin = new { action = "actions/checkout"; sha = "${sha("a")}"; version = "v1.0.0" }\nsetup: Pin = new { action = "actions/setup-node"; sha = "${sha("b")}"; version = "v2.0.0" }\n`;

/** A fake GitHub. Latest releases and tag commits are given, everything else answers like a healthy repository. */
function github(o: { latest: Record<string, string>; commits: Record<string, string>; missingRelease?: string[] }) {
  return Bun.serve({
    port: 0,
    fetch(req) {
      const path = new URL(req.url).pathname;
      let m = /^\/repos\/([^/]+\/[^/]+)\/releases\/latest$/.exec(path);
      if (m) return o.latest[m[1]] ? Response.json({ tag_name: o.latest[m[1]] }) : new Response("{}", { status: 404 });
      m = /^\/repos\/([^/]+\/[^/]+)\/git\/ref\/tags\/(.+)$/.exec(path);
      if (m) return o.commits[`${m[1]}@${m[2]}`] ? Response.json({ object: { type: "commit", sha: o.commits[`${m[1]}@${m[2]}`] } }) : new Response("{}", { status: 404 });
      m = /^\/repos\/([^/]+\/[^/]+)\/releases\/tags\/(.+)$/.exec(path);
      if (m) return (o.missingRelease ?? []).includes(`${m[1]}@${m[2]}`) ? new Response("{}", { status: 404 }) : Response.json({ draft: false });
      if (/^\/repos\/[^/]+\/[^/]+\/commits\/[0-9a-f]{40}$/.test(path)) return Response.json({});
      return new Response("unexpected " + path, { status: 500 });
    },
  });
}

async function run(server: ReturnType<typeof github>, root: string, ...args: string[]) {
  const p = Bun.spawn(["bun", script, "--pins", join(root, "Pins.pkl"), ...args], { stdout: "pipe", stderr: "pipe", env: { ...process.env, PANTHEON_SCAFFOLD_GITHUB_API: `http://localhost:${server.port}` } });
  const [out, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { out: out.trim(), err: err.trim(), code };
}
const behind = () => github({ latest: { "actions/checkout": "v1.1.0", "actions/setup-node": "v2.0.0" }, commits: { "actions/checkout@v1.1.0": sha("c"), "actions/setup-node@v2.0.0": sha("b") } });

test("a no-write run lists the changes and writes nothing", async () => {
  const root = tempRepo();
  put(root, "Pins.pkl", PINS);
  const server = behind();
  try {
    const r = await run(server, root, "--no-write", "--freshness-out", join(root, "freshness.json"));
    expect(r.code).toBe(0);
    expect(r.out).toContain("actions/checkout v1.0.0 -> v1.1.0");
    expect(r.out).toContain("no-write run: nothing written");
    expect(readFileSync(join(root, "Pins.pkl"), "utf8")).toBe(PINS);
    expect(has(root, "freshness.json")).toBe(false);
  } finally {
    server.stop(true);
  }
});

test("a real run rewrites the changed pin, writes the freshness record and a summary for the pull request", async () => {
  const root = tempRepo();
  put(root, "Pins.pkl", PINS);
  const server = behind();
  try {
    const r = await run(server, root, "--freshness-out", join(root, "freshness.json"), "--summary", join(root, "summary.md"));
    expect(r.code).toBe(0);
    expect(readFileSync(join(root, "Pins.pkl"), "utf8")).toBe(PINS.replace(`sha = "${sha("a")}"; version = "v1.0.0"`, `sha = "${sha("c")}"; version = "v1.1.0"`));
    expect(JSON.parse(readFileSync(join(root, "freshness.json"), "utf8"))).toMatchObject({ schemaVersion: 1, lastSuccessfulPinCheck: expect.stringMatching(/Z$/) });
    const summary = readFileSync(join(root, "summary.md"), "utf8");
    expect(summary).toContain("`actions/checkout` v1.0.0 to v1.1.0");
    expect(summary).not.toContain("setup-node");
  } finally {
    server.stop(true);
  }
});

test("when nothing changed the freshness record is still written, and no summary is", async () => {
  const root = tempRepo();
  put(root, "Pins.pkl", PINS);
  const server = github({ latest: { "actions/checkout": "v1.0.0", "actions/setup-node": "v2.0.0" }, commits: { "actions/checkout@v1.0.0": sha("a"), "actions/setup-node@v2.0.0": sha("b") } });
  try {
    const r = await run(server, root, "--freshness-out", join(root, "freshness.json"), "--summary", join(root, "summary.md"));
    expect(r.code).toBe(0);
    expect(r.out).toContain("no pin changed");
    expect(has(root, "freshness.json")).toBe(true);
    expect(has(root, "summary.md")).toBe(false);
  } finally {
    server.stop(true);
  }
});

test("a problem fails the run and writes nothing, not even the freshness record", async () => {
  const root = tempRepo();
  put(root, "Pins.pkl", PINS);
  const server = github({ latest: { "actions/checkout": "v1.1.0", "actions/setup-node": "v2.0.0" }, commits: { "actions/checkout@v1.1.0": sha("c"), "actions/setup-node@v2.0.0": sha("b") }, missingRelease: ["actions/checkout@v1.1.0"] });
  try {
    const r = await run(server, root, "--freshness-out", join(root, "freshness.json"));
    expect(r.code).toBe(1);
    expect(r.err).toContain("not a published release");
    expect(readFileSync(join(root, "Pins.pkl"), "utf8")).toBe(PINS);
    expect(has(root, "freshness.json")).toBe(false);
  } finally {
    server.stop(true);
  }
});

test("a major bump is marked in the summary", async () => {
  const root = tempRepo();
  put(root, "Pins.pkl", PINS);
  const server = github({ latest: { "actions/checkout": "v2.0.0", "actions/setup-node": "v2.0.0" }, commits: { "actions/checkout@v2.0.0": sha("c"), "actions/setup-node@v2.0.0": sha("b") } });
  try {
    await run(server, root, "--summary", join(root, "summary.md"));
    expect(readFileSync(join(root, "summary.md"), "utf8")).toContain("major");
  } finally {
    server.stop(true);
  }
});

test("an unknown option is a usage error", async () => {
  const server = behind();
  try {
    expect((await run(server, tempRepo(), "--nope")).code).toBe(2);
  } finally {
    server.stop(true);
  }
});
