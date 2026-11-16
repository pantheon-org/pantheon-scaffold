# The TypeScript module tests, and where each one went

Written on 15-11-2026 for Phase 0 of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). A TypeScript test of a module that has a Rust counterpart is deleted (Phase 6) only when a named Rust test covers the same case. This page is that evidence. It covers the nine TypeScript test files whose module (or helper) already exists in `crates/scaffold-core`: `checksums`, `semver`, `freshness`, `knownbad`, `manifest`, `preflight`, `schema`, `contract` and `equality`: 114 tests.

Rust tests are in `crates/scaffold-core/src/<module>.rs` unless a path is given. Where a TypeScript test was not covered, Phase 0 added the Rust test (marked **added in Phase 0**). Where the Rust side is deliberately different, the row says so and the section "Deliberate differences" gives the reason.

## Result

| Module | TypeScript tests | Covered by an existing Rust test | Added in Phase 0 | Not applicable |
|---|---|---|---|---|
| `checksums` | 8 | 8 | 0 | 0 |
| `semver` | 5 | 5 | 0 (2 more pin deliberate differences) | 0 |
| `freshness` | 9 | 9 | 0 | 0 |
| `knownbad` | 12 | 12 | 0 | 0 |
| `manifest` | 11 | 10 | 0 | 1 |
| `preflight` | 22 | 22 | 0 | 0 |
| `schema` | 10 | 10 | 0 | 0 |
| `contract` | 16 | 12 | 3 | 1 |
| `equality` | 21 | 21 | 0 | 0 |
| **Total** | **114** | **109** | **3** | **2** |

The two "not applicable" tests (one in `manifest`, one in `contract`) only check that an error type is exported. That is a property of the TypeScript module system, and Rust has the types (`ManifestError`, `ContractError`).

## checksums

| TypeScript test (`runner/test/checksums.test.ts`) | Rust test | Note |
|---|---|---|
| parses sha256sum lines, with or without the binary marker, and ignores blank lines | `parses_sha256sum_lines_with_or_without_the_binary_marker_and_ignores_blank_lines` | covered |
| a line that is not a sha256sum line is an error, never skipped | `a_line_that_is_not_a_sha256sum_line_is_an_error_never_skipped` | covered |
| an empty file has no checksums | `an_empty_file_has_no_checksums` | covered |
| finds the checksum of one named file | `finds_the_checksum_of_one_named_file` | covered |
| a missing name and a repeated name are both errors | `a_missing_name_and_a_repeated_name_are_both_errors` | covered |
| the asset lives next to the release it describes | `the_asset_lives_next_to_the_release_it_describes` | covered |
| formatChecksums writes sha256sum lines that parseChecksums reads back, sorted by name | `format_checksums_writes_sha256sum_lines_that_parse_back_sorted_by_name` | covered |
| formatChecksums refuses a checksum or a name that parseChecksums could not read back | `format_checksums_refuses_a_checksum_or_a_name_that_could_not_be_read_back` | covered |

## semver

| TypeScript test (`runner/test/semver.test.ts`) | Rust test | Note |
|---|---|---|
| parses a plain version, a leading v and a pre-release | `parses_a_plain_version_a_leading_v_and_a_pre_release` | covered |
| rejects text that is not a version | `rejects_text_that_is_not_a_version` | covered |
| compares numerically, not as text | `compares_numerically_not_as_text` | covered |
| a pre-release is lower than its release and pre-releases follow the semver order | `a_pre_release_is_lower_than_its_release_and_pre_releases_follow_the_semver_order` | covered |
| comparing an unparsable version throws, never guesses | `comparing_an_unparsable_version_is_an_error_naming_it` | covered. Rust returns an error, TypeScript throws |

## freshness

