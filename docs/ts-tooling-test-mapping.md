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
| pklProjectVersion reads the one version line of the package block | `pkl_project_version_reads_the_one_version_line_of_the_package_block` | covered; the TypeScript test and `release.ts` were deleted in Phase 7a, with their last user `package-uri.test.ts` |
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


## Added in Phase 7a: the first package tests, ported

Written on 16-11-2026 for Phase 7a of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). These TypeScript tests were ported to the crate `scaffold-package-tests` and deleted. Every row was checked against the files by script: the TypeScript title is the one in the deleted file (read from `origin/main`), and the Rust test exists. A TypeScript loop that generated several tests is mapped row by row.

### templates

| TypeScript test (`runner/test/templates.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/templates.rs`) | Note |
|---|---|---|
| every toml, yaml and json asset a builtin ships is a Jinja template, apart from the one named exception | `every_toml_yaml_and_json_asset_a_builtin_ships_is_a_jinja_template_apart_from_the_one_named_exception` | covered |
| no asset still uses an @@NAME@@ placeholder | `no_asset_still_uses_an_at_at_name_at_at_placeholder` | covered |
| no builtin patches rendered text with replaceAll or replaceFirst: a variable and a conditional do that in the template | `no_builtin_patches_rendered_text_with_replace_all_or_replace_first_a_variable_and_a_conditional_do_that_in_the_template` | covered |

### config-name

| TypeScript test (`runner/test/config-name.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/config_name.rs`) | Note |
|---|---|---|
| no tracked file is named pantheon.pkl | `no_tracked_file_is_named_pantheon_pkl` | covered |
| the old config name appears only in the files that are allowed to say it | `the_old_config_name_appears_only_in_the_files_that_are_allowed_to_say_it` | covered |
| every allowed file still says it, so the list does not outlive its reasons | `every_allowed_file_still_says_it_so_the_list_does_not_outlive_its_reasons` | covered |

### mode-index

| TypeScript test (`runner/test/mode-index.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/mode_index.rs`) | Note |
|---|---|---|
| git records the nine shared scripts as 100755 and the configs as 100644 after a render and a commit | `git_records_the_nine_shared_scripts_as_100755_and_the_configs_as_100644_after_a_render_and_a_commit` | covered |
| check on a fresh clone of the committed render finds no drift | `check_on_a_fresh_clone_of_the_committed_render_finds_no_drift` | covered |
| a script committed without the executable bit is mode drift on a fresh clone (exit 4) | `a_script_committed_without_the_executable_bit_is_mode_drift_on_a_fresh_clone_exit_4` | covered |

### package-render

| TypeScript test (`runner/test/package-render.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/package_render.rs`) | Note |
|---|---|---|
| the rendered workflows carry version comments and still equal the pinned upstream copies | `the_rendered_workflows_carry_version_comments_and_still_equal_the_pinned_upstream_copies` | covered |

### consumer-check

| TypeScript test (`runner/test/consumer-check.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/consumer_check.rs`) | Note |
|---|---|---|
| the consumer check runs on pull_request only, never pull_request_target | `the_consumer_check_runs_on_pull_request_only_never_pull_request_target` | covered |
| the consumer check uses no secrets | `the_consumer_check_uses_no_secrets` | covered |
| the token is the built-in one with read access only | `the_token_is_the_built_in_one_with_read_access_only` | covered |
| every action is pinned by a 40 character SHA | `every_action_is_pinned_by_a_40_character_sha` | covered |
| checkout does not persist credentials | `checkout_does_not_persist_credentials` | covered |
| the tools come from the lock file | `the_tools_come_from_the_lock_file` | covered |
| the package cache is kept between runs, so an outage does not block the check | `the_package_cache_is_kept_between_runs_so_an_outage_does_not_block_the_check` | covered |

### smoke-fixtures

| TypeScript test (`runner/test/smoke-fixtures.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/smoke_fixtures.rs`) | Note |
|---|---|---|
| smoke-orphan-a renders two files and smoke-orphan-b drops the second | `smoke_orphan_a_renders_two_files_and_smoke_orphan_b_drops_the_second` | covered |
| smoke-case-collision renders two paths that differ only by case | `smoke_case_collision_renders_two_paths_that_differ_only_by_case` | covered |
| smoke-case-a and smoke-case-b render the same file under names that differ only by case | `smoke_case_a_and_smoke_case_b_render_the_same_file_under_names_that_differ_only_by_case` | covered |
| the phase 3 fixtures render the same 19 files, with the default and with all features | `the_phase_3_fixtures_render_the_same_19_files_with_the_default_and_with_all_features` | covered |

### package-uri

| TypeScript test (`runner/test/package-uri.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/package_uri.rs`) | Note |
|---|---|---|
| a package loaded from a package URI reports that URI, and the runner reads its version from it | `a_package_loaded_from_a_package_uri_reports_that_uri_and_the_runner_reads_its_version_from_it` | covered |
| a package loaded from a local folder has no packageUri | `a_package_loaded_from_a_local_folder_has_no_package_uri` | covered |
| a file marked removeOnDetach reaches the contract, and an unmarked one does not carry the key | `a_file_marked_remove_on_detach_reaches_the_contract_and_an_unmarked_one_does_not_carry_the_key` | covered |
| a consumer that pins the package with its checksum still gets a packageUri the runner can read the version from | `a_consumer_that_pins_the_package_with_its_checksum_still_gets_a_package_uri_the_runner_can_read_the_version_from` | covered |
| the mise builtin pins the runner at the package version when loaded from a package URI | `the_mise_builtin_pins_the_runner_at_the_package_version_when_loaded_from_a_package_uri` | covered (one iteration of a TypeScript loop) |
| the mise builtin pins the runner at the package version when loaded from a package URI that carries the pinned checksum | `the_mise_builtin_pins_the_runner_at_the_package_version_when_loaded_from_a_package_uri_that_carries_the_pinned_checksum` | covered (one iteration of a TypeScript loop); the last two rows are one TypeScript loop over `pinChecksum` false and true |

### dependabot-entry

| TypeScript test (`runner/test/dependabot-entry.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/dependabot_entry.rs`) | Note |
|---|---|---|
| a binary: the github-actions entry is rendered, the cargo entry always | `a_binary_renders_the_github_actions_entry_and_the_cargo_entry` | covered (one iteration of a TypeScript loop) |
| a composite action: the github-actions entry is rendered, the cargo entry always | `a_composite_action_renders_the_github_actions_entry_and_the_cargo_entry` | covered (one iteration of a TypeScript loop) |
| the draft release shape: the github-actions entry is rendered, the cargo entry always | `the_draft_release_shape_renders_the_github_actions_entry_and_the_cargo_entry` | covered (one iteration of a TypeScript loop) |
| builtins left out (one workflow): the github-actions entry is rendered, the cargo entry always | `builtins_left_out_one_workflow_render_the_github_actions_entry_and_the_cargo_entry` | covered (one iteration of a TypeScript loop) |
| a tool Builtin left out (it renders no workflow): the github-actions entry is left out, the cargo entry always | `a_tool_builtin_left_out_that_renders_no_workflow_leaves_the_github_actions_entry_out` | covered (one iteration of a TypeScript loop) |
| a Builtin the toggles already removed (release without binary): the github-actions entry is left out, the cargo entry always | `a_builtin_the_toggles_already_removed_leaves_the_github_actions_entry_out` | covered (one iteration of a TypeScript loop) |
| everything a workflow Builtin could be left out: the github-actions entry is rendered, the cargo entry always | `everything_a_workflow_builtin_could_be_left_out_renders_the_github_actions_entry` | covered (one iteration of a TypeScript loop) |
| a plain library (the dev-kit): the github-actions entry is left out, the cargo entry always | `a_plain_library_the_dev_kit_leaves_the_github_actions_entry_out` | covered (one iteration of a TypeScript loop) |
| on, on a plain library: the github-actions entry is rendered, the cargo entry always | `on_on_a_plain_library_renders_the_github_actions_entry` | covered (one iteration of a TypeScript loop) |
| off, on a binary: the github-actions entry is left out, the cargo entry always | `off_on_a_binary_leaves_the_github_actions_entry_out` | covered (one iteration of a TypeScript loop) |
| auto, spelt out, on a plain library: the github-actions entry is left out, the cargo entry always | `auto_spelt_out_on_a_plain_library_leaves_the_github_actions_entry_out` | covered (one iteration of a TypeScript loop) |
| the header has its four variants, and none claims what the file does not do | `the_header_has_its_four_variants_and_none_claims_what_the_file_does_not_do` | covered |
| this repository's options render the committed .github/dependabot.yml, so check stays clean | `this_repositorys_options_render_the_committed_dependabot_yml_so_check_stays_clean` | covered |
| a misspelt value is refused by the render | `a_misspelt_value_is_refused_by_the_render` | covered; the first eleven rows are the eleven cases of one TypeScript loop (`CASES`) |

41 rows. `runner/test/release.test.ts` and `runner/src/release.ts` were deleted in the same change: their one remaining test (`pklProjectVersion`) is the first row of the `release` table above, and its last user was `package-uri.test.ts`.

## Added in Phase 7b: the rendering tests, first part

Written on 16-11-2026 for Phase 7b of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). These five TypeScript test files were ported to `scaffold-package-tests` and deleted. Every row was checked by script: the TypeScript title is the one in the deleted file (read from `origin/main`; a title built in a loop is the template of the loop), and each Rust test is in the list that `cargo test -- --list` prints for the crate. A TypeScript loop that generated several tests is mapped with the Rust tests that replace it.

### builtins

