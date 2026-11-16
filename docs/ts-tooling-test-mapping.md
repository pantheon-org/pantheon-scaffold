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