| TypeScript test (`runner/test/freshness.test.ts`) | Rust test | Note |
|---|---|---|
| the record lives on the freshness branch of the package repository | `the_record_lives_on_the_freshness_branch_of_the_package_repository` | covered |
| a valid record parses to its UTC time | `a_valid_record_parses_to_its_utc_time` | covered |
| anything that is not a valid record is refused | `anything_that_is_not_a_valid_record_is_refused` | covered |
| a record up to 30 days old is fresh and older than that is stale | `a_record_up_to_30_days_old_is_fresh_and_older_than_that_is_stale` | covered |
| the age is reported in whole days and the limit can be changed | `the_age_is_reported_in_whole_days_and_the_limit_can_be_changed` | covered |
| a record dated a little in the future is fresh, and one far in the future is refused as a clock or data error | `a_record_dated_a_little_in_the_future_is_fresh_with_age_zero`, `a_record_far_in_the_future_is_refused_as_a_clock_or_data_error` | covered (split in two) |
| a fetch that fails is retried and the third attempt succeeds | `a_fetch_that_fails_is_retried_and_the_third_attempt_succeeds` | covered |
| a fetch that never works fails closed after every attempt, it never reports fresh | `a_fetch_that_never_works_fails_closed_after_every_attempt` | covered |
| a record that is fetched but invalid is not retried | `a_record_that_is_fetched_but_invalid_is_not_retried` | covered |

## knownbad

| TypeScript test (`runner/test/knownbad.test.ts`) | Rust test | Note |
|---|---|---|
| a valid list parses, anything else does not | `a_valid_list_parses_anything_else_does_not` | covered |
| a fetched list is used and written to the cache | `a_fetched_list_is_used_and_written_to_the_cache` | covered |
| when the fetch fails the cached list is used, with a note | `when_the_fetch_fails_the_cached_list_is_used_with_a_note` | covered |
| with no fetch and no cache the bundled snapshot is used, with a note | `with_no_fetch_and_no_cache_the_bundled_snapshot_is_used_with_a_note` | covered |
| a fetched list that does not validate is ignored and never cached | `a_fetched_list_that_does_not_validate_is_ignored_and_never_cached` | covered |
| a corrupt cache is ignored | `a_corrupt_cache_is_ignored` | covered |
| a fetch that succeeds after an earlier cache replaces the cache | `a_fetch_that_succeeds_after_an_earlier_cache_replaces_the_cache` | covered |
| the default fetcher reads the URL and fails on a bad status | `http::tests::a_200_returns_the_body_as_text`, `http::tests::every_status_outside_200_to_299_is_an_error_naming_it`; the fall-back to the bundled list is `with_no_fetch_and_no_cache_the_bundled_snapshot_is_used_with_a_note` | covered (split between the HTTP layer and the loader) |
| the default fetch gives up after the timeout | `http::tests::a_server_that_never_answers_is_stopped_at_the_timeout`; the fall-back is `when_the_fetch_fails_the_cached_list_is_used_with_a_note` | covered (split between the HTTP layer and the loader) |
| the default URL is the list on the package repo's main branch | `the_default_url_is_the_list_on_the_package_repos_main_branch` | covered |
| known-bad.json on main is a valid list and names v0.1.0, which cannot read a checksum-pinned package URI | `the_built_in_snapshot_is_a_valid_list_and_names_the_version_that_cannot_read_a_checksum` | covered. Rust reads the repository's own `known-bad.json` (`include_str!`), so there is no second file to drift |
| the bundled snapshot is a valid list, and everything in it is also in known-bad.json | `the_built_in_snapshot_is_a_valid_list_and_names_the_version_that_cannot_read_a_checksum` | deliberate difference: the bundled snapshot is the root file, so the two cannot differ; the test still compares them |

## manifest

| TypeScript test (`runner/test/manifest.test.ts`) | Rust test | Note |
|---|---|---|
| serialisation is sorted, indented and ends with one newline | `serialisation_is_sorted_indented_and_ends_with_one_newline` | covered |
| serialising twice gives the same text | `serialising_twice_gives_the_same_text` | covered |
| a missing manifest reads as empty | `a_missing_manifest_reads_as_empty` | covered |
| a manifest round-trips from disk | `a_manifest_round_trips_from_disk` | covered |
| an unknown manifest version is refused | `an_unknown_manifest_version_is_refused` | covered |
| a manifest with merge conflict markers points at --repair | `a_manifest_with_merge_conflict_markers_points_at_repair` | covered |
| sha256 matches a known value | `sha256_matches_a_known_value` | covered |
| the ManifestError type is exported | none | not applicable: a type, not a behaviour. Rust has `ManifestError` |
| the package version and the detach flag are kept, and the package version is written right after the manifest version | `the_package_version_and_the_detach_flag_are_kept_and_the_version_comes_right_after_the_manifest_version` | covered |
| a manifest without a package version serialises exactly as before | `an_empty_manifest_serialises_with_an_empty_files_map` | covered |
| a package version that is not a string is refused | `a_package_version_that_is_not_a_string_is_refused` | covered |