| TypeScript test (`runner/test/builtins.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/builtins.rs`) | Note |
|---|---|---|
| the slice renders the three workflows, five verbatim files and no external file | `the_slice_renders_the_three_workflows_five_verbatim_files_and_no_external_file` | covered |
| ${name}.yml equals the pinned pantheon-adr copy under the P.1 rule | `cargo_deny_yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule`, `inline_scripts_yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule`, `rust_ci_yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule` | 3 TypeScript tests (a loop over three names), 3 Rust tests |
| ${name}.sh is byte for byte the asset and executable | `cargo_deny_sh_is_byte_for_byte_the_asset_and_executable`, `check_inline_scripts_sh_is_byte_for_byte_the_asset_and_executable`, `check_toolchain_version_sh_is_byte_for_byte_the_asset_and_executable` | 3 TypeScript tests (a loop over three names), 3 Rust tests |
| deny.toml renders as valid toml with the shared licence list and is not executable | `deny_toml_renders_as_valid_toml_with_the_shared_licence_list_and_is_not_executable` | covered |
| rust-toolchain.toml carries the one toolchain option | `rust_toolchain_toml_carries_the_one_toolchain_option` | covered |
| the toolchain option reaches both workflows that install it | `the_toolchain_option_reaches_both_workflows_that_install_it` | covered |
| the slice uses no escape hatch | `the_slice_uses_no_escape_hatch` | covered |
| the only pin is checkout at its release tag | `the_only_pin_is_checkout_at_its_release_tag` | covered |
| ${path} renders byte for byte as the pinned pantheon-adr file (git blob ${blob.slice(0, 7)}) | `each_verbatim_file_renders_byte_for_byte_as_the_pinned_pantheon_adr_file` | 5 TypeScript tests, one for each entry of `runner/test/upstream/blobs.json`; one Rust test loops over the same 5 entries and asserts there are 5 |

### workflows-a

| TypeScript test (`runner/test/workflows-a.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/workflows_a.rs`) | Note |
|---|---|---|
| the pinned upstream copies are the pantheon-adr workflows at 1e2fd46, identical in all four repositories | `the_pinned_upstream_copies_are_the_pantheon_adr_workflows_at_1e2fd46_identical_in_all_four_repositories` | covered |
| wave A renders the six workflows and the four config files they use | `wave_a_renders_the_six_workflows_and_the_four_config_files_they_use` | covered |
| the one per-file ignore in the rendered .rumdl.toml is for the pull request template this contract renders | `the_one_per_file_ignore_in_the_rendered_rumdl_toml_is_for_the_pull_request_template_this_contract_renders` | covered |
| ${name}.yml equals the pinned pantheon-adr copy under the P.1 rule | `plumber::yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule`, `sync_labels::yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule`, `pr_auto_rebase::yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule`, `release_automerge::yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule`, `commit_types::yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule`, `conventional_commits::yml_equals_the_pinned_pantheon_adr_copy_under_the_p1_rule` | 6 TypeScript tests (a loop over six workflows), 6 Rust tests, one in a module for each |
| ${name}.yml keeps every comment a tool acts on | `plumber::yml_keeps_every_comment_a_tool_acts_on`, `sync_labels::yml_keeps_every_comment_a_tool_acts_on`, `pr_auto_rebase::yml_keeps_every_comment_a_tool_acts_on`, `release_automerge::yml_keeps_every_comment_a_tool_acts_on`, `commit_types::yml_keeps_every_comment_a_tool_acts_on`, `conventional_commits::yml_keeps_every_comment_a_tool_acts_on` | 6 and 6, as above |
| ${name}.yml keeps its job ids and job names, which the required status checks depend on | `plumber::yml_keeps_its_job_ids_and_job_names_which_the_required_status_checks_depend_on`, `sync_labels::yml_keeps_its_job_ids_and_job_names_which_the_required_status_checks_depend_on`, `pr_auto_rebase::yml_keeps_its_job_ids_and_job_names_which_the_required_status_checks_depend_on`, `release_automerge::yml_keeps_its_job_ids_and_job_names_which_the_required_status_checks_depend_on`, `commit_types::yml_keeps_its_job_ids_and_job_names_which_the_required_status_checks_depend_on`, `conventional_commits::yml_keeps_its_job_ids_and_job_names_which_the_required_status_checks_depend_on` | 6 and 6, as above |
| the zizmor ignore comment on the pull_request_target trigger is rendered on the on: line | `the_zizmor_ignore_comment_on_the_pull_request_target_trigger_is_rendered_on_the_on_line` | covered |
| the four config files are byte for byte the pinned pantheon-adr files and not executable | `the_four_config_files_are_byte_for_byte_the_pinned_pantheon_adr_files_and_not_executable` | covered |
| the pins are the three actions the workflows use, each a commit with a version tag | `the_pins_are_the_three_actions_the_workflows_use_each_a_commit_with_a_version_tag` | covered |
| wave A uses no escape hatch | `wave_a_uses_no_escape_hatch` | covered |
| the App client id is an option, with the pantheon-ai-bot id as the default | `the_app_client_id_is_an_option_with_the_pantheon_ai_bot_id_as_the_default` | covered |
| actionlint is clean on the six rendered workflows | `actionlint_is_clean_on_the_six_rendered_workflows` | covered |
| plumber.yml ends its header with the notice that it is generated, and no other workflow carries it | `plumber_yml_ends_its_header_with_the_notice_that_it_is_generated_and_no_other_workflow_carries_it` | covered |

### workflows-b

