# Installing the runner in a repository

The runner is installed through mise from the GitHub release, with the checksums in the lock file and the build provenance attestation enforced.

## `mise.toml`

```toml
[settings]
lockfile = true
locked_verify_provenance = true
lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]

[tools]
"github:pantheon-org/pantheon-scaffold" = "0.1.1"
```

Then `mise lock` and `mise install --locked`. A CI job runs `mise install --locked`, so a tampered checksum or a missing attestation fails the install.

## Supported platforms

The runner is built for `linux-x64`, `linux-arm64` and `macos-arm64` (glibc Linux). There is no binary for Intel Macs, Windows or musl Linux such as Alpine.

**Always set `lockfile_platforms` as above.** Without it mise locks every platform it knows. For a platform the release has no runner for it picks whatever asset it can, and for this release that is the package zip, so an Intel Mac or Windows machine would be handed a file that is not a runner. For musl Linux it picks the glibc binary. Found on the published v0.1.1: the lock for `macos-x64` and `windows-x64` named `pantheon-scaffold@0.1.1.zip`, and the `linux-*-musl` entries named the glibc builds.

## What guards this

- `pantheon-scaffold upgrade` checks the runner's lock entries after `mise lock`. A platform the runner is not built for, or the wrong asset for one it is, fails the upgrade and puts the pins and the lock file back, with the setting to add (see [runner-upgrade.md](runner-upgrade.md)).
- The release pipeline's `verify-release` job installs the published release as a consumer with exactly this setting, and checks that the lock names only the three runner binaries (see [release-process.md](release-process.md)).
- The `mise.toml` that the package renders for a repository must carry the setting. That template is part of the tooling layer (phase 3), which is not built yet.