## preflight

| TypeScript test (`runner/test/preflight.test.ts`) | Rust test | Note |
|---|---|---|
| the version is read from the package URI, pre-releases included | `the_version_is_read_from_the_package_uri_pre_releases_included` | covered |
| a local package (file:) or no URI has no version | `a_local_package_or_no_uri_has_no_version` | covered |
| a package URI with no readable version is an error, never ignored | `a_package_uri_with_no_readable_version_is_an_error_never_ignored` | covered |
| a render with no package version and nothing to report is clean | `a_render_with_no_package_version_and_nothing_to_report_is_clean` | covered |
| the version is returned for the manifest | `the_version_is_returned_for_the_manifest` | covered |
| a runner older than the package requires is an error | `a_runner_older_than_the_package_requires_is_an_error` | covered |
| a runner that meets the requirement, or exceeds it, passes | `a_runner_that_meets_the_requirement_or_exceeds_it_passes` | covered |
| a requirement that is not a version is an error | `a_requirement_that_is_not_a_version_is_an_error` | covered |
| a requirement is checked for a local package too | `a_requirement_is_checked_for_a_local_package_too` | covered |
| an older package than the last render is refused, and --force allows it with a warning | `an_older_package_than_the_last_render_is_refused_and_force_allows_it_with_a_warning` | covered |
| a pre-release of the recorded version counts as older | `a_pre_release_of_the_recorded_version_counts_as_older` | covered |
| the same version and a newer version are fine | `the_same_version_and_a_newer_version_are_fine` | covered |
| a check only warns about an older package, it writes nothing | `a_check_only_warns_about_an_older_package_it_writes_nothing` | covered |
| a manifest version that is not a version is an error | `a_manifest_version_that_is_not_a_version_is_an_error` | covered |
| a known-bad package version warns with the reason | `a_known_bad_package_version_warns_with_the_reason` | covered |
| --strict turns a known-bad package version into a refusal | `strict_turns_a_known_bad_package_version_into_a_refusal` | covered |
| a version that is not on the list passes, even with --strict | `a_version_that_is_not_on_the_list_passes_even_with_strict` | covered |
| a disabled Security builtin is a warning for each one, and an enabled one is silent | `a_disabled_security_builtin_is_a_warning_for_each_one_and_an_enabled_one_is_silent` | covered |
| a pre-release runner of the required version is older than the requirement, so a requirement must be a version pre-releases of the next release can meet | `a_pre_release_runner_of_the_required_version_is_older_than_the_requirement` | covered |
| the version is read from a package URI that carries a checksum, pre-releases included | `the_version_is_read_from_a_package_uri_that_carries_a_checksum` | covered |
| a checksum that is not 64 hex characters is not accepted as part of the URI | `a_checksum_that_is_not_64_hex_characters_is_not_accepted_as_part_of_the_uri` | covered |
| a render with a checksum-pinned package URI gets its version, the way every real consumer does | `a_render_with_a_checksum_pinned_package_uri_gets_its_version` | covered |

## schema

| TypeScript test (`runner/test/schema.test.ts`) | Rust test | Note |
|---|---|---|
| accepts a valid value | `accepts_a_valid_value` | covered |
| reports a missing required key | `reports_a_missing_required_key` | covered |
| reports an unexpected key | `reports_an_unexpected_key` | covered |
| reports a wrong type with its path | `reports_a_wrong_type_with_its_path` | covered |
| a float is not an integer | `a_float_is_not_an_integer_but_a_whole_float_is` | covered |
| reports a minimum | `reports_a_minimum` | covered |
| reports an enum miss | `reports_an_enum_miss` | covered |
| reports a pattern miss | `reports_a_pattern_miss_and_a_short_string_in_that_order` | covered |
| reports an item error with an index | `reports_an_item_error_with_an_index` | covered |
| reports a const miss | `reports_a_const_miss` | covered |

## contract

