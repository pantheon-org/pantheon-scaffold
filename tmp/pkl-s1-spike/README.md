# pkl-s1-spike

Local half of spike S.1 of the pkl plan: build a minimal pkl package and consume it through a local server by rewriting
`https://github.com/` with pkl's `--http-rewrite`. Run on 07-11-2026 with pkl 0.32.2 and bun 1.4.2. Untracked.
Re-run: `bun check.ts` in this folder (16 assertions, temp directories only, nothing is pushed anywhere).

| Question | Answer |
|---|---|
| Asset names | `pkl project package` writes `<name>@<version>` (metadata JSON), `<name>@<version>.zip`, and a `.sha256` file beside each. |
| Package URI and zip URL | `baseUri` may carry the tag path (`.../releases/download/v<version>/<name>`), so the package URI is `package://github.com/<org>/<repo>/releases/download/v<version>/<name>@<version>`. `packageZipUrl` points at the zip release asset. |
| Inline `::sha256` on the amends URI | Works, and it is the checksum of the metadata file (the `.sha256` asset), not of the zip. A wrong value is rejected with a clear error. The zip checksum is not accepted in its place. |
| Offline | A warm pkl cache renders with the server stopped. A cold cache fails. |
| Pre-release | pkl resolves any exact version the same way: there is no latest or range lookup, so the GitHub pre-release flag does not affect pkl. Whether a pre-release GitHub release serves its assets at the download URL still needs the GitHub half. |
| Allowed flags | The package fetch is a resource read of the `https` URL. `--allowed-modules` needs `package:`, and `--allowed-resources` needs `https:`. It does not need `package:` in resources. (The `http:` entry in the test is only for the local rewrite.) |

Not covered here (needs the real GitHub release in `pantheon-org/pantheon-scaffold-spike`, a separate approved step):
the redirect from `github.com/.../releases/download` to GitHub's asset storage, a pre-release release's asset URL,
mise installing the runner, and the hk interaction under a consumer `PklProject` fallback.
The package metadata author is a placeholder address, not a real mailbox.
The `console.log` lines in `check.ts` are the test output on purpose.

## GitHub half (07-11-2026)

Published to `pantheon-org/pantheon-scaffold-spike`: one initial commit on `main` (README.md, pkg/PklProject, pkg/Config.pkl, authored with the
GitHub no-reply address), a normal release `v0.0.1` and a pre-release `v0.0.2-rc.1`, four assets each. Re-run: `bash real-github.sh` (6 checks, all pass).

| Question | Answer |
|---|---|
| Release download URL | `https://github.com/<org>/<repo>/releases/download/<tag>/<asset>` answers `302` to a short-lived signed URL on `release-assets.githubusercontent.com`, then `200`. pkl follows it. |
| Pre-release | Its assets are served at the same download URL (`302`), and pkl renders it by exact pin with the inline checksum. |
| Inline checksum on real GitHub | Works for both versions. A wrong checksum is rejected. |
| Restricted flags | `--allowed-modules pkl:,file:,package:` with `--allowed-resources package:,https:` still fetches from real GitHub, so the redirect target is not blocked. |
| Offline | A warm cache renders with the network blocked (`--http-proxy` to a dead port). A cold cache fails. |
| Propagation | Right after a release was created, pkl got `404` on the metadata URL for several minutes, while curl with an `Accept` header got `302`. The same URL rendered fine later. A request with no `Accept` header also got `404` in curl, so GitHub treats it differently while the release settles. Anything that fetches a new release immediately (the consumer upgrade workflow, a release pipeline smoke test) needs a retry. |

Not covered: build provenance attestations, mise installing the runner (S.2), and the hk interaction under a consumer `PklProject` fallback.