| TypeScript test (`runner/test/workflows-b.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/workflows_b.rs`) | Note |
|---|---|---|
| the pinned copies are the repositories' files at the pinned commits | `the_pinned_copies_are_the_repositories_files_at_the_pinned_commits` | covered |
| the files each kind of repository gets | `the_files_each_kind_of_repository_gets` | covered |
| ${label}: ${path} equals the pinned copy under P.1 | `adr_like_release_please::equals_the_pinned_copy_under_p1`, `adr_like_zizmor::equals_the_pinned_copy_under_p1`, `adr_like_agnix::equals_the_pinned_copy_under_p1`, `adr_like_aislop::equals_the_pinned_copy_under_p1`, `adr_like_rust_ci::equals_the_pinned_copy_under_p1`, `library_like_release_please::equals_the_pinned_copy_under_p1`, `library_like_zizmor::equals_the_pinned_copy_under_p1`, `library_like_agnix::equals_the_pinned_copy_under_p1`, `library_like_aislop::equals_the_pinned_copy_under_p1`, `library_like_rust_ci::equals_the_pinned_copy_under_p1`, `auditor_like_zizmor::equals_the_pinned_copy_under_p1`, `adr_like_dependabot_yml_equals_the_pinned_copy_under_p1`, `library_like_dependabot_yml_equals_the_pinned_copy_under_p1` | 13 TypeScript tests (the `same` helper: 11 workflows and 2 dependabot.yml files), 13 Rust tests |
| ${label}: ${path} keeps the comments a tool acts on and its job ids and names | `adr_like_release_please::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `adr_like_zizmor::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `adr_like_agnix::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `adr_like_aislop::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `adr_like_rust_ci::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `library_like_release_please::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `library_like_zizmor::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `library_like_agnix::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `library_like_aislop::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `library_like_rust_ci::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names`, `auditor_like_zizmor::keeps_the_comments_a_tool_acts_on_and_its_job_ids_and_names` | 11 TypeScript tests (the workflows only), 11 Rust tests |
| the verbatim config files are byte for byte the repositories' files | `the_verbatim_config_files_are_byte_for_byte_the_repositories_files` | covered |
| a repository without a skill differs from one with a skill only in the first line of .agnix.toml | `a_repository_without_a_skill_differs_from_one_with_a_skill_only_in_the_first_line_of_agnix_toml` | covered |
| the tool versions the workflows pin are the ones in Tools.pkl, the same ones mise.toml pins | `the_tool_versions_the_workflows_pin_are_the_ones_in_tools_pkl_the_same_ones_mise_toml_pins` | covered |
| the new pins are each a commit with a version tag, and the workflows that use them report them | `the_new_pins_are_each_a_commit_with_a_version_tag_and_the_workflows_that_use_them_report_them` | covered |
| wave B uses no escape hatch in any of the three renders | `wave_b_uses_no_escape_hatch_in_any_of_the_three_renders` | covered |
| actionlint is clean on the rendered workflows of a repository with a binary and of one without | `actionlint_is_clean_on_the_rendered_workflows_of_a_repository_with_a_binary_and_of_one_without` | covered |

### workflows-c

| TypeScript test (`runner/test/workflows-c.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/workflows_c.rs`) | Note |
|---|---|---|
| it renders one owned file that detach deletes | `it_renders_one_owned_file_that_detach_deletes` | covered |
| it runs on a schedule or by hand, never on a pull request or a push | `it_runs_on_a_schedule_or_by_hand_never_on_a_pull_request_or_a_push` | covered |
| nothing has permissions by default and the job reads contents only | `nothing_has_permissions_by_default_and_the_job_reads_contents_only` | covered |
| the App token is the only credential, and it asks for workflows write as well as contents and pull requests | `the_app_token_is_the_only_credential_and_it_asks_for_workflows_write_as_well_as_contents_and_pull_requests` | covered |
| every action is pinned by a 40 character SHA and checkout keeps no credentials | `every_action_is_pinned_by_a_40_character_sha_and_checkout_keeps_no_credentials` | covered |
| no run script interpolates an expression: everything goes through env | `no_run_script_interpolates_an_expression_everything_goes_through_env` | covered |
| every run script is 15 lines or fewer | `every_run_script_is_15_lines_or_fewer` | covered |
| it checks freshness, then finds the latest release, then upgrades, in that order | `it_checks_freshness_then_finds_the_latest_release_then_upgrades_in_that_order` | covered |
| a repository already on the latest release opens no pull request | `a_repository_already_on_the_latest_release_opens_no_pull_request` | covered |
| the pull request is opened with the App token, never the default token | `the_pull_request_is_opened_with_the_app_token_never_the_default_token` | covered |
| the commit and the pull request title are conventional commits | `the_commit_and_the_pull_request_title_are_conventional_commits` | covered |

### tooling

| TypeScript test (`runner/test/tooling.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/tooling.rs`) | Note |
|---|---|---|
| the tooling files are the nine shared scripts, check-toolchain-version.sh, the two configs and the base files | `the_tooling_files_are_the_nine_shared_scripts_check_toolchain_version_sh_the_two_configs_and_the_base_files` | covered |
| all nine shared scripts render executable and every config renders not executable | `all_nine_shared_scripts_render_executable_and_every_config_renders_not_executable` | covered |
| every file is an owned file with content, and the Phase 3 files use no escape hatch | `every_file_is_an_owned_file_with_content_and_the_phase_3_files_use_no_escape_hatch` | covered |
| (a title built from the entry: `${path} renders byte for byte as the pinned pantheon-adr file (git blob ${blob.slice(0, 7)})`, and for `.rumdl.toml` `${path} renders as the pinned pantheon-adr file plus the one per-file-ignores table (git blob ${blob.slice(0, 7)} without it)`) | `each_pinned_file_renders_byte_for_byte_as_the_pinned_pantheon_adr_file_and_rumdl_toml_with_its_one_table` | 9 TypeScript tests, one for each entry of `runner/test/upstream/tooling-blobs.json`; one Rust test loops over the same 9 entries and asserts there are 9, with the `.rumdl.toml` case written as in the TypeScript |
| the rendered .rumdl.toml ignores MD041 for the pull request template and nowhere else | `the_rendered_rumdl_toml_ignores_md041_for_the_pull_request_template_and_nowhere_else` | covered |
| the pinned list covers the seven new scripts and both configs, and the nine scripts are all accounted for | `the_pinned_list_covers_the_seven_new_scripts_and_both_configs_and_the_nine_scripts_are_all_accounted_for` | covered |
| the one script that is not executable upstream is rendered executable on purpose, and it is the only one | `the_one_script_that_is_not_executable_upstream_is_rendered_executable_on_purpose_and_it_is_the_only_one` | covered |

The five files had 103 TypeScript tests (builtins 17, workflows-a 28, workflows-b 32, workflows-c 11, tooling 15) and are replaced by 91 Rust tests. The 12 fewer are the entries of two data-driven loops that one Rust test each now covers: 5 entries in builtins (4 fewer) and 9 in tooling (8 fewer).

## Added in Phase 7b, second part: hk, mise and the phase5 files

Written on 16-11-2026 for Phase 7b of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). These seven TypeScript test files were ported to `scaffold-package-tests` and deleted. Every row was checked by script: the TypeScript title is the one in the deleted file (read from `origin/main`; a title built in a loop is the template of the loop), and each Rust test is in the list that `cargo test -- --list` prints for the crate. For a plain title the Rust name is the title in snake_case, derived by the script, so a name cannot be mistyped.

### hk

| TypeScript test (`runner/test/hk.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/hk.rs`) | Note |
|---|---|---|
| the pinned upstream copies are the hk.pkl files of pantheon-adr at 1e2fd46 and of the library repo at 42f48b4 | `the_pinned_upstream_copies_are_the_hk_pkl_files_of_pantheon_adr_at_1e2fd46_and_of_the_library_repo_at_42f48b4` | covered |
| the hook files are the owned shared steps module and the seeded hk.pkl | `the_hook_files_are_the_owned_shared_steps_module_and_the_seeded_hk_pkl` | covered |
| the pre-commit steps are byte for byte the ones in pantheon-adr's hk.pkl | `the_pre_commit_steps_are_byte_for_byte_the_ones_in_pantheon_adrs_hk_pkl` | covered |
| the pre-push steps with the default features are byte for byte the ones in pantheon-adr's hk.pkl | `the_pre_push_steps_with_the_default_features_are_byte_for_byte_the_ones_in_pantheon_adrs_hk_pkl` | covered |
| the pre-push steps with all features are byte for byte the ones in the library repo's hk.pkl | `the_pre_push_steps_with_all_features_are_byte_for_byte_the_ones_in_the_library_repos_hk_pkl` | covered |
| the pre-commit steps differ from the library repo's only by one comment, which is wording tied to .github/actionlint.yaml | `the_pre_commit_steps_differ_from_the_library_repos_only_by_one_comment_which_is_wording_tied_to_github_actionlint_yaml` | covered |
| the seeded hk.pkl is pantheon-adr's hooks block with the steps taken from the shared module | `the_seeded_hk_pkl_is_pantheon_adrs_hooks_block_with_the_steps_taken_from_the_shared_module` | covered |
| the seeded hk.pkl pins the hk config package, imports the shared module and holds no step of its own | `the_seeded_hk_pkl_pins_the_hk_config_package_imports_the_shared_module_and_holds_no_step_of_its_own` | covered |
| the shared module says it is rendered and imports the same hk config package | `the_shared_module_says_it_is_rendered_and_imports_the_same_hk_config_package` | covered |
| real hk validates the render with the default features and with all features | `real_hk_validates_the_render_with_the_default_features_and_with_all_features` | covered |
| real hk plans the five shared steps, and a step a repository adds to the seeded file next to them | `real_hk_plans_the_five_shared_steps_and_a_step_a_repository_adds_to_the_seeded_file_next_to_them` | covered |
| the hook fixers and checks over a fresh render pass and change nothing | `the_hook_fixers_and_checks_over_a_fresh_render_pass_and_change_nothing` | covered |

### mise

| TypeScript test (`runner/test/mise.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/mise.rs`) | Note |
|---|---|---|
| the mise files are the owned mise.toml, the external mise.lock and the seeded per-repo config | `the_mise_files_are_the_owned_mise_toml_the_external_mise_lock_and_the_seeded_per_repo_config` | covered |
| the pinned upstream copy is the pantheon-adr mise.toml at 1e2fd46 (git blob 16c264c) | `the_pinned_upstream_copy_is_the_pantheon_adr_mise_toml_at_1e2fd46_git_blob_16c264c` | covered |
| mise.toml differs from the repositories' current file only by the named additions and the two comments | `mise_toml_differs_from_the_repositories_current_file_only_by_the_named_additions_and_the_two_comments` | covered |
| the tool specifiers are the ones the repositories use today: exact stays exact and latest stays latest (decision q14, option A) | `the_tool_specifiers_are_the_ones_the_repositories_use_today_exact_stays_exact_and_latest_stays_latest_decision_q14_option_a` | covered |
| lockfile_platforms is exactly the platforms the runner is built for | `lockfile_platforms_is_exactly_the_platforms_the_runner_is_built_for` | covered |
| the toolchain option reaches the rust entry | `the_toolchain_option_reaches_the_rust_entry` | covered |
| the seeded per-repo config says what it is for and holds no tools | `the_seeded_per_repo_config_says_what_it_is_for_and_holds_no_tools` | covered |
| using the mise builtin without a runner version and not as a released package fails with a clear message | `using_the_mise_builtin_without_a_runner_version_and_not_as_a_released_package_fails_with_a_clear_message` | covered |
| real mise reads both files, adds the repo's own tool and lets the owned mise.toml win a conflict | `real_mise_reads_both_files_adds_the_repos_own_tool_and_lets_the_owned_mise_toml_win_a_conflict` | covered |

### phase5-dependabot

| TypeScript test (`runner/test/phase5-dependabot.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/phase5_dependabot.rs`) | Note |
|---|---|---|
| ${repo}: with a binary, Dependabot ignores upload-artifact and download-artifact in the github-actions entry only | `pantheon_adr_with_a_binary_dependabot_ignores_upload_artifact_and_download_artifact_in_the_github_actions_entry_only`, `pantheon_journal_with_a_binary_dependabot_ignores_upload_artifact_and_download_artifact_in_the_github_actions_entry_only`, `pantheon_skill_auditor_with_a_binary_dependabot_ignores_upload_artifact_and_download_artifact_in_the_github_actions_entry_only` | 3 TypeScript tests (a loop over three repositories), 3 Rust tests |
| the library repository, with no binary, has no github-actions entry and so no ignore list | `the_library_repository_with_no_binary_has_no_github_actions_entry_and_so_no_ignore_list` | covered |
| the header says why the two actions are ignored | `the_header_says_why_the_two_actions_are_ignored` | covered |
| ${fixture}: the github-actions entry excludes exactly the owned workflow files, and the cargo entry excludes nothing | `real_workflows_pantheon_adr_the_github_actions_entry_excludes_exactly_the_owned_workflow_files_and_the_cargo_entry_excludes_nothing` | 1 TypeScript test (a loop over one fixture), 1 Rust test |
| a repository with skills and a binary excludes audit-skill.yml and dist-check.yml | `a_repository_with_skills_and_a_binary_excludes_audit_skill_yml_and_dist_check_yml` | covered |

### phase5-dist

| TypeScript test (`runner/test/phase5-dist.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/phase5_dist.rs`) | Note |
|---|---|---|
| dist-check.yml is rendered for a repository with a binary and not for one without | `dist_check_yml_is_rendered_for_a_repository_with_a_binary_and_not_for_one_without` | covered |
| scaffold-check.yml is rendered for every repository | `scaffold_check_yml_is_rendered_for_every_repository` | covered |
| the dist version the check installs is the one dist-workspace.toml names | `the_dist_version_the_check_installs_is_the_one_dist_workspace_toml_names` | covered |
| dist-check.yml runs dist generate --check, on a pull request, a push to main or by hand, and never on pull_request_target | `dist_check_yml_runs_dist_generate_check_on_a_pull_request_a_push_to_main_or_by_hand_and_never_on_pull_request_target` | covered |
| dist-check.yml uses no secrets, reads contents only, pins every action and keeps no checkout credentials | `dist_check_yml_uses_no_secrets_reads_contents_only_pins_every_action_and_keeps_no_checkout_credentials` | covered |
| scaffold-check.yml equals the consumer-check sample under P.1, with the pins masked, and the directive comments | `scaffold_check_yml_equals_the_consumer_check_sample_under_p1_with_the_pins_masked_and_the_directive_comments` | covered |
| scaffold-check.yml is the same for every repository | `scaffold_check_yml_is_the_same_for_every_repository` | covered |
| the pins the two workflows use are the cache, checkout, mise and nothing unlisted | `the_pins_the_two_workflows_use_are_the_cache_checkout_mise_and_nothing_unlisted` | covered |

### phase5-parameterised

| TypeScript test (`runner/test/phase5-parameterised.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/phase5_parameterised.rs`) | Note |
|---|---|---|
| the pinned upstream copies are the repositories' files at the surveyed commits | `the_pinned_upstream_copies_are_the_repositories_files_at_the_surveyed_commits` | covered |
| ${r}: audit-skill.yml equals the repository's file under P.1, with the directive comments and job names | `pantheon_adr_skill::audit_skill_yml_equals_the_repositorys_file_under_p1_with_the_directive_comments_and_job_names`, `pantheon_journal_skill::audit_skill_yml_equals_the_repositorys_file_under_p1_with_the_directive_comments_and_job_names`, `pantheon_skill_auditor_skill::audit_skill_yml_equals_the_repositorys_file_under_p1_with_the_directive_comments_and_job_names` | 3 and 3: a loop over the three repositories with a skill, one Rust module for each |
| ${r}: tag-release.sh is byte for byte the repository's file and executable | `pantheon_adr_skill::tag_release_sh_is_byte_for_byte_the_repositorys_file_and_executable`, `pantheon_journal_skill::tag_release_sh_is_byte_for_byte_the_repositorys_file_and_executable`, `pantheon_skill_auditor_skill::tag_release_sh_is_byte_for_byte_the_repositorys_file_and_executable` | 3 and 3, as above |
| ${r}: .coderabbit.yaml is byte for byte the repository's file | `pantheon_adr_coderabbit_yaml_is_byte_for_byte_the_repositorys_file`, `pantheon_journal_coderabbit_yaml_is_byte_for_byte_the_repositorys_file`, `pantheon_skill_auditor_coderabbit_yaml_is_byte_for_byte_the_repositorys_file`, `pantheon_agentic_harness_dev_kit_coderabbit_yaml_is_byte_for_byte_the_repositorys_file` | 4 and 4: a loop over the four repositories |
| SECURITY.md is seeded, so a repository that words its scope differently keeps its own text | `security_md_is_seeded_so_a_repository_that_words_its_scope_differently_keeps_its_own_text` | covered |
| ${r}: SECURITY.md is byte for byte the repository's file | `pantheon_adr_security_md_is_byte_for_byte_the_repositorys_file`, `pantheon_journal_security_md_is_byte_for_byte_the_repositorys_file`, `pantheon_agentic_harness_dev_kit_security_md_is_byte_for_byte_the_repositorys_file` | 3 and 3: a loop over the three repositories whose file is the pinned one |
| pantheon-skill-auditor: SECURITY.md differs only in the three scope lines that name its crates, skill and composite action | `pantheon_skill_auditor_security_md_differs_only_in_the_three_scope_lines_that_name_its_crates_skill_and_composite_action` | covered |
| the library repository, with no binary and no skill, gets no audit-skill.yml and no tag-release.sh | `the_library_repository_with_no_binary_and_no_skill_gets_no_audit_skill_yml_and_no_tag_release_sh` | covered |
| the composite-action variant runs the repository's own action and the bootstrap binary, the others use the released action | `the_composite_action_variant_runs_the_repositorys_own_action_and_the_bootstrap_binary_the_others_use_the_released_action` | covered |
| every other file is unchanged by the new builtins: the rendered set grows by exactly the new files | `every_other_file_is_unchanged_by_the_new_builtins_the_rendered_set_grows_by_exactly_the_new_files` | covered |

### phase5-toggles

| TypeScript test (`runner/test/phase5-toggles.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/phase5_toggles.rs`) | Note |
|---|---|---|
| ${label}: removes and changes exactly the files that toggle owns, and adds none | `skills_off_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none`, `binary_off_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none`, `features_all_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none`, `composite_action_on_removes_and_changes_exactly_the_files_that_toggle_owns_and_adds_none` | 4 TypeScript tests (a loop over the four toggles), 4 Rust tests |
| a base configuration renders the same twice: the toggles have no hidden input | `a_base_configuration_renders_the_same_twice_the_toggles_have_no_hidden_input` | covered |
| skills on without skillName is a clear error that names the option and the way out | `skills_on_without_skill_name_is_a_clear_error_that_names_the_option_and_the_way_out` | covered |
| skills off needs no skillName, and binary off needs no crateName | `skills_off_needs_no_skill_name_and_binary_off_needs_no_crate_name` | covered |
| binary on without crateName is a clear error that names the option and the way out | `binary_on_without_crate_name_is_a_clear_error_that_names_the_option_and_the_way_out` | covered |
| turning skills and binary off deletes only the files they owned, and never touches the external release.yml | `turning_skills_and_binary_off_deletes_only_the_files_they_owned_and_never_touches_the_external_release_yml` | covered |
| turning a toggle back on writes its files again | `turning_a_toggle_back_on_writes_its_files_again` | covered |

### phase5-verbatim

| TypeScript test (`runner/test/phase5-verbatim.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/phase5_verbatim.rs`) | Note |
|---|---|---|
| a repository with a binary gets .gitignore, actionlint.yaml, dist-workspace.toml and release.yml | `a_repository_with_a_binary_gets_gitignore_actionlint_yaml_dist_workspace_toml_and_release_yml` | covered |
| a repository without a binary gets .gitignore only | `a_repository_without_a_binary_gets_gitignore_only` | covered |
| ${path} is byte for byte the pinned pantheon-adr file, owned and not executable | `actionlint_yaml_is_byte_for_byte_the_pinned_pantheon_adr_file_owned_and_not_executable` | 1 TypeScript test (a loop over the one entry of `BLOBS`, `.github/actionlint.yaml`), 1 Rust test |
| dist-workspace.toml is the pantheon-adr file with exactly the two action commits that release.yml uses | `dist_workspace_toml_is_the_pantheon_adr_file_with_exactly_the_two_action_commits_that_release_yml_uses` | covered |
| .gitignore is the pantheon-adr file plus exactly the line .pkl-gen/cache/, owned and not executable | `gitignore_is_the_pantheon_adr_file_plus_exactly_the_line_pkl_gen_cache_owned_and_not_executable` | covered |
| .gitignore is the same in both kinds of repository: target/ is the decided pick (decision 74) | `gitignore_is_the_same_in_both_kinds_of_repository_target_is_the_decided_pick_decision_74` | covered |
| release.yml is external: listed with no content, because dist generate writes it | `release_yml_is_external_listed_with_no_content_because_dist_generate_writes_it` | covered |
| an external release.yml is never written, and an existing one is left alone | `an_external_release_yml_is_never_written_and_an_existing_one_is_left_alone` | covered |

The seven files had 73 TypeScript tests (hk 12, mise 9, phase5-dependabot 7, phase5-dist 8, phase5-parameterised 19, phase5-toggles 10, phase5-verbatim 8) and are replaced by 73 Rust tests: one for each, because the loops here are over fixed names and are written out. The other 132 Rust tests of the crate are those of Phases 7a and 7b part one.

## Added in Phase 7c: presence and the release options

Written on 16-11-2026 for Phase 7c of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). These three TypeScript test files were ported to `scaffold-package-tests` and deleted. Every row was checked by script: the TypeScript title is the one in the deleted file (read from `origin/main`; a title built in a loop is the template of the loop), and each Rust test is in the list that `cargo test -- --list` prints for the crate. For a plain title the Rust name is the title in snake_case, derived by the script.

### presence

| TypeScript test (`runner/test/presence.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/presence.rs`) | Note |
|---|---|---|
| the registry of builtin names in Shared.pkl is the list of folders under pkl/Builtins | `the_registry_of_builtin_names_in_shared_pkl_is_the_list_of_folders_under_pkl_builtins` | covered |
| every builtin has an entry in the table of what leaving it out removes | `every_builtin_has_an_entry_in_the_table_of_what_leaving_it_out_removes` | covered |
| leaving a builtin out removes what it owns, changes only dependabot.yml and the files that read presence, and adds nothing | `leaving_a_builtin_out_removes_what_it_owns_changes_only_dependabot_yml_and_the_files_that_read_presence_and_adds_nothing` | covered |
| dependabot's exclude list follows the workflows that are rendered | `dependabots_exclude_list_follows_the_workflows_that_are_rendered` | covered |
| a name that is not a builtin is an error that lists the builtins, and so is an empty or a repeated name | `a_name_that_is_not_a_builtin_is_an_error_that_lists_the_builtins_and_so_is_an_empty_or_a_repeated_name` | covered |
| leaving out a builtin that another needs fails and names both, with the way out | `leaving_out_a_builtin_that_another_needs_fails_and_names_both_with_the_way_out` | covered |
| mise and hk are not hard requirements: a repository can supply mise.toml and hk.pkl itself | `mise_and_hk_are_not_hard_requirements_a_repository_can_supply_mise_toml_and_hk_pkl_itself` | covered |
| leaving every builtin out renders nothing and is not an error | `leaving_every_builtin_out_renders_nothing_and_is_not_an_error` | covered |
| a name that a toggle already removes is fine, and the render equals the toggle alone | `a_name_that_a_toggle_already_removes_is_fine_and_the_render_equals_the_toggle_alone` | covered |
| features = all with the Rust builtins left out still renders, with no placeholder left in any file | `features_all_with_the_rust_builtins_left_out_still_renders_with_no_placeholder_left_in_any_file` | covered |
| a Preset that amends a left-out builtin fails and says options.without may have left it out | `a_preset_that_amends_a_left_out_builtin_fails_and_says_options_without_may_have_left_it_out` | covered |
| a single builtin function still renders when its builtin is left out: without only affects the aggregators | `a_single_builtin_function_still_renders_when_its_builtin_is_left_out_without_only_affects_the_aggregators` | covered |
| in an adopted repository, leaving a builtin out deletes the files it rendered, and a seeded file stays | `in_an_adopted_repository_leaving_a_builtin_out_deletes_the_files_it_rendered_and_a_seeded_file_stays` | covered |
| in an adopted repository, a file of a left-out builtin that was edited by hand blocks the render | `in_an_adopted_repository_a_file_of_a_left_out_builtin_that_was_edited_by_hand_blocks_the_render` | covered |
| turning a builtin back on writes its files again | `turning_a_builtin_back_on_writes_its_files_again` | covered |

### presence-templates

| TypeScript test (`runner/test/presence-templates.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/presence_templates.rs`) | Note |
|---|---|---|
| the present mapping has one entry for every builtin folder, and the tools mapping one for every tool of the table | `the_present_mapping_has_one_entry_for_every_builtin_folder_and_the_tools_mapping_one_for_every_tool_of_the_table` | covered |
| every owner in the table is a builtin | `every_owner_in_the_table_is_a_builtin` | covered |
| a tool is present while any of its owners is, and gone when all are left out | `a_tool_is_present_while_any_of_its_owners_is_and_gone_when_all_are_left_out` | covered |
| pkl and the runner are always pinned | `pkl_and_the_runner_are_always_pinned` | covered |
| every tool a script names outside its comments is owned by that script's builtin | `every_tool_a_script_names_outside_its_comments_is_owned_by_that_scripts_builtin` | covered |
| .rumdl.toml names mise run lint only when mise is present, and keeps the pull request template ignore either way | `rumdl_toml_names_mise_run_lint_only_when_mise_is_present_and_keeps_the_pull_request_template_ignore_either_way` | covered |
| by default mise.toml pins the eleven tools and has the five tasks, the env and both hooks | `by_default_mise_toml_pins_the_eleven_tools_and_has_the_five_tasks_the_env_and_both_hooks` | covered |
| mise.toml with ${c.without.join(" and ")} left out pins ${c.tools.length} tools and keeps the tasks of what is present | `mise_toml_with_hk_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present`, `mise_toml_with_hk_and_inline_scripts_left_out_pins_9_tools_and_keeps_the_tasks_of_what_is_present`, `mise_toml_with_hk_and_actionlint_left_out_pins_8_tools_and_keeps_the_tasks_of_what_is_present`, `mise_toml_with_rumdl_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present`, `mise_toml_with_rust_ci_left_out_pins_11_tools_and_keeps_the_tasks_of_what_is_present`, `mise_toml_with_rust_ci_and_rust_toolchain_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present`, `mise_toml_with_zizmor_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present`, `mise_toml_with_agnix_left_out_pins_10_tools_and_keeps_the_tasks_of_what_is_present` | 8 TypeScript tests (a loop over eight cases), 8 Rust tests |
| jq goes only when all five of its owners are left out | `jq_goes_only_when_all_five_of_its_owners_are_left_out` | covered |
| the rust comment names check-toolchain-version.sh only with rust-ci, and the actionlint comment names zizmor only with zizmor | `the_rust_comment_names_check_toolchain_version_sh_only_with_rust_ci_and_the_actionlint_comment_names_zizmor_only_with_zizmor` | covered |
| real mise reads a reduced mise.toml and lists the tools that are left | `real_mise_reads_a_reduced_mise_toml_and_lists_the_tools_that_are_left` | covered |
| hk.shared.pkl with ${c.without.join(" and ") || "nothing"} left out has the steps ${c.steps.join(", ")}, and real hk accepts it with the seeded hk.pkl | `hk_shared_pkl_with_nothing_left_out_has_all_the_steps_and_real_hk_accepts_it_with_the_seeded_hk_pkl`, `hk_shared_pkl_with_rumdl_left_out_has_the_steps_without_rumdl_and_real_hk_accepts_it_with_the_seeded_hk_pkl`, `hk_shared_pkl_with_actionlint_left_out_has_the_steps_without_actionlint_and_real_hk_accepts_it_with_the_seeded_hk_pkl`, `hk_shared_pkl_with_rust_ci_left_out_has_the_steps_without_clippy_and_test_and_real_hk_accepts_it_with_the_seeded_hk_pkl`, `hk_shared_pkl_with_rust_ci_and_rust_toolchain_left_out_has_the_steps_without_the_rust_ones_and_real_hk_accepts_it_with_the_seeded_hk_pkl`, `hk_shared_pkl_with_rumdl_actionlint_rust_ci_and_rust_toolchain_left_out_has_two_steps_and_real_hk_accepts_it_with_the_seeded_hk_pkl` | 6 TypeScript tests (a loop over six cases), 6 Rust tests |
| the hk files say where tools are pinned only when mise is present | `the_hk_files_say_where_tools_are_pinned_only_when_mise_is_present` | covered |
| the pre-push comment names the rust-ci workflow only when rust-ci is present | `the_pre_push_comment_names_the_rust_ci_workflow_only_when_rust_ci_is_present` | covered |
| the zizmor workflow header runs zizmor through mise, and says it is pinned there, only when mise is present | `the_zizmor_workflow_header_runs_zizmor_through_mise_and_says_it_is_pinned_there_only_when_mise_is_present` | covered |

### release-builtins

| TypeScript test (`runner/test/release-builtins.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/release_builtins.rs`) | Note |
|---|---|---|
| the shape follows binary unless it is set, and each shape has its own header | `the_shape_follows_binary_unless_it_is_set_and_each_shape_has_its_own_header` | covered |
| the draft shape renders the publishing script, the others do not | `the_draft_shape_renders_the_publishing_script_the_others_do_not` | covered |
| the seeded release-please files are seeds, differ by shape, and are skipped by seedReleaseConfig = false | `the_seeded_release_please_files_are_seeds_differ_by_shape_and_are_skipped_by_seed_release_config_false` | covered |
| the seed for this repository's options is the committed release-please-config.json | `the_seed_for_this_repositorys_options_is_the_committed_release_please_config_json` | covered |
| release-automerge: manual by default with a notice when the schedule is off, today's schedule with always | `release_automerge_manual_by_default_with_a_notice_when_the_schedule_is_off_todays_schedule_with_always` | covered |
| release-bump renders only with versionFile, as a workflow and a script that name the file and the scope | `release_bump_renders_only_with_version_file_as_a_workflow_and_a_script_that_name_the_file_and_the_scope` | covered |
| the release options are refused, with a message that names what is wrong, whatever is rendered | `the_release_options_are_refused_with_a_message_that_names_what_is_wrong_whatever_is_rendered` | covered |
| release-bump waits for the workflow that the release shape names | `release_bump_waits_for_the_workflow_that_the_release_shape_names` | covered |

The three files had 50 TypeScript tests (presence 15, presence-templates 27, release-builtins 8) and are replaced by 50 Rust tests, one for each: the loops here are over fixed cases and are written out. The other 205 Rust tests of the crate are those of Phases 7a and 7b.

## Added in Phase 7d: the workflow and script tests

Written on 16-11-2026 for Phase 7d of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). These four TypeScript test files were ported to `scaffold-package-tests` and deleted. Every row was checked by script: the TypeScript title is the one in the deleted file (read from `origin/main`; a title built in a loop is the template of the loop), and each Rust test is in the list that `cargo test -- --list` prints for the crate. For a plain title the Rust name is the title in snake_case, derived by the script. `update-pins-workflow.test.ts` is not in this list: it waits for Phase 6b.

### release-workflow

| TypeScript test (`runner/test/release-workflow.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/release_workflow.rs`) | Note |
|---|---|---|
| it runs on a v-tag push or by hand, never on pull_request_target or a branch push | `it_runs_on_a_v_tag_push_or_by_hand_never_on_pull_request_target_or_a_branch_push` | covered |
| the manual run is a dry run unless told otherwise | `the_manual_run_is_a_dry_run_unless_told_otherwise` | covered |
| nothing has permissions by default and every job asks for its own | `nothing_has_permissions_by_default_and_every_job_asks_for_its_own` | covered |
| only the release job can write contents, attest and use the id token | `only_the_release_job_can_write_contents_attest_and_use_the_id_token` | covered |
| every action is pinned by a 40 character SHA | `every_action_is_pinned_by_a_40_character_sha` | covered |
| it uses no secrets and persists no checkout credentials | `it_uses_no_secrets_and_persists_no_checkout_credentials` | covered |
| no run script interpolates text a pusher controls: tags, refs and inputs go through env | `no_run_script_interpolates_text_a_pusher_controls_tags_refs_and_inputs_go_through_env` | covered |
| only the release job publishes, through publish-release.sh, and never on a dry run | `only_the_release_job_publishes_through_publish_release_sh_and_never_on_a_dry_run` | covered |
| publish-release.sh makes a pre-release, adopts only a draft for the tag, and never re-publishes | `publish_release_sh_makes_a_pre_release_adopts_only_a_draft_for_the_tag_and_never_re_publishes` | covered |
| the attestation is made only on a real release, and covers the assets | `the_attestation_is_made_only_on_a_real_release_and_covers_the_assets` | covered |
| the tag is checked against PklProject before anything is built | `the_tag_is_checked_against_pkl_project_before_anything_is_built` | covered |
| the release tool is built in a job that cannot write and holds no token, and prepare and release only download it | `the_release_tool_is_built_in_a_job_that_cannot_write_and_holds_no_token_and_prepare_and_release_only_download_it` | covered |
| no job that can write runs cargo | `no_job_that_can_write_runs_cargo` | covered |
| prepare and release have no job-wide token in their environment, so the tool runs without one | `prepare_and_release_have_no_job_wide_token_in_their_environment_so_the_tool_runs_without_one` | covered |
| a dry run also checks the tag this version would get, so check-tag is exercised before a real tag | `a_dry_run_also_checks_the_tag_this_version_would_get_so_check_tag_is_exercised_before_a_real_tag` | covered |
| the runner is built on a machine of each target, in a matrix, and a failure on one does not hide the others | `the_runner_is_built_on_a_machine_of_each_target_in_a_matrix_and_a_failure_on_one_does_not_hide_the_others` | covered |
| each build installs the pinned Rust toolchain, builds with the release tool, and uploads its own binary | `each_build_installs_the_pinned_rust_toolchain_builds_with_the_release_tool_and_uploads_its_own_binary` | covered |
| nothing cross-compiles, and the TypeScript build script is gone from the release | `nothing_cross_compiles_and_the_type_script_build_script_is_gone_from_the_release` | covered |
| ${label} installs zig through mise's rust-build environment and reads no package archive | `release_yml_build_installs_zig_through_mises_rust_build_environment_and_reads_no_package_archive`, `ci_yml_rust_binary_installs_zig_through_mises_rust_build_environment_and_reads_no_package_archive` | 2 TypeScript tests (a loop over 2 jobs), 2 Rust tests |
| no other job gets zig, so no other job depends on its download host | `no_other_job_gets_zig_so_no_other_job_depends_on_its_download_host` | covered |
| the rust-build environment pins zig to one exact version and locks it for the three platforms with a checksum each | `the_rust_build_environment_pins_zig_to_one_exact_version_and_locks_it_for_the_three_platforms_with_a_checksum_each` | covered |
| the package is built on its own and uploaded as package-files | `the_package_is_built_on_its_own_and_uploaded_as_package_files` | covered |
| smoke tests the binary built for its own target, and the release gets the package and all three binaries into one folder | `smoke_tests_the_binary_built_for_its_own_target_and_the_release_gets_the_package_and_all_three_binaries_into_one_folder` | covered |
| the release workflow and the pull request checks build the binary with the same script | `the_release_workflow_and_the_pull_request_checks_build_the_binary_with_the_same_script` | covered |
| the smoke job runs the Rust release tool built for its own target, and the TypeScript smoke suite is no longer run by the release | `the_smoke_job_runs_the_rust_release_tool_built_for_its_own_target_and_the_type_script_smoke_suite_is_no_longer_run_by_the_release` | covered |
| the build job builds the release tool before it builds the runner with it, and runs no TypeScript to do so | `the_build_job_builds_the_release_tool_before_it_builds_the_runner_with_it_and_runs_no_type_script_to_do_so` | covered |
| the pull request checks build the tool before the runner, and build and smoke-test the runner on every platform | `the_pull_request_checks_build_the_tool_before_the_runner_and_build_and_smoke_test_the_runner_on_every_platform` | covered |
| the release tool for the smoke job is built where nothing can write, and its artifact is kept out of the released files | `the_release_tool_for_the_smoke_job_is_built_where_nothing_can_write_and_its_artifact_is_kept_out_of_the_released_files` | covered |
| the pull request checks run the Rust smoke and baseline scripts, and no differential script of a deleted tool | `the_pull_request_checks_run_the_rust_smoke_and_baseline_scripts_and_no_differential_script_of_a_deleted_tool` | covered |
| the release is verified from the outside after it is published, on all three platforms | `the_release_is_verified_from_the_outside_after_it_is_published_on_all_three_platforms` | covered |
| two releases never run at once, and a running release is not cancelled | `two_releases_never_run_at_once_and_a_running_release_is_not_cancelled` | covered |
| the consumer the release is verified with limits the lock to the supported platforms and checks the assets mise chose | `the_consumer_the_release_is_verified_with_limits_the_lock_to_the_supported_platforms_and_checks_the_assets_mise_chose` | covered |
| verify-release renders the standard workflows and files through the published package, checks them, and renders again with nothing to write | `verify_release_renders_the_standard_workflows_and_files_through_the_published_package_checks_them_and_renders_again_with_nothing_to_write` | covered |
| verify-release imports Builtins from the same package URI, with the same checksum, as it amends Config | `verify_release_imports_builtins_from_the_same_package_uri_with_the_same_checksum_as_it_amends_config` | covered |
| the release notes start with the pre-release wording, link the Upgrading section, and keep the link when promoted | `the_release_notes_start_with_the_pre_release_wording_link_the_upgrading_section_and_keep_the_link_when_promoted` | covered |

### promote-workflow

| TypeScript test (`runner/test/promote-workflow.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/promote_workflow.rs`) | Note |
|---|---|---|
| it runs by hand only, never on a push, a pull request or pull_request_target | `it_runs_by_hand_only_never_on_a_push_a_pull_request_or_pull_request_target` | covered |
| it takes a tag and is a dry run unless told otherwise | `it_takes_a_tag_and_is_a_dry_run_unless_told_otherwise` | covered |
| nothing has permissions by default and every job asks for its own | `nothing_has_permissions_by_default_and_every_job_asks_for_its_own` | covered |
| only the promote job can write, and it writes only contents | `only_the_promote_job_can_write_and_it_writes_only_contents` | covered |
| the promote job sits behind the protected environment, and the dry-run job never touches it | `the_promote_job_sits_behind_the_protected_environment_and_the_dry_run_job_never_touches_it` | covered |
| the promote job runs only after the tool and guards jobs, and only when the run is not a dry run | `the_promote_job_runs_only_after_the_tool_and_guards_jobs_and_only_when_the_run_is_not_a_dry_run` | covered |
| the guards job checks with the release tool, and only the promote job can ask it to edit | `the_guards_job_checks_with_the_release_tool_and_only_the_promote_job_can_ask_it_to_edit` | covered |
| there is one tool: no input chooses it, no step compares two, and nothing installs bun or mise | `there_is_one_tool_no_input_chooses_it_no_step_compares_two_and_nothing_installs_bun_or_mise` | covered |
| the release tool is built in a job that cannot write and holds no token | `the_release_tool_is_built_in_a_job_that_cannot_write_and_holds_no_token` | covered |
| guards and promote download the release tool and check its checksum before they run it | `guards_and_promote_download_the_release_tool_and_check_its_checksum_before_they_run_it` | covered |
| no job that can write runs cargo | `no_job_that_can_write_runs_cargo` | covered |
| no job has a job-wide token in its environment: each step that needs one names it | `no_job_has_a_job_wide_token_in_its_environment_each_step_that_needs_one_names_it` | covered |
| every action is pinned by a 40 character SHA | `every_action_is_pinned_by_a_40_character_sha` | covered |
| it uses no secrets and persists no checkout credentials | `it_uses_no_secrets_and_persists_no_checkout_credentials` | covered |
| no run script interpolates text a dispatcher controls: the tag goes through env | `no_run_script_interpolates_text_a_dispatcher_controls_the_tag_goes_through_env` | covered |
| runs never overlap and are never cancelled half way | `runs_never_overlap_and_are_never_cancelled_half_way` | covered |

### publish-release

| TypeScript test (`runner/test/publish-release.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/publish_release.rs`) | Note |
|---|---|---|
| one draft for the tag: assets are uploaded to it and it is published as a pre-release by id | `one_draft_for_the_tag_assets_are_uploaded_to_it_and_it_is_published_as_a_pre_release_by_id` | covered |
| no release for the tag: it is created as a pre-release, as for a tag pushed by hand | `no_release_for_the_tag_it_is_created_as_a_pre_release_as_for_a_tag_pushed_by_hand` | covered |
| a published release for the tag stops it, and nothing is created or edited | `a_published_release_for_the_tag_stops_it_and_nothing_is_created_or_edited` | covered |
| two drafts for the tag stop it before anything is uploaded | `two_drafts_for_the_tag_stop_it_before_anything_is_uploaded` | covered |

### release-bump

| TypeScript test (`runner/test/release-bump.test.ts`) | Rust test (`crates/scaffold-package-tests/tests/release_bump.rs`) | Note |
|---|---|---|
| it runs only after the release workflow completed, and only for a successful tag push | `it_runs_only_after_the_release_workflow_completed_and_only_for_a_successful_tag_push` | covered |
| nothing has permissions by default; the job reads contents, and the App token can write only contents and pull requests | `nothing_has_permissions_by_default_the_job_reads_contents_and_the_app_token_can_write_only_contents_and_pull_requests` | covered |
| it checks out main without credentials, pins every action by SHA, and passes event text through env only | `it_checks_out_main_without_credentials_pins_every_action_by_sha_and_passes_event_text_through_env_only` | covered |
| v0.3.0 released and main still at 0.3.0: the next patch is put in a branch, a pull request is opened, and it is merged when checks pass | `v0_3_0_released_and_main_still_at_0_3_0_the_next_patch_is_put_in_a_branch_a_pull_request_is_opened_and_it_is_merged_when_checks_pass` | covered |
| the patch carries over two digits: v1.2.9 gives 1.2.10 | `the_patch_carries_over_two_digits_v1_2_9_gives_1_2_10` | covered |
| a failing check leaves the pull request open and fails the run | `a_failing_check_leaves_the_pull_request_open_and_fails_the_run` | covered |
| no check yet is not green: it waits, then gives up without merging | `no_check_yet_is_not_green_it_waits_then_gives_up_without_merging` | covered |
| main already above the released version: a visible notice and no branch | `main_already_above_the_released_version_a_visible_notice_and_no_branch` | covered |
| main below the released version, a tag that is not the run's commit, and a pre-release tag are each stopped or skipped without a branch | `main_below_the_released_version_a_tag_that_is_not_the_runs_commit_and_a_pre_release_tag_are_each_stopped_or_skipped_without_a_branch` | covered |
| an open pull request for the same branch is not duplicated | `an_open_pull_request_for_the_same_branch_is_not_duplicated` | covered |
| a version file with a space and regex characters in its path is read, fetched and written with the path encoded | `a_version_file_with_a_space_and_regex_characters_in_its_path_is_read_fetched_and_written_with_the_path_encoded` | covered |
| a version file with no version line, or with two, is refused before anything is created | `a_version_file_with_no_version_line_or_with_two_is_refused_before_anything_is_created` | covered |
| a failure after the branch is made and before the pull request is open deletes the branch again | `a_failure_after_the_branch_is_made_and_before_the_pull_request_is_open_deletes_the_branch_again` | covered |
| a failure after the pull request is open leaves the branch alone | `a_failure_after_the_pull_request_is_open_leaves_the_branch_alone` | covered |

The four files had 70 TypeScript tests (release-workflow 36, promote-workflow 16, publish-release 4, release-bump 14) and are replaced by 70 Rust tests, one for each. The other 255 Rust tests of the crate are those of Phases 7a to 7c.

## Phase 7e: the module tests with a Rust twin are deleted

Written on 16-11-2026 for Phase 7e of [ts-tooling-rust-port.md](ts-tooling-rust-port.md). The TypeScript test files of `contract`, `equality`, `manifest`, `preflight` and `schema` (80 tests) and their modules were deleted. The evidence is the tables of Phase 0 above. Before deleting, a script checked that every one of the 80 TypeScript titles is in those tables, and that the Rust test named by each row exists in the list that `cargo test --workspace -- --list` prints: equality 21 of 21, preflight 22 of 22, schema 10 of 10, manifest 10 of 11 and contract 15 of 16. The two rows with no Rust test are the ones Phase 0 recorded as "not applicable": the export of an error type, in `manifest` and in `contract`, which is a property of the TypeScript module system and not a behaviour.

`semver` and `freshness`, with their tests, stay until Phase 6b: `pinupdate.ts` imports both. `helpers.ts` is cut down to the four functions the update-pins script test uses.

### Phase 7e, second part: the fixtures move

On 16-11-2026 `runner/test/fixtures` (37 folders) and `runner/test/upstream` (44 files) moved to `crates/scaffold-package-tests/fixtures` and `upstream` with `git mv`, so their history is kept. Every Rust call site, the `actionlint` step of `ci.yml` and the docs that name them read the new paths. The pkl fixtures amend `../../../../pkl/Config.pkl`, which is the same four levels from the root at the new depth. `scaffold-release baseline snapshot` reads the new folder and falls back to `runner/test/fixtures` for a tree from before the move; a test covers both. The retired work in `archive/` and the corpus `crates/scaffold-core/tests/data/yaml-corpus.json` (a snapshot of text) still name the old paths and were left as they were.

### The baseline moves out of `runner/`

On 16-11-2026 `runner/scripts/builtins-baseline/baseline.json` and `README.md` moved to `crates/scaffold-release/baseline/` with `git mv`, before Phase 8, so that deleting `runner/` cannot delete the frozen snapshot that the `baseline` job of `ci.yml` compares against. The blob hash of `baseline.json` is the same before and after (`beaf9f2`). The `ci.yml` job, the workflow test, the hygiene test and the docs read the new path.

## Phase 6b: the update-pins tests and the differential scenarios

Written on 16-11-2026 for Phase 6b of [ts-tooling-rust-port.md](ts-tooling-rust-port.md), before the TypeScript is deleted. `semver` (5 tests) and `freshness` (9 tests) have their rows above and are covered. These are the rows of the two files that had none, and of the 30 scenarios of `update-pins-diff.sh`, the only byte for byte check of the output, the files and the requests of the two tools. Three Rust tests were added for gaps this mapping found: `a_pins_file_with_no_pins_checks_zero_and_still_writes_the_record`, `a_move_to_a_pre_release_tag_with_a_dot_in_it_is_written` and `the_tag_of_an_up_to_date_pin_that_cannot_be_found_is_a_problem`, all in `crates/scaffold-release/src/update_pins/tests.rs`.

### pinupdate

| TypeScript test (`runner/test/pinupdate.test.ts`) | Rust test | Note |
|---|---|---|
| every pin line is read, with the repository part of a workflow path | `every_pin_line_is_read_with_the_repository_part_of_a_workflow_path` | covered |
| a pin line that cannot be read is an error, never skipped | `a_pin_line_that_cannot_be_read_is_an_error_never_skipped` | covered |
| the real Pins.pkl is read completely | `the_real_pins_pkl_is_read_completely` | covered |
| pins that are already at the latest release and match their tag change nothing and raise no problem | `pins_already_at_the_latest_release_and_matching_their_tag_change_nothing` | covered |
| a pin behind its latest release is updated to that release's commit, and a major bump is flagged | `a_pin_behind_its_latest_release_moves_to_that_commit_and_a_major_bump_is_flagged` | covered |
| a pin is never moved to an older release | `a_pin_is_never_moved_to_an_older_release` | covered |
| a pin whose commit is not the commit of its recorded tag is a problem even when it is up to date | `a_pin_whose_commit_is_not_the_commit_of_its_recorded_tag_is_a_problem_even_when_up_to_date` | covered |
| a latest tag that is not a published release is a problem and nothing is changed for it | `a_latest_tag_that_is_not_a_published_release_is_a_problem_and_nothing_changes_for_it` | covered |
| a commit that does not exist in the upstream repository is a problem | `a_commit_that_does_not_exist_in_the_upstream_repository_is_a_problem` | covered |
| a repository with no release, and a GitHub error, are problems and never read as unchanged | `a_repository_with_no_release_and_a_github_error_are_problems_never_unchanged` | covered |
| applying changes rewrites only the pin lines that changed, and applying them twice changes nothing more | `applying_changes_rewrites_only_the_pin_lines_that_changed_and_twice_changes_nothing_more` | covered |
| applying a change to a pin that is not in the file is an error | `applying_a_change_to_a_pin_that_is_not_in_the_file_is_an_error` | covered |
| the freshness record the update writes is one the freshness check accepts | `the_record_the_update_writes_is_one_the_check_accepts` (in `scaffold-core`) | covered |

### update-pins-script

| TypeScript test (`runner/test/update-pins-script.test.ts`) | Rust test | Note |
|---|---|---|
| a no-write run lists the changes and writes nothing | `a_no_write_run_lists_the_changes_and_writes_nothing` | covered |
| a real run rewrites the changed pin, writes the freshness record and a summary for the pull request | `a_real_run_rewrites_the_changed_pin_and_writes_the_freshness_record_and_a_summary` | covered |
| when nothing changed the freshness record is still written, and no summary is | `when_nothing_changed_the_freshness_record_is_still_written_and_no_summary_is` | covered |
| a problem fails the run and writes nothing, not even the freshness record | `a_problem_fails_the_run_and_writes_nothing_not_even_the_freshness_record` | covered |
| a major bump is marked in the summary | `a_major_bump_is_marked_in_the_summary_and_in_the_listing` | covered |
| an unknown option is a usage error | `the_options_are_read_and_an_unknown_one_or_a_missing_value_is_a_usage_error` | covered |

### The 30 scenarios of `update-pins-diff.sh`

| Scenario | Rust test | Note |
|---|---|---|
| every pin is current: the record is written, the pins are not | `when_nothing_changed_the_freshness_record_is_still_written_and_no_summary_is` | covered |
| the same with --no-write: nothing is written | `a_no_write_run_lists_the_changes_and_writes_nothing` | covered with pins that move; the run with every pin current and `--no-write` is the two tests together |
| a minor move is written with the record and the summary | `a_real_run_rewrites_the_changed_pin_and_writes_the_freshness_record_and_a_summary` | covered |
| a minor move with --no-write: listed, nothing written | `a_no_write_run_lists_the_changes_and_writes_nothing` | covered |
| a major move is flagged in the listing and the summary | `a_major_bump_is_marked_in_the_summary_and_in_the_listing` | covered |
| a move to a pre-release tag with a dot in it | `a_move_to_a_pre_release_tag_with_a_dot_in_it_is_written` | covered (added in 6b) |
| the latest tag is not a published release | `a_latest_tag_that_is_not_a_published_release_is_a_problem_and_nothing_changes_for_it` | covered |
| the latest release is a draft | `a_release_is_published_only_when_it_exists_and_is_not_a_draft` | covered (HTTP layer) |
| the commit does not belong to the repository | `a_commit_that_does_not_exist_in_the_upstream_repository_is_a_problem` | covered |
| a repository with no release | `a_repository_with_no_release_and_a_github_error_are_problems_never_unchanged` | covered |
| a 503 is retried and the next answer is used | `a_5xx_is_retried_up_to_three_attempts_and_a_404_is_an_answer_that_is_not` | covered |
| a 502 that never goes away fails after three attempts and writes nothing | `a_5xx_is_retried_up_to_three_attempts_and_a_404_is_an_answer_that_is_not`, `a_problem_fails_the_run_and_writes_nothing_not_even_the_freshness_record` | covered (two tests) |
| a 403 is not retried and is a problem | `the_latest_release_is_its_tag_and_a_repository_with_none_is_none` | covered (HTTP layer: one 403 answer, one request) |
| the tag of an up to date pin cannot be found | `the_tag_of_an_up_to_date_pin_that_cannot_be_found_is_a_problem` | covered (added in 6b) |
| an up to date pin whose commit is not its tag's commit | `a_pin_whose_commit_is_not_the_commit_of_its_recorded_tag_is_a_problem_even_when_up_to_date` | covered |
| the latest release is older than the pin: nothing moves | `a_pin_is_never_moved_to_an_older_release` | covered |
| an annotated tag is followed to its commit | `a_tag_resolves_to_its_commit_and_an_annotated_tag_is_followed_to_it` | covered |
| a tag that points to a tree | `a_tag_resolves_to_its_commit_and_an_annotated_tag_is_followed_to_it` | covered (the v3.0.0 case of that test) |
| the latest tag is not a version | `a_version_that_is_not_a_version_is_a_problem_and_the_update_does_not_guess` | covered |
| the pinned version is not a version | `a_version_that_is_not_a_version_is_a_problem_and_the_update_does_not_guess` | covered |
| one pin moves and another has a problem: nothing is written | `a_problem_fails_the_run_and_writes_nothing_not_even_the_freshness_record` | covered |
| an action with a workflow path in its name uses the repository part | `every_pin_line_is_read_with_the_repository_part_of_a_workflow_path` | covered |
| a Pins.pkl with no pins checks zero and writes the record | `a_pins_file_with_no_pins_checks_zero_and_still_writes_the_record` | covered (added in 6b) |
| no token at all: no Authorization header is sent | `the_token_is_github_token_then_gh_token_and_an_empty_one_is_no_token`, `a_request_carries_the_accept_header_and_the_token_when_there_is_one` | covered |
| GH_TOKEN alone is used | `the_token_is_github_token_then_gh_token_and_an_empty_one_is_no_token` | covered |
| an empty GITHUB_TOKEN ends the search: no header, and GH_TOKEN is not tried | `the_token_is_github_token_then_gh_token_and_an_empty_one_is_no_token` | covered |
| both set: GITHUB_TOKEN is used | `the_token_is_github_token_then_gh_token_and_an_empty_one_is_no_token` | covered |
| usage: an unknown option (exit code only) | `the_options_are_read_and_an_unknown_one_or_a_missing_value_is_a_usage_error`, `the_command_reports_a_usage_error_with_exit_2` | covered |
| usage: --summary with no value (exit code only) | `the_options_are_read_and_an_unknown_one_or_a_missing_value_is_a_usage_error` | covered |
| a line of Pins.pkl that cannot be read (exit code only) | `a_pins_file_that_is_missing_or_has_a_line_that_cannot_be_read_exits_1` | covered |

The last differential output before deletion, from the same commit, was `30 comparisons, 0 differ`. The commit that still holds the TypeScript tool is f6a24a6 on `origin/main`. What the differential script proved at the byte level for the requests (order, Accept and Authorization headers) is covered in Rust by `a_request_carries_the_accept_header_and_the_token_when_there_is_one` and `the_whole_update_runs_against_a_server_standing_in_for_github`; the TypeScript tool has no other reference once it is deleted.

### update-pins-workflow

`runner/test/update-pins-workflow.test.ts` (17 tests) is ported to `crates/scaffold-package-tests/tests/update_pins_workflow.rs` (18 tests), written against `update-pins.yml` as Phase 6b leaves it: one tool, no `implementation` input, no mise or bun step. Fourteen titles are ported (five of them adjusted because the TypeScript write step is gone), three are rewritten and one test is new.

| TypeScript test | Rust test | Note |
|---|---|---|
| it runs on a schedule or by hand, and on no push: merging a change to the tool must not start a real run | `it_runs_on_a_schedule_or_by_hand_and_on_no_push_merging_a_change_to_the_tool_must_not_start_a_real_run` | covered |
| a manual run is a dry run unless told otherwise, and says which tool writes in a real run, the Rust one by default | `a_manual_run_is_a_dry_run_unless_told_otherwise_and_dry_run_is_its_only_input` | rewritten: there is one tool, so the `implementation` choice is gone and `dry_run` is the only input |
| nothing has permissions by default and every job reads contents only: writes use the App token | `nothing_has_permissions_by_default_and_every_job_reads_contents_only_writes_use_the_app_token` | covered |
| the only secret is the App key, and the default token is never used to write | `the_only_secret_is_the_app_key_and_the_default_token_is_never_used_to_write` | covered |
| every action is pinned by a 40 character SHA and checkout keeps no credentials | `every_action_is_pinned_by_a_40_character_sha_and_checkout_keeps_no_credentials` | covered |
| no run script interpolates an expression, and each is 15 lines or fewer | `no_run_script_interpolates_an_expression_and_each_is_15_lines_or_fewer` | covered |
| the freshness record is written only after the pins were checked and the pull request step ran | `the_freshness_record_is_written_only_after_the_pins_were_checked_and_the_pull_request_step_ran` | adjusted: the TypeScript write step is not in the order |
| each writing step asks for the record and the summary, and a failing check fails the job | `the_writing_step_asks_for_the_record_and_the_summary_and_a_failing_check_fails_the_job` | adjusted: one writing step |
| the record goes to the freshness branch, never main | `the_record_goes_to_the_freshness_branch_never_main` | covered |
| the pull request step does nothing when no pin changed | `the_pull_request_step_does_nothing_when_no_pin_changed` | covered |
| the pull request is a conventional commit and is opened with the App token | `the_pull_request_is_a_conventional_commit_and_is_opened_with_the_app_token` | covered |
| the release tool is built in a job that holds no token | `the_release_tool_is_built_in_a_job_that_holds_no_token` | covered |
| the update job, which holds the App token, never runs cargo and downloads the tool and checks its checksum before it runs it | `the_update_job_which_holds_the_app_token_never_runs_cargo_and_downloads_the_tool_and_checks_its_checksum_before_it_runs_it` | covered, and stronger: the checksum step is pinned exactly and no step before it runs the tool (see breakages A6 and C4) |
| no job has a job-wide token in its environment: each step that needs one names it | `no_job_has_a_job_wide_token_in_its_environment_each_step_that_needs_one_names_it` | covered |
| a dry run reaches nothing that writes: no App token, no write step, no pull request, no freshness record | `a_dry_run_reaches_nothing_that_writes_no_app_token_no_write_step_no_pull_request_no_freshness_record` | adjusted: the TypeScript write step is not among the writers; the conditions are exact strings |
| a dry run runs both tools and fails when they disagree, whatever the Rust one says | `a_dry_run_runs_the_one_tool_with_no_write_and_the_step_fails_with_the_tool` | rewritten: there is no second tool to compare with. The step is exactly `tool/scaffold-release update-pins --no-write` with the read-only token |
| a real run uses the Rust tool unless asked for the TypeScript one, with the App token, and mise is installed only for the TypeScript tool | `a_real_run_uses_the_one_tool_with_the_app_token_and_no_step_installs_mise` | rewritten: the real run has the `NOT_DRY` condition alone, the App token, and no step installs mise |
| (new in 6b) | `there_is_one_tool_nothing_chooses_it_and_nothing_names_a_second_one_or_bun` | new: the workflow names no `implementation`, `update-pins.ts`, `bun`, `mise`, `typescript` or `disagree` |

#### Breakage comparison of the workflow test

On 16-11-2026, in plain clones, a deliberate breakage of `update-pins.yml` was applied to the branch (Rust tests) and, with the same text where the line is the same in both files, to a clone of `origin/main` at f6a24a6 (TypeScript tests). The failing tests of the two suites were paired by the table above. Both trees were left clean.

Group A, breakages that are the same in both files (18 runs):

| Breakage | TypeScript failing | Rust failing | Result |
|---|---|---|---|
| the mint condition loses its `!` | 15 | 15 | identical |
| the mint step has no `if` | 15 | 15 | identical |
| workflow permissions are `write-all` | 3 | 3 | identical |
| the tool job may write contents | 3, 12 | 3, 12 | identical |
| checkout keeps credentials | 5 | 5 | identical |
| the record goes to `main` | 7, 9, 15 | 7, 9, 15 | identical |
| the pull request step has no `if` | 15 | 15 | identical |
| the freshness step has no `if` | 15 | 15 | identical |
| the dry run uses the default secret token | 4, 15 | 4, 15 and the rewritten 16 | identical on the ported tests |
| a push trigger is added | 1 | 1 | identical |
| the update job has a job-wide token | 14 | 14 | identical |
| the writing step ignores a failure | 8 | 8 | identical |
| the pull request is not a conventional commit | 11 | 11 | identical |
| a run script interpolates an expression | 6 | 6 and the rewritten 16 | identical on the ported tests |
| the pull request step no longer checks for a change | 10 | 10 | identical |
| the update job runs cargo | 13 | 13 | identical |
| the update job does not wait for the tool job | 13 | 13 | identical |
| **the checksum check ends in `\|\| true`** | none | 13 | **different: neither TypeScript test nor the first Rust version caught it. The Rust test now pins the step exactly** |
| **the update job runs the tool before it checks it** | none | 13 | **different: same gap; the Rust test now fails if any step before the check runs the tool** |

The first run found three tests that failed in no breakage (the interpolation, pull request step and update job tests) and one breakage that neither suite caught (the checksum). A second round added the interpolation, no-check and cargo breakages and a tightened assertion. After the second round every one of the 18 Rust tests fails in at least one breakage.

Group B, breakages on the branch only, for the rewritten and new tests: an `implementation` input back (fails the input test and the new test), a mise step back (the real run test and the new test), `--no-write` dropped from the dry run (the dry run tests), the App token given to the dry run (the dry run tests), the real run depending on an `implementation` input (the dry run reach test, the real run test and the new test), the dry run condition loosened (the dry run reach test) and bun named in a comment (the new test). Each fails what it should.

The script was `breakage-update-pins.ts` (TypeScript on bun) in the session scratchpad; the breakages are the one-line `perl` substitutions named in the table.