| TypeScript test (`runner/test/contract.test.ts`) | Rust test | Note |
|---|---|---|
| the pkl output of the minimal fixture satisfies the schema (contract test, both sides) | `pkl::tests::the_minimal_fixture_evaluates_to_a_contract`; `features/cli.feature` scenario "contract evaluates a config and reports what it holds" (parses it with `parse_contract`, prints "contract ok: 4 files") | covered |
| the accepted example is valid | `a_valid_contract_parses_and_keeps_every_file` | covered |
| rejects an unknown schema version | `an_unknown_schema_version_is_refused_before_anything_else` | deliberate difference: Rust says "contract schemaVersion 3 is newer than this runner supports (2): the package needs a newer runner" for a newer version, and keeps the old wording for older ones (the test pins both) |
| rejects a missing schema version | `an_unknown_schema_version_is_refused_before_anything_else` | covered (same wording as TypeScript) |
| rejects an external file with content | `an_external_file_must_not_carry_content` | covered |
| rejects an owned file without content | `an_owned_or_seed_file_needs_content` | covered |
| rejects a duplicate path | `a_path_listed_twice_is_refused` | covered |
| rejects a bad mode | `a_contract_that_does_not_match_the_schema_is_refused_with_every_problem_listed` | covered (one test pins mode, class, pin sha and an extra key together) |
| rejects a bad pin sha | `a_contract_that_does_not_match_the_schema_is_refused_with_every_problem_listed` | covered |
| rejects an unexpected top-level key | `a_contract_that_does_not_match_the_schema_is_refused_with_every_problem_listed` | covered |
| the ContractError type is exported | none | not applicable: a type, not a behaviour. Rust has `ContractError` |
| a scaffold.pkl cannot read arbitrary files (restricted resources) | `pkl::tests::a_scaffold_pkl_cannot_read_arbitrary_files` | **added in Phase 0** (it was a gap) |
| allowing files under the package folder does not open the rest of the disk | `pkl::tests::allowing_files_under_the_package_folder_does_not_open_the_rest_of_the_disk` | **added in Phase 0** (it was a gap). `pkl::tests` also pins the allowed-resources text |
| the optional lifecycle fields are accepted | `the_optional_fields_are_read` | covered |
| a lifecycle field with the wrong shape is rejected | `a_lifecycle_field_with_the_wrong_shape_is_refused` | **added in Phase 0** (it was a gap): six cases, the same as the TypeScript test |
| removeOnDetach is only allowed on owned files | `remove_on_detach_is_only_allowed_on_owned_files` | covered |

## equality

| TypeScript test (`runner/test/equality.test.ts`) | Rust test | Note |
|---|---|---|
| identical text is equal | `identical_text_is_equal` | covered |
| comments, key order, quoting and block lists do not matter | `comments_key_order_quoting_and_block_lists_do_not_matter` | covered |
| null and {} are equal inside on: | `null_and_an_empty_map_are_equal_inside_on` | covered |
| null and {} are NOT treated as equal outside on: | `null_and_an_empty_map_are_not_equal_outside_on` | covered |
| a one character difference in a run script is a difference | `a_one_character_difference_in_a_run_script_is_a_difference` | covered |
| a trailing newline difference in a run script is a difference (byte for byte) | `a_trailing_newline_difference_in_a_run_script_is_a_difference` | covered |
| an extra key is a difference | `an_extra_key_is_a_difference_shown_as_undefined` | covered |
| a duplicate key in the rendered file fails the comparison | `a_duplicate_key_in_the_rendered_file_fails_the_comparison` | covered |
| duplicate keys: same key in different list items is fine | `duplicate_keys_same_key_in_different_list_items_is_fine` | covered |
| duplicate keys: same key in different mappings is fine | `duplicate_keys_same_key_in_different_mappings_is_fine` | covered |
| duplicate keys: a repeated key in one nested mapping is found | `duplicate_keys_a_repeated_key_in_one_nested_mapping_is_found` | covered |
| duplicate keys: block scalar content is ignored | `duplicate_keys_block_scalar_content_is_ignored` | covered |
| duplicate keys: comments and the document marker are ignored | `duplicate_keys_comments_and_the_document_marker_are_ignored` | covered |
| duplicate keys: quoted keys count | `duplicate_keys_quoted_keys_count` | covered |
| a zizmor ignore comment is found with the line it sits on | `a_zizmor_ignore_comment_is_found_with_the_line_it_sits_on` | covered |
| ordinary comments are not directives | `ordinary_comments_are_not_directives` | covered |
| a zizmor directive inside a run script is part of the script and not a YAML comment | `a_zizmor_directive_inside_a_run_script_is_part_of_the_script` | covered |
| the same directives compare equal, however the file is otherwise formatted | `the_same_directives_compare_equal_however_the_file_is_otherwise_formatted` | covered |
| a dropped directive and an added one are both reported | `a_dropped_directive_and_an_added_one_are_both_reported` | covered |
| a directive moved to a different line is reported | `a_directive_moved_to_a_different_line_is_reported_twice` | covered (Rust pins that it is reported twice) |
| a directive on a quoted key is the same line as on the bare key | `a_directive_on_a_quoted_key_is_the_same_line_as_on_the_bare_key` | covered |

