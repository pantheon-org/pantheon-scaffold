# Sample check of the v0.1.5 differences

Phase 7, wave E (task 7.2, decision 56). Generated on 09-11-2026 by `runner/scripts/proof-dossier.ts`. The package renders the standard set into each of the four repositories and 94 files differ from the repository's own copy (the others are identical). This is **19 of 94** of them (20 percent rounded up is 19, and at least one of every category), chosen with a seeded generator so the choice can be repeated: the seed is `pantheon-scaffold-0.1.5-wave-e`.

The owner reads each diff below and ticks it. A sample the owner does not finish is recorded as not signed. The owner is also the maintainer, so this check is **not independent** (decisions 69 and 72).

| Category | Differences | In the sample |
|---|---|---|
| comments and formatting only (P.1) | 52 | 5 |
| migration: hk.pkl replaced by the seeded one (decision 31) | 4 | 3 |
| migration: mise.toml, 10 lines added and 4 removed | 4 | 1 |
| mode change only (decision 79) | 4 | 2 |
| new file | 15 | 2 |
| pin moved | 4 | 1 |
| recorded difference: decision 85 | 3 | 2 |
| recorded difference: decision 86 | 3 | 1 |
| recorded difference: decisions 74 and 87 | 4 | 1 |
| recorded difference: seeded file, the repository's own scope wording | 1 | 1 |

### 1. pantheon-adr: .github/scripts/sync-labels.sh

- Category: mode change only (decision 79). 0644 to 0755
- Owner reviewed: [ ]

```diff
(content identical, mode 0644 to 0755)
```

### 2. pantheon-adr: .github/workflows/inline-scripts.yml

- Category: comments and formatting only (P.1)
- Owner reviewed: [ ]

```diff
@@ -1,3 +1 @@
-name: Inline Scripts
-
 # Rule: a `run:` block in a workflow or composite action may hold at most 15 lines
@@ -11,12 +9,16 @@ name: Inline Scripts
 # main as a post-merge safety net. Deterministic and offline.
-on:
+name: Inline Scripts
+'on':
   pull_request:
-    types: [opened, synchronize, reopened]
-    branches: [main]
+    types:
+    - opened
+    - synchronize
+    - reopened
+    branches:
+    - main
   push:
-    branches: [main]
-
+    branches:
+    - main
 permissions:
   contents: read
-
 concurrency:
@@ -24,3 +26,2 @@ concurrency:
   cancel-in-progress: true
-
 jobs:
@@ -31,7 +32,6 @@ jobs:
     steps:
-      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
-        with:
-          persist-credentials: false
-
-      - name: Check run blocks are short
-        run: bash .github/scripts/check-inline-scripts.sh
+    - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
... (cut at 40 lines)
```

### 3. pantheon-adr: .github/workflows/sync-labels.yml

- Category: comments and formatting only (P.1)
- Owner reviewed: [ ]

```diff
@@ -1,3 +1 @@
-name: Sync Labels
-
 # Creates the labels Dependabot applies (see .github/dependabot.yml). Dependabot
@@ -5,11 +3,12 @@ name: Sync Labels
 # Edit .github/labels.txt to add a label; this runs on push to main or on demand.
-on:
+name: Sync Labels
+'on':
   push:
-    branches: [main]
+    branches:
+    - main
     paths:
-      - .github/labels.txt
-      - .github/scripts/sync-labels.sh
-      - .github/workflows/sync-labels.yml
-  workflow_dispatch:
-
+    - .github/labels.txt
+    - .github/scripts/sync-labels.sh
+    - .github/workflows/sync-labels.yml
+  workflow_dispatch: {}
 permissions:
@@ -17,3 +16,2 @@ permissions:
   issues: write
-
 jobs:
@@ -24,10 +22,9 @@ jobs:
     steps:
-      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
-        with:
-          persist-credentials: false
-
-      - name: Create or update labels
-        env:
-          GH_TOKEN: ${{ github.token }}
-          GH_REPO: ${{ github.repository }}
-        run: bash .github/scripts/sync-labels.sh
+    - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
... (cut at 40 lines)
```

### 4. pantheon-adr: hk.pkl

- Category: migration: hk.pkl replaced by the seeded one (decision 31)
- Owner reviewed: [ ]

