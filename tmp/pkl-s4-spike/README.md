# pkl-s4-spike

Spike S.4 of the pkl plan: YAML null, cron quoting and the `on` key under pkl's `YamlRenderer`.
Run on 07-11-2026 with pkl 0.32.2 and bun 1.4.2. Local only, untracked. Re-run: `bun check.ts` in this folder.

| Question | Answer |
|---|---|
| Can `workflow_dispatch: null` be rendered? | Yes, but only with `omitNullProperties = false`. The class default is `true` in pkl 0.32.2, and under it a null property or a `Mapping` null entry is dropped. A null inside a `Listing` is kept. |
| What does `omitNullProperties = false` cost? | Every null field is emitted, not only the intended one (`name: null` on a job with an unset optional `name`). A typed model must therefore never hold null for an unset optional field, or the render needs a post-step. |
| Are cron strings starting with `*` quoted? | Yes: `'*/15 * * * *'` and `'* * * * *'` are quoted. `0 8 * * 1` is not quoted and parses back as a string. |
| How is the `on` key emitted? | Quoted as `'on':`, and it parses as the string `on` (not `true`). |

Parser: `Bun.YAML.parse`. I did not confirm it follows YAML 1.2 against its test suite, which the plan (P.1) still requires for the named parser.
Not answered here: whether GitHub Actions treats `workflow_dispatch: {}` the same as null. That needs the throwaway-repo run.