## Deliberate differences

Where the Rust side differs from the TypeScript side on purpose. The TypeScript side is the one that is wrong or weaker in each case, except the last. The plan's Phase 0 rule "a gap becomes a Rust test first" has this second branch: record the difference and pin the Rust behaviour with a test.

| Module | TypeScript | Rust | Pinned by |
|---|---|---|---|
| `semver` | `compareSemver("1.0.0-01", "1.0.0-1")` and the reverse both return 1, which is not an order | Compares numeric identifiers by length, then text, so the two orders are opposite | **added in Phase 0**: `the_order_of_two_numbers_that_differ_only_in_leading_zeros_is_the_same_both_ways_round` |
| `semver` | `parseSemver("99999999999999999999.0.0")` accepts it as 1e20 | Returns `None` (the number does not fit `u64`) | **added in Phase 0**: `a_number_too_large_to_hold_is_not_a_version` |
| `contract` | A newer `schemaVersion` says "unsupported contract schemaVersion 3, this runner supports 2" | Says "contract schemaVersion 3 is newer than this runner supports (2): the package needs a newer runner"; older versions keep the old wording | `an_unknown_schema_version_is_refused_before_anything_else` |
| `preflight` | A runner version that is not a version throws from `compareSemver` | Reports an error naming it | `a_runner_version_that_is_not_a_version_is_an_error` |
| `manifest` | No `presets` | Reads and writes `presets` | `presets_are_written_after_the_files_sorted_and_read_back` and three more |
| `knownbad` | A second file, `runner/src/known-bad.bundled.json`, and a test that it is a subset of the root file | The built-in snapshot is the root `known-bad.json` itself (`include_str!`), so the two cannot differ | `the_built_in_snapshot_is_a_valid_list_and_names_the_version_that_cannot_read_a_checksum` |

Found by the Technical review of the plan and **not yet checked or tested**:

- `manifest`: the review reasoned (did not run) that TypeScript accepts an array as the `files` map, because `typeof [] === "object"`, and that Rust refuses it. Rust has `a_manifest_without_a_files_map_is_refused`.
- `checksums`: JavaScript treats a lone byte-order mark as whitespace and Rust does not, so a line holding only U+FEFF is skipped by TypeScript and is an error in Rust. Low.
- `manifest::sha256` takes text. Release and promote hash binary files (`release.ts`, `promote.ts`), so Phase 1 needs a variant that takes bytes.

## Phase 0 probes

Run on 15-11-2026 against `origin/main` at `8eae6ad` plus the tests added here.