```diff
@@ -2,3 +2,10 @@ amends "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Config.
 import "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Builtins.pkl"
+import "hk.shared.pkl" as shared
 
+// This file is seeded once by pantheon-scaffold and is yours to edit. The steps every repository shares are in
+// hk.shared.pkl, which the runner renders and owns: do not edit that file. To add a step of your own, edit a hook below (a module can
+// define `hooks` only once), for example in the check hook
+//
+//   steps { ...shared.preCommit(); ["my-step"] { glob = List("*.txt"); check = "my-check {{files}}" } }
+//
 // Tools are pinned in mise.toml; HK_MISE=1 runs each hook under `mise x`.
@@ -7,48 +14,2 @@ import "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Builtin
 
-// -- pre-commit (fast, staged files only) --
-local preCommit = new Mapping<String, Step> {
-    ["rustfmt"] {
-        glob = List("*.rs")
-        check = "cargo fmt --all --check"
-        fix = "cargo fmt --all"
-    }
-    ["rumdl"] {
-        glob = List("*.md")
-        check = "rumdl check --config .rumdl.toml {{files}}"
-        fix = "rumdl check --fix --config .rumdl.toml {{files}}"
-    }
-    ["yamllint"] {
-        glob = List("*.yml", "*.yaml")
-        shell = "sh -c"
-        check = "command -v yq >/dev/null 2>&1 && yq eval '.' {{files}} >/dev/null"
-    }
-    ["shellcheck"] {
-        glob = List("*.sh")
-        shell = "sh -c"
-        check = "command -v shellcheck >/dev/null 2>&1 && shellcheck {{files}}"
-    }
-    // Catches workflow mistakes yamllint cannot: unknown keys, bad expressions,
-    // `needs:` references, and shell problems inside `run:` blocks. Scoped to
-    // the workflow files so it only runs when one is staged. Reads
-    // .github/actionlint.yaml. It is a local gate: there is no actionlint
-    // workflow, so it is not a required status check.
... (cut at 40 lines)
```

### 5. pantheon-agentic-harness-dev-kit: .github/workflows/plumber.yml

- Category: pin moved. getplumber/plumber@ec8cb3a
- Owner reviewed: [ ]

```diff
@@ -1,3 +1 @@
-name: Plumber
-
 # CI/CD security scan of this repository's GitHub Actions workflows (unpinned
@@ -11,16 +9,20 @@ name: Plumber
 # finding in the gate step below. Config lives in .plumber.yaml.
-
-on:
-  push:
-    branches: [main]
+#
+# Generated by pantheon-scaffold from pantheon.pkl: change it there, not in this file.
+name: Plumber
+'on':
   pull_request:
-    types: [opened, synchronize, reopened]
-    branches: [main]
+    types:
+    - opened
+    - synchronize
+    - reopened
+    branches:
+    - main
+  push:
+    branches:
+    - main
   schedule:
-    # Weekly scan (Mondays 03:41 UTC) to catch newly published action advisories.
-    - cron: "41 3 * * 1"
-
+  - cron: 41 3 * * 1
 permissions:
   contents: read
-
 concurrency:
@@ -28,3 +30,2 @@ concurrency:
   cancel-in-progress: true
-
 jobs:
@@ -33,35 +34,29 @@ jobs:
... (cut at 40 lines)
```

### 6. pantheon-agentic-harness-dev-kit: .github/workflows/release-automerge.yml

- Category: comments and formatting only (P.1)
- Owner reviewed: [ ]

```diff
@@ -1,3 +1 @@
-name: Release Auto-merge
-
 # Mondays: squash-merges the open release-please PR so a release lands without a
@@ -6,12 +4,9 @@ name: Release Auto-merge
 # Ported from pantheon-org/tekhne. Needs the org secret GH_APP_PRIVATE_KEY.
-
-on:
+name: Release Auto-merge
+'on':
   schedule:
-    # Mondays 08:37 UTC. Cron is UTC; there is no DST handling.
-    - cron: "37 8 * * 1"
+  - cron: 37 8 * * 1
   workflow_dispatch: {}
-
 permissions:
   contents: read
-
 concurrency:
@@ -19,3 +14,2 @@ concurrency:
   cancel-in-progress: false
-
 jobs:
@@ -26,21 +20,19 @@ jobs:
     steps:
-      - name: Mint pantheon-ai-bot token
-        id: app-token
-        uses: actions/create-github-app-token@64ab729a6294000f5aa4a4bc0b73d1f32bc45510 # v3.3.0
-        with:
-          client-id: "Iv23ligzn16vkDCWfHgc"
-          private-key: ${{ secrets.GH_APP_PRIVATE_KEY }}
-          permission-contents: write
-          permission-pull-requests: write
-
-      - name: Checkout scripts
-        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
-        with:
-          persist-credentials: false
-          sparse-checkout: .github/scripts
... (cut at 40 lines)
```

