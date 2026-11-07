# pkl-yaml-trial

Trial of `Bun.YAML` for the P.1 equality rule (decision 73). Run on 07-11-2026 with bun 1.4.2. Untracked.

- `trial.ts <suite-dir> [results.json]`: runs the official YAML test suite (`yaml/yaml-test-suite`, `data` branch, commit `6ad3d2c`, 2022-01-17, unpacked into the session scratchpad) against `Bun.YAML.parse` and scores every case. It reads the real workflows of `pantheon-adr` and `pantheon-skill-auditor` (read-only `gh api`) to see which YAML features are used, and scores the workflow-style subset separately.
- `differential.ts` (needs `YAML_PKG=<folder with node_modules/yaml>`): compares Bun.YAML with the `yaml` package 2.9.1 on those real workflows.

| Result | Value |
|---|---|
| Suite cases (including sub-cases) | 735 |
| Valid with expected JSON | 510 pass, 0 wrong result, 0 unexpected error |
| Invalid inputs | 172 rejected, 0 accepted |
| Valid without expected JSON | 53 parsed, 0 threw (values not verified) |
| Workflow-style subset (valid, one document, no tags, no complex keys) | 369 cases: 340 pass, 29 parsed without expected JSON, 0 wrong, 0 error |
| Real workflow features (of 30 files) | flow sequence or map 28, double-quoted 21, block scalar 14, single-quoted 10, anchors 0, aliases 0, merge keys 0, tags 0, extra documents 0, complex keys 0 |
| Differential on the 30 real workflows | Bun.YAML and the `yaml` package parse to identical data in 30 of 30 |
| Negative controls | a wrong expected value, valid YAML marked invalid, and invalid YAML given an expected value are each reported (the harness can fail) |

Verdict against the proposed pass bar (100% on the workflow subset, no invalid workflow-style input silently accepted): met.

Limits and findings:
- Bun.YAML accepts duplicate mapping keys silently (`a: 1` then `a: 2` gives `a: 2`), while the `yaml` package rejects them. The suite has no case that marks duplicates as an error, so the trial does not catch it. The equality test needs its own duplicate-key check.
- 29 workflow-style cases have no expected JSON in the suite, so their values were not compared, only that they parse.
- The `yaml` package installed through the package gate with one warning: version 2.9.1 was published without attestations, so its provenance cannot be verified. That is a cost of the fallback.
- Results are for bun 1.4.2. Re-run when the pinned bun version changes.
The `console.log` lines are the tools' output on purpose.
