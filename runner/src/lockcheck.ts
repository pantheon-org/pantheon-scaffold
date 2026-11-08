import { RUNNER_TOOL } from "./pins";

/** The platforms the runner is built for, and the release asset each one must resolve to. */
export const SUPPORTED_PLATFORMS = {
  "linux-x64": "pantheon-scaffold-linux-x64",
  "linux-arm64": "pantheon-scaffold-linux-arm64",
  "macos-arm64": "pantheon-scaffold-darwin-arm64",
} as const;

// One [tools."github:..."."platforms.<name>"] section: its header, then lines up to the next section.
const SECTION = new RegExp(`^\\[tools\\."${RUNNER_TOOL}"\\."platforms\\.([^"]+)"\\][ \\t]*\\n((?:(?!\\[)[^\\n]*(?:\\n|$))*)`, "gm");

/**
 * Checks what `mise lock` recorded for the runner. With no platform list mise locks every platform it knows, and for one the
 * release has no binary for it picks whatever asset it can, which for this release is the package zip. A lock that names a platform
 * we do not build for, or the wrong asset for one we do, is a problem. Returns the problems, empty when the lock is right or has
 * no entry for the runner.
 */
export function checkRunnerLock(lockText: string): string[] {
  const problems: string[] = [];
  for (const m of lockText.matchAll(SECTION)) {
    const platform = m[1];
    const url = /^url\s*=\s*"([^"]*)"/m.exec(m[2])?.[1];
    const asset = url === undefined ? undefined : decodeURIComponent(url.split("/").pop() ?? "");
    const expected = (SUPPORTED_PLATFORMS as Record<string, string>)[platform];
    if (asset === undefined) problems.push(`${platform}: no url recorded for the runner`);
    else if (expected === undefined) problems.push(`${platform}: not a supported platform, mise chose ${asset}`);
    else if (asset !== expected) problems.push(`${platform}: mise chose ${asset}, expected ${expected}`);
  }
  return problems;
}