### 7. pantheon-agentic-harness-dev-kit: .gitignore

- Category: recorded difference: decisions 74 and 87. added target/, .pkl-gen/cache/, removed /target
- Owner reviewed: [ ]

```diff
@@ -1,2 +1,3 @@
-/target
+target/
 .context/
+.pkl-gen/cache/
```

### 8. pantheon-agentic-harness-dev-kit: hk.shared.pkl

- Category: new file. a file the package adds
- Owner reviewed: [ ]

```diff
(new file, 52 lines)
```

### 9. pantheon-agentic-harness-dev-kit: mise.toml

- Category: migration: mise.toml, 10 lines added and 4 removed
- Owner reviewed: [ ]

```diff
@@ -7,4 +7,3 @@ rust = { version = "1.96.1", components = "rustfmt,clippy" }
 actionlint = "1.7.12"
-# Workflow security linter. Keep in sync with `version` in
-# .github/workflows/zizmor.yml.
+# Workflow security linter. Pinned in the scaffold package (Tools.pkl).
 zizmor = "1.32.0"
@@ -18,5 +17,6 @@ shellcheck = "latest"
 rumdl = "0.2.77"
-# Lints skill frontmatter and agent files. Keep in sync with AGNIX_VERSION in
-# .github/workflows/agnix.yml.
+# Lints skill frontmatter and agent files. Pinned in the scaffold package (Tools.pkl).
 "github:agent-sh/agnix" = "0.56.6"
+# The scaffold runner, released together with the package that rendered this file.
+"github:pantheon-org/pantheon-scaffold" = "0.1.5"
 
@@ -25,2 +25,8 @@ rumdl = "0.2.77"
 minimum_release_age = "7d"
+# Record every tool in mise.lock with its checksum, and refuse a tool whose provenance
+# attestation does not verify. Limited to the platforms the runner is built for: with no
+# list mise locks every platform and names the package zip for the ones with no runner.
+lockfile = true
+locked_verify_provenance = true
+lockfile_platforms = ["linux-x64", "linux-arm64", "macos-arm64"]
```

### 10. pantheon-journal: .github/scripts/sync-labels.sh

- Category: mode change only (decision 79). 0644 to 0755
- Owner reviewed: [ ]

```diff
(content identical, mode 0644 to 0755)
```

### 11. pantheon-journal: .github/workflows/release-please.yml

- Category: comments and formatting only (P.1)
- Owner reviewed: [ ]

```diff
@@ -1,3 +1 @@
-name: Release Please
-
 # Keeps one rolling release PR up to date on every push to main (version bump in
@@ -12,12 +10,10 @@ name: Release Please
 # Needs the org secret GH_APP_PRIVATE_KEY.
-
-on:
+name: Release Please
+'on':
   push:
-    branches: [main]
+    branches:
+    - main
   workflow_dispatch: {}
-
 permissions:
   contents: read
-
-# Never cancel a run mid-way: it could leave a version bumped but untagged.
 concurrency:
@@ -25,3 +21,2 @@ concurrency:
   cancel-in-progress: false
-
 jobs:
@@ -32,26 +27,23 @@ jobs:
     steps:
-      - name: Mint pantheon-ai-bot token
-        id: app-token
-        uses: actions/create-github-app-token@64ab729a6294000f5aa4a4bc0b73d1f32bc45510 # v3.3.0
-        with:
-          client-id: "Iv23ligzn16vkDCWfHgc"
-          private-key: ${{ secrets.GH_APP_PRIVATE_KEY }}
-          permission-contents: write
-          permission-pull-requests: write
-
-      - uses: googleapis/release-please-action@45996ed1f6d02564a971a2fa1b5860e934307cf7 # v5.0.0
-        with:
-          token: ${{ steps.app-token.outputs.token }}
-          config-file: release-please-config.json
... (cut at 40 lines)
```

### 12. pantheon-journal: .github/workflows/scaffold-check.yml

- Category: new file. a file the package adds
- Owner reviewed: [ ]

```diff
(new file, 36 lines)
```

### 13. pantheon-journal: dist-workspace.toml

- Category: recorded difference: decision 85. 2 action commit line(s)
- Owner reviewed: [ ]