| Probe | Result |
|---|---|
| Crates with a `build.rs` in the `scaffold-core` build tree (80 crates, normal and build dependencies) | 15: `crc32fast`, `getrandom`, `httparse`, `libc`, `num-traits`, `proc-macro2`, `quote`, `ring`, `rustix`, `rustls`, `serde`, `serde_core`, `serde_json`, `thiserror`, `zmij`. Also three procedural macros that run at build time: `serde_derive`, `thiserror-impl`, `time-macros`. All would compile in whatever job builds `scaffold-release` while it depends on `scaffold-core`, which is why D2 builds it in an unprivileged job |
| `cargo deny check` on the current tree | advisories ok, bans ok, licences ok, sources ok |
| `cargo deny check` with the `toml` crate added as a dev-dependency of `scaffold-core` (tried in a scratch clone, not committed) | advisories ok, bans ok, licences ok, sources ok. It adds six crates: `toml`, `toml_datetime`, `toml_parser`, `toml_writer`, `serde_spanned`, `winnow`. The TypeScript workflow tests use `Bun.TOML`, so a TOML crate is needed if they are ported |
| An HTTP server for the baseline's `packaged` script | No new crate needed: `http.rs` already has a `TcpListener` test server. Not yet tried for the baseline itself |
| Cold build time | `cargo build -p scaffold-core --locked` with an empty `target`: 15 s in the debug profile and 25 s in the release profile, on the owner's Mac. GitHub's runners are slower and were not measured |
| Other repositories that use these scripts | GitHub code search returned 0 hits, but it also returned 0 hits for `release-assets` inside this repository, so it proves nothing. Instead the workflow files of `pantheon-adr`, `pantheon-journal`, `pantheon-skill-auditor`, `pantheon-agentic-harness-dev-kit`, `tekhne` and `pantheon-scaffold-spike` were read: none mentions `release-assets`, `update-pins`, `runner/scripts`, `runner/src` or `bun runner`. The organisation's other repositories were not read |

Not probed: whether two successive real releases of the same source have identical `checksums.txt`.

## How to re-derive this page

For a module, list the TypeScript tests with `grep -nE '^\s*(test|it)\(' runner/test/<module>.test.ts` and the Rust tests with `grep -nE 'fn ' crates/scaffold-core/src/<module>.rs` after the line `mod tests`. Most Rust tests are named after the TypeScript title in snake case. The rows with a note are the ones that are not.

## Added in Phase 6: the release tooling tests

Written on 16-11-2026 for Phase 6 of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). The three test files below were ported with their modules in Phases 1, 2 and 5; this is the evidence, before they are deleted, that each TypeScript test has a named Rust test. Every row was checked against the file: the Rust name exists.

### release

| TypeScript test (`runner/test/release.test.ts`) | Rust test (`crates/scaffold-release/src/lib.rs`) | Note |
|---|---|---|
| pklProjectVersion reads the one version line of the package block | `pkl_project_version_reads_the_one_version_line_of_the_package_block` | covered; the TypeScript test stays, because `package-uri.test.ts` still imports `pklProjectVersion` (Phase 7) |
| a tag that matches the PklProject version passes, pre-release tags included | `a_tag_that_matches_the_pkl_project_version_passes_pre_release_tags_included` | covered |
| a tag that is not v followed by a version is refused | `a_tag_that_is_not_v_followed_by_a_version_is_refused` | covered |
| a tag that differs from the PklProject version is refused, so the package URI cannot point at the wrong release | `a_tag_that_differs_from_the_pkl_project_version_is_refused` | covered |
| the release carries the package files, the three binaries and the checksums | `the_release_carries_the_package_files_the_three_binaries_and_the_checksums` | covered |
| checksums.txt lists the binaries, the package metadata and the zip, and parseChecksums reads it back | `checksums_list_the_binaries_the_package_metadata_and_the_zip_and_parse_back` | covered |
| the metadata checksum is the one pkl checks with ::sha256, the same as the .sha256 file | `the_metadata_checksum_is_the_one_pkl_checks_with_the_same_as_the_sha256_file` | covered |
| a missing binary or package file is an error that lists what is missing | `a_missing_binary_or_package_file_is_an_error_that_lists_what_is_missing` | covered |
| a zip that does not match the checksum in the metadata is refused | `a_zip_that_does_not_match_the_checksum_in_the_metadata_is_refused` | covered |
| a .sha256 file that does not match its file is refused | `a_sha256_file_that_does_not_match_its_file_is_refused` | covered |
| metadata for another version is refused | `metadata_for_another_version_is_refused_even_when_its_sha256_file_matches` | covered |
| the wrapper prints the version for a matching tag and fails with exit 1 for another | `the_command_prints_the_version_for_a_matching_tag_and_exits_1_for_another` | covered |
| the wrapper writes checksums.txt into the release folder | `the_command_writes_checksums_txt_into_the_release_folder_and_prints_it` | covered |
| the wrapper reports a usage error with exit 2 | `a_usage_error_exits_2_with_the_usage_line` | covered |
| the wrapper prints the PklProject version for a dry run that has no tag | `the_command_prints_the_pkl_project_version_for_a_dry_run_that_has_no_tag` | covered |
| a .zip.sha256 file that does not match the zip is refused | `a_zip_sha256_file_that_does_not_match_the_zip_is_refused` | covered |

