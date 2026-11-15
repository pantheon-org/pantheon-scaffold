// A fake GitHub API for runner/scripts/update-pins-diff.sh. It serves the routes of a scenario file and records every request.
// usage: bun runner/scripts/fake-github-api.ts <scenario.json> <port-file> <request-log>
//   scenario.json: { "routes": { "/repos/o/r/releases/latest": [ { "status": 200, "body": { "tag_name": "v1" } } ], ... } }
//   Each request to a path takes the next answer of its list, and the last answer repeats. A path that is not listed answers 500.
//   The port the server listens on is written to <port-file> once it is ready. Each request is appended to <request-log> as
//   "GET <path> accept=<accept header> auth=<the Authorization header, or none>".
// This goes with the TypeScript tools when Phase 6 of docs/ts-tooling-rust-port.md deletes them.
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";

const [scenarioPath, portPath, logPath] = Bun.argv.slice(2);
if (!scenarioPath || !portPath || !logPath) {
  console.error("usage: fake-github-api.ts <scenario.json> <port-file> <request-log>");
  process.exit(2);
}
const routes: Record<string, { status: number; body: unknown }[]> = JSON.parse(readFileSync(scenarioPath, "utf8")).routes;

const server = Bun.serve({
  port: 0,
  fetch(req) {
    const url = new URL(req.url);
    const path = url.pathname + url.search;
    appendFileSync(logPath, `${req.method} ${path} accept=${req.headers.get("accept")} auth=${req.headers.get("authorization") ?? "none"}\n`);
    const answers = routes[path];
    if (!answers) return new Response("unexpected " + path, { status: 500 });
    const answer = answers.length > 1 ? answers.shift()! : answers[0];
    return Response.json(answer.body, { status: answer.status });
  },
});
writeFileSync(portPath, String(server.port));