```diff
@@ -32,3 +32,3 @@ x86_64-apple-darwin = "macos-latest"
 "actions/checkout" = "3d3c42e5aac5ba805825da76410c181273ba90b1"       # v7
-"actions/upload-artifact" = "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a" # v7
-"actions/download-artifact" = "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c" # v8
+"actions/upload-artifact" = "cf430e030ddbb5b0abf93d22962f4752f3646cd9" # v7
+"actions/download-artifact" = "9000827ccba6bdab643e8b6fd33ac0654aef8333" # v8
```

### 14. pantheon-journal: hk.pkl

- Category: migration: hk.pkl replaced by the seeded one (decision 31)
- Owner reviewed: [ ]

```diff
@@ -2,3 +2,10 @@ amends "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Config.
 import "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Builtins.pkl"
+import "hk.shared.pkl" as shared
 
+// This file is seeded once by pantheon-scaffold and is yours to edit. The steps every repository shares are in
+// hk.shared.pkl, which the runner renders and owns: do not edit that file. To add a step of your own, edit a hook below (a module can
+// define `hooks` only once), for example in the check hook
+//
+//   steps { ...shared.preCommit(); ["my-step"] { glob = List("*.txt"); check = "my-check {{files}}" } }
+//
 // Tools are pinned in mise.toml; HK_MISE=1 runs each hook under `mise x`.
@@ -7,48 +14,2 @@ import "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Builtin
 
-// -- pre-commit (fast, staged files only) --
-local preCommit = new Mapping<String, Step> {
-    ["rustfmt"] {
-        glob = List("*.rs")
-        check = "cargo fmt --all --check"
-        fix = "cargo fmt --all"
-    }
-    ["rumdl"] {
-        glob = List("*.md")
-        check = "rumdl check --config .rumdl.toml {{files}}"
-        fix = "rumdl check --fix --config .rumdl.toml {{files}}"
-    }
-    ["yamllint"] {
-        glob = List("*.yml", "*.yaml")
-        shell = "sh -c"
-        check = "command -v yq >/dev/null 2>&1 && yq eval '.' {{files}} >/dev/null"
-    }
-    ["shellcheck"] {
-        glob = List("*.sh")
-        shell = "sh -c"
-        check = "command -v shellcheck >/dev/null 2>&1 && shellcheck {{files}}"
-    }
-    // Catches workflow mistakes yamllint cannot: unknown keys, bad expressions,
-    // `needs:` references, and shell problems inside `run:` blocks. Scoped to
-    // the workflow files so it only runs when one is staged. Reads
-    // .github/actionlint.yaml. It is a local gate: there is no actionlint
-    // workflow, so it is not a required status check.
... (cut at 40 lines)
```

### 15. pantheon-skill-auditor: .github/dependabot.yml

- Category: recorded difference: decision 86. $.updates[1].ignore: undefined vs [{"dependency-name":"actions/upload-artifact"},{"dependency-name":"actions/download-artifact"}]
- Owner reviewed: [ ]

```diff
@@ -1,3 +1 @@
-version: 2
-
 # Keeps Cargo dependencies and the SHA-pinned GitHub Actions current. Minor and
@@ -11,53 +9,57 @@ version: 2
 # hand when pantheon-agentic-harness-dev-kit cuts a release.
-
+#
+# release.yml caveat: release.yml is written by `dist generate` from dist-workspace.toml, which
+# Dependabot cannot edit, so a Dependabot bump of upload-artifact or download-artifact inside
+# it would fail the dist check. They are ignored below: change their commits in
+# dist-workspace.toml and run `dist generate`.
+version: 2
 updates:
-  - package-ecosystem: "cargo"
-    directory: "/"
-    schedule:
-      interval: "weekly"
-      day: "monday"
-    open-pull-requests-limit: 5
-    # Wait 7 days after a release before opening an update PR, so a yanked or
-    # compromised release has time to surface before we adopt it.
-    cooldown:
-      default-days: 7
-    ignore:
-      - dependency-name: "pahdk-core"
-      - dependency-name: "pahdk-install"
-    labels:
-      - "dependencies"
-      - "rust"
-    commit-message:
-      prefix: "chore"
-      prefix-development: "chore"
-      include: "scope"
-    groups:
-      cargo-minor-patch:
-        patterns:
-          - "*"
-        update-types:
-          - "minor"
... (cut at 40 lines)
```

### 16. pantheon-skill-auditor: .github/workflows/zizmor.yml