### promote

| TypeScript test (`runner/test/promote.test.ts`) | Rust test (`crates/scaffold-release/src/promote/tests.rs`) | Note |
|---|---|---|
| a dry run passes every guard and edits nothing | `a_dry_run_passes_every_guard_and_edits_nothing` | covered |
| a promotion edits the flag, the title and the notes once, and checks the result | `a_promotion_edits_the_flag_the_title_and_the_notes_once_and_checks_the_result` | covered |
| it runs only from main | `it_runs_only_from_main` | covered |
| a tag that is not a release tag is refused | `a_tag_that_is_not_a_release_tag_is_refused` | covered |
| a release that is already stable, or a draft, is not promoted | `a_release_that_is_already_stable_or_a_draft_is_not_promoted` | covered |
| the release must carry exactly the files a release carries | `the_release_must_carry_exactly_the_files_a_release_carries` | covered |
| a checksums file without a valid attestation stops the promotion | `a_checksums_file_without_a_valid_attestation_stops_the_promotion` | covered |
| an asset whose bytes do not match checksums.txt stops the promotion | `an_asset_whose_bytes_do_not_match_checksums_txt_stops_the_promotion` | covered |
| a version on the known-bad list is refused with its reason | `a_version_on_the_known_bad_list_is_refused_with_its_reason` | covered |
| the release workflow must have succeeded for the tag, verify job included | `the_release_workflow_must_have_succeeded_for_the_tag_verify_job_included` | covered |
| after the edit, a release that is still a pre-release, not latest, or whose checksums changed is reported as a failure | `after_the_edit_a_release_that_is_still_a_pre_release_not_latest_or_with_changed_checksums_is_a_failure` | covered |
| the stable wording drops the pre-release label and nothing else | `the_stable_wording_drops_the_pre_release_label_and_nothing_else` | covered |

### build-rust

| TypeScript test (`runner/test/build-rust.test.ts`) | Rust test (`crates/scaffold-release/src/build_rust/tests.rs`) | Note |
|---|---|---|
| every release target has a Rust target, and the Linux ones are static musl builds | `every_release_target_has_a_rust_target_and_the_linux_ones_are_static_musl_builds` | covered |
| the plan builds the command line crate, release, locked, for the target, with the version set | `the_plan_builds_the_command_line_crate_release_locked_for_the_target_with_the_version_set` | covered |
| the asset names are the ones the release has always used, so mise picks the same files | `the_asset_names_are_the_ones_the_release_has_always_used_so_mise_picks_the_same_files` | covered |
| a Linux build compiles its C code with a zig cc wrapper for its own target, a macOS build is left alone | `a_linux_build_compiles_its_c_code_with_a_zig_cc_wrapper_for_its_own_target_and_a_macos_build_is_left_alone` | covered |
| nothing in a plan needs musl-gcc or the Ubuntu archive | `nothing_in_a_plan_needs_musl_gcc_or_the_ubuntu_archive` | covered |
| the wrapper hands zig the other arguments untouched, with spaces kept, and drops the --target the cc crate adds | `the_wrapper_hands_zig_the_other_arguments_untouched_with_spaces_kept_and_drops_the_target_the_cc_crate_adds` | covered |
| only the musl builds must be static | `only_the_musl_builds_must_be_static` | covered |
| a leading v is dropped from the version, and a pre-release is a version | `a_leading_v_is_dropped_from_the_version_and_a_pre_release_is_a_version` | covered |
| an unknown target or a version that is not one is refused | `an_unknown_target_or_a_version_that_is_not_one_is_refused` | covered |
| ldd output says whether a binary is static | `ldd_output_says_whether_a_binary_is_static` | covered |
| the host decides which target can be built here | `the_host_decides_which_target_can_be_built_here` | covered |
| the arguments are --target, --version and --out, and the target is required | `the_arguments_are_target_version_and_out_and_the_target_is_required` | covered |

