let t = await Bun.file("runner/test/upstream/mise.toml").text();
const rep = (a: string, b: string) => { if (!t.includes(a)) throw new Error("missing: " + a.slice(0, 70)); t = t.replace(a, () => b); };
rep(`rust = { version = "1.96.1", components = "rustfmt,clippy" }`, `rust = { version = "@@TOOLCHAIN@@", components = "rustfmt,clippy" }`);
rep(`actionlint = "1.7.12"`, `actionlint = "@@ACTIONLINT@@"`);
rep(`# Workflow security linter. Keep in sync with \`version\` in
# .github/workflows/zizmor.yml.
zizmor = "1.32.0"`, `# Workflow security linter. Pinned in the scaffold package (Tools.pkl).
zizmor = "@@ZIZMOR@@"`);
rep(`rumdl = "0.2.77"`, `rumdl = "@@RUMDL@@"`);
rep(`# Lints skill frontmatter and agent files. Keep in sync with AGNIX_VERSION in
# .github/workflows/agnix.yml.
"github:agent-sh/agnix" = "0.56.6"`, `# Lints skill frontmatter and agent files. Pinned in the scaffold package (Tools.pkl).
"github:agent-sh/agnix" = "@@AGNIX@@"
# The scaffold runner, released together with the package that rendered this file.
"github:pantheon-org/pantheon-scaffold" = "@@RUNNER@@"`);
rep(`minimum_release_age = "7d"`, `minimum_release_age = "@@MIN_RELEASE_AGE@@"
# Record every tool in mise.lock with its checksum, and refuse a tool whose provenance
# attestation does not verify. Limited to the platforms the runner is built for: with no
# list mise locks every platform and names the package zip for the ones with no runner.
lockfile = true
locked_verify_provenance = true
lockfile_platforms = [@@LOCKFILE_PLATFORMS@@]`);
await Bun.write("pkl/assets/mise.toml.tmpl", t);