- Category: comments and formatting only (P.1)
- Owner reviewed: [ ]

```diff
@@ -1,3 +1 @@
-name: zizmor
-
 # Static analysis for GitHub Actions security problems: credential persistence,
@@ -21,24 +19,21 @@ name: zizmor
 #     --config .github/zizmor.yml .github action.yml
-on:
+name: zizmor
+'on':
   pull_request:
-    types: [opened, synchronize, reopened]
-    branches: [main]
+    types:
+    - opened
+    - synchronize
+    - reopened
+    branches:
+    - main
   push:
-    branches: [main]
-
+    branches:
+    - main
 permissions:
   contents: read
-
 concurrency:
-  # Supersede an in-flight run when the ref updates (e.g. a rebase), so
-  # stale runs are cancelled instead of piling onto the shared API quota.
   group: ${{ github.workflow }}-${{ github.ref }}
   cancel-in-progress: true
-
 jobs:
   zizmor:
-    # The job name is the required status check context. Keep it stable: the
-    # ruleset matches on it, so renaming it leaves the old name required and
-    # every pull request waits for a check that no longer reports until the
-    # ruleset is updated.
     name: zizmor
@@ -47,18 +42,11 @@ jobs:
... (cut at 40 lines)
```

### 17. pantheon-skill-auditor: dist-workspace.toml

- Category: recorded difference: decision 85. 2 action commit line(s)
- Owner reviewed: [ ]

```diff
@@ -32,3 +32,3 @@ x86_64-apple-darwin = "macos-latest"
 "actions/checkout" = "3d3c42e5aac5ba805825da76410c181273ba90b1"       # v7
-"actions/upload-artifact" = "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a" # v7
-"actions/download-artifact" = "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c" # v8
+"actions/upload-artifact" = "cf430e030ddbb5b0abf93d22962f4752f3646cd9" # v7
+"actions/download-artifact" = "9000827ccba6bdab643e8b6fd33ac0654aef8333" # v8
```

### 18. pantheon-skill-auditor: hk.pkl

- Category: migration: hk.pkl replaced by the seeded one (decision 31)
- Owner reviewed: [ ]

```diff
@@ -2,3 +2,10 @@ amends "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Config.
 import "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Builtins.pkl"
+import "hk.shared.pkl" as shared
 
+// This file is seeded once by pantheon-scaffold and is yours to edit. The steps every repository shares are in
+// hk.shared.pkl, which the runner renders and owns: do not edit that file. To add a step of your own, edit a hook below (a module can
+// define `hooks` only once), for example in the check hook
+//
+//   steps { ...shared.preCommit(); ["my-step"] { glob = List("*.txt"); check = "my-check {{files}}" } }
+//
 // Tools are pinned in mise.toml; HK_MISE=1 runs each hook under `mise x`.
@@ -7,48 +14,2 @@ import "package://github.com/jdx/hk/releases/download/v1.49.0/hk@1.49.0#/Builtin
 
-// -- pre-commit (fast, staged files only) --
-local preCommit = new Mapping<String, Step> {
-    ["rustfmt"] {
-        glob = List("*.rs")
-        check = "cargo fmt --all --check"
-        fix = "cargo fmt --all"
-    }
-    ["rumdl"] {
-        glob = List("*.md")
-        check = "rumdl check --config .rumdl.toml {{files}}"
-        fix = "rumdl check --fix --config .rumdl.toml {{files}}"
-    }
-    ["yamllint"] {
-        glob = List("*.yml", "*.yaml")
-        shell = "sh -c"
-        check = "command -v yq >/dev/null 2>&1 && yq eval '.' {{files}} >/dev/null"
-    }
-    ["shellcheck"] {
-        glob = List("*.sh")
-        shell = "sh -c"
-        check = "command -v shellcheck >/dev/null 2>&1 && shellcheck {{files}}"
-    }
-    // Catches workflow mistakes yamllint cannot: unknown keys, bad expressions,
-    // `needs:` references, and shell problems inside `run:` blocks. Scoped to
-    // the workflow files so it only runs when one is staged. Reads
-    // .github/actionlint.yaml. It is a local gate: there is no actionlint
-    // workflow, so it is not a required status check.
... (cut at 40 lines)
```

### 19. pantheon-skill-auditor: SECURITY.md

- Category: recorded difference: seeded file, the repository's own scope wording. 3 scope line(s)
- Owner reviewed: [ ]

```diff

```
