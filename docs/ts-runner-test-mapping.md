# The TypeScript runner's tests, and where each one went

Written on 13-11-2026 for pull request A of the minijinja goal (retire the TypeScript rendering runner). A test of a module is deleted only when the
module is deleted, and only when the Rust runner covers what it asserted. This page is the evidence, and the merge gate: one row for every deleted test.

## What stays in TypeScript, and why

Bun still runs the release tooling, so these stay, with the `runner/src` modules they import and those modules' tests:

| Kept | Used by |
|---|---|
| `runner/scripts/release-assets.ts`, `promote.ts`, `update-pins.ts`, `build-rust.ts`, `smoke.ts` | `release.yml`, `promote.yml`, `update-pins.yml`, `ci.yml` (the Rust build and its smoke test) |
| `runner/scripts/builtins-baseline/*` | the comparison of a render with the baseline |
| `runner/src`: `release`, `semver`, `checksums`, `promote`, `knownbad`, `preflight`, `manifest`, `contract`, `schema`, `pinupdate`, `freshness` | imported by the scripts above (`freshness` by the pin update test, which checks the record it writes) |
| `runner/test/fixtures/**` | the Rust BDD crate, `pkl.rs`, `snapshot.ts` |
| `runner/package.json` | bun |
| `equality.ts`, moved to `runner/test/support/` | the package tests that compare a render with the pinned upstream workflows; it is a test helper, not the runner |

Deleted modules: `apply`, `check`, `cli`, `detach`, `guard`, `latest`, `lock`, `lockcheck`, `paths`, `pins`, `pkl`, `upgrade`, `version`, `versions`. Deleted scripts:
`build.ts` (the TypeScript binary), `builtin-differential.ts` (TypeScript against Rust), `yaml-corpus.ts` and `coverage-estimate.ts` (the corpus in
`crates/scaffold-core/tests/data/yaml-corpus.json` stays as frozen test data).

## Unit tests, by module

Every deleted module was ported to Rust with a header that says so (`Ported from runner/src/<module>.ts`), and the Rust module has at least as many tests.

| Module | TypeScript tests | Rust tests (`crates/scaffold-core/src/<module>.rs`) |
|---|---|---|
| apply | 51 | 64 |
| check | 8 | 11 |
| detach | 10 | 14 |
| guard | 10 | 17 |
| latest | 11 | 14 |
| lock | 10 | 15 |
| lockcheck | 7 | 14 |
| paths | 14 | 15 |
| pins | 20 | 30 |
| upgrade | 26 | 55 |
| versions | 12 | 17 |
| pkl | 9 | 19 |

## Rows that needed a decision

The match column is a word-overlap score between the TypeScript test name and the closest Rust test or scenario; below 0.30 a person read both. These 24 were read (some rows share a decision):

| TypeScript test | Decision |
|---|---|
| `apply`: an unreadable existing file does not crash planning | The test only adopts a readable file (the name is misleading). Covered by `a_readable_existing_file_is_adopted`. |
| `lock`: releasing twice is harmless | Not expressible in Rust: `release(self)` consumes the lock. |
| `pkl`: a 404 from a just-created release is retried; a pkl error that is not a fetch problem is not retried; one attempt means no retry | `download_and_network_problems_look_like_fetch_failures`, `any_other_error_is_returned_at_once`, `a_fetch_failure_is_retried_then_fails_closed`. |
| `pkl`: a cache directory is passed as `--cache-dir`; without one pkl keeps its default | `the_pkl_options_come_from_the_flags` (lib.rs) and `the_evaluation_gets_the_cache_folder_and_the_local_package_folders_of_the_command` (tests/upgrade.rs). |
| `cli`: contract summarises a valid fixture | `contract evaluates a config and reports what it holds` (cli.feature). |
| `cli`: contract fails with exit 1 on a refused read; a local package cannot be read without `--allow-local-package` | **Ported:** the scenario `a config that reads a local package is refused without --allow-local-package` (cli.feature). |
| `cli`: render writes the slice with modes and a manifest | `render writes the files of the contract` and `the executable bit is set from the contract` (render.feature). |
| `cli`: the rendered workflows carry version comments and equal the pinned upstream copies | **Ported:** `runner/test/package-render.test.ts` renders through the Rust binary and compares with the pinned upstream workflows. |
| `cli`: check on an empty repo reports every owned file as missing | **Ported:** the scenario `check on an empty repository reports every owned file as missing` (render.feature). |
| `cli`: `--dry-run` takes no lock and a check runs while the lock is held | **Ported:** the scenario `a dry run takes no lock, and a check runs while the lock is held` (safety.feature). |
| `cli`: the usage text names the exit codes | `USAGE` in `crates/scaffold-cli/src/lib.rs` holds the codes; `a_bad_option_prints_the_message_and_the_usage`. |
| `cli-lifecycle`: a runner older than the package requires fails with exit 1 | `the_gate_stops_with_an_error_for_a_runner_that_is_too_old_or_a_bad_package_uri`. |
| `cli-lifecycle`: an unreachable known-bad list falls back to the bundled snapshot | `when_the_list_cannot_be_fetched_the_cached_copy_is_used_and_the_run_goes_on` and `the_gate_passes_the_package_version_on_and_falls_back_when_the_known_bad_list_cannot_be_fetched`. |
| `cli-lifecycle`: pkl gets the package cache under the repository by default | `the_pkl_options_come_from_the_flags`. |
| `cli-lifecycle`: `--repair` rebuilds an unreadable manifest | `--repair rebuilds a manifest that is not JSON` (lifecycle.feature) and `a_disabled_security_builtin_is_a_warning_and_repair_does_not_read_the_manifest`. |
| `cli-lifecycle`: a package URI that carries the pinned checksum renders and records the version | `runner/test/package-uri.test.ts` (kept) renders through a package URI with the checksum, and `preflight.rs` covers the URI parsing. |
| `cli-freshness`: a fresh record exits 0; a stale record exits 3; a record that is not valid is exit 1; a limit that is not a whole number is a usage error | The freshness scenarios of releases.feature (a recent record is fresh, older than thirty days is stale, a record that is not JSON is an error) and `a_number_option_needs_a_whole_number_of_at_least_one`. |

## Every deleted test

| TypeScript test file | Test | Closest Rust test or scenario | Match |
|---|---|---|---|
| `apply.test.ts` | a fresh run creates owned files with the contract mode and writes the manifest last | `scaffold-core/src/apply.rs`: a_fresh_run_creates_owned_files_with_the_contract_mode_and_writes_the_manifest_l | 1.00 |
| `apply.test.ts` | a second run changes nothing | `scaffold-core/src/apply.rs`: a_second_run_changes_nothing | 1.00 |
| `apply.test.ts` | no stage files are left behind | `scaffold-core/src/apply.rs`: no_stage_files_are_left_behind | 1.00 |
| `apply.test.ts` | an unmodified owned file is updated when the render changes | `scaffold-core/src/apply.rs`: an_unmodified_owned_file_is_updated_when_the_render_changes | 1.00 |
| `apply.test.ts` | a hand-edited owned file is refused and nothing else is written | `scaffold-core/src/apply.rs`: a_hand_edited_owned_file_is_refused_and_nothing_else_is_written | 1.00 |
| `apply.test.ts` | --force overwrites a hand-edited owned file | `scaffold-core/src/apply.rs`: force_overwrites_a_hand_edited_owned_file | 1.00 |
| `apply.test.ts` | first adoption: an identical existing file is adopted, not rewritten | `scaffold-core/src/apply.rs`: first_adoption_an_identical_existing_file_is_adopted_not_rewritten | 1.00 |
| `apply.test.ts` | first adoption: a differing existing file is refused with a diff | `scaffold-core/src/apply.rs`: first_adoption_a_differing_existing_file_is_refused_with_a_diff | 1.00 |
| `apply.test.ts` | first adoption: --force overwrites a differing existing file | `scaffold-core/src/apply.rs`: first_adoption_force_overwrites_a_differing_existing_file | 1.00 |
| `apply.test.ts` | seed: written once, then left alone even when the render changes | `scaffold-core/src/apply.rs`: seed_is_written_once_then_left_alone_even_when_the_render_changes | 1.00 |
| `apply.test.ts` | seed to owned is refused for an edited seed | `scaffold-core/src/apply.rs`: seed_to_owned_is_refused_for_an_edited_seed | 1.00 |
| `apply.test.ts` | seed to owned is allowed for an untouched seed | `scaffold-core/src/apply.rs`: seed_to_owned_is_allowed_for_an_untouched_seed | 1.00 |
| `apply.test.ts` | owned to seed leaves the file and records it as a seed | `scaffold-core/src/apply.rs`: owned_to_seed_leaves_the_file_and_records_it_as_a_seed | 1.00 |
| `apply.test.ts` | an external file is listed, never written, hashed or touched | `scaffold-core/src/apply.rs`: an_external_file_is_listed_never_written_hashed_or_touched | 1.00 |
| `apply.test.ts` | --dry-run plans and writes nothing | `scaffold-core/src/apply.rs`: dry_run_plans_and_writes_nothing | 1.00 |
| `apply.test.ts` | a path outside the repository stops the run before anything is written | `scaffold-core/src/apply.rs`: a_path_outside_the_repository_stops_the_run_before_anything_is_written | 1.00 |
| `apply.test.ts` | paths that differ only by case stop the run | `scaffold-core/src/apply.rs`: paths_that_differ_only_by_case_stop_the_run | 1.00 |
| `apply.test.ts` | a write through a symlink that leaves the repository is refused | `scaffold-core/src/apply.rs`: a_write_through_a_symlink_that_leaves_the_repository_is_refused | 1.00 |
| `apply.test.ts` | --repair rebuilds the manifest from files equal to the render and never adopts a differing file | `scaffold-core/src/apply.rs`: repair_rebuilds_the_manifest_from_files_equal_to_the_render_and_never_adopts_a_d | 1.00 |
| `apply.test.ts` | --repair after a broken manifest lets a normal run continue | `scaffold-core/src/apply.rs`: repair_after_a_broken_manifest_lets_a_normal_run_continue | 1.00 |
| `apply.test.ts` | an unreadable existing file does not crash planning of the others | `scaffold-core/src/apply.rs`: a_readable_existing_file_is_adopted | 0.25 **REVIEW** |
| `apply.test.ts` | shortDiff shows the first differing lines only | `scaffold-core/src/render.rs`: short_diff_shows_the_first_differing_lines | 0.50 |
| `apply.test.ts` | pinned uses lines get their version comment before hashing, and a second run is a no-op | `scaffold-core/src/apply.rs`: pinned_uses_lines_get_their_version_comment_before_hashing_and_a_second_run_is_a | 1.00 |
| `apply.test.ts` | a pinned uses line with no known version stops the run | `scaffold-core/src/apply.rs`: a_pinned_uses_line_with_no_known_version_stops_the_run | 1.00 |
| `apply.test.ts` | version comments are not added to files outside workflows | `scaffold-core/src/apply.rs`: version_comments_are_not_added_to_files_outside_workflows | 1.00 |
| `apply.test.ts` | a file dropped from the contract is deleted when the tree is clean and the file is tracked | `scaffold-core/src/apply.rs`: a_file_dropped_from_the_contract_is_deleted_when_the_tree_is_clean_and_the_file_ | 1.00 |
| `apply.test.ts` | --dry-run shows the deletion and removes nothing | `scaffold-core/src/apply.rs`: dry_run_shows_the_deletion_and_removes_nothing | 1.00 |
| `apply.test.ts` | a dirty tracked tree refuses the deletion and nothing else is written | `scaffold-core/src/apply.rs`: a_dirty_tracked_tree_refuses_the_deletion_and_nothing_else_is_written | 1.00 |
| `apply.test.ts` | --dry-run reports a guard refusal too | `scaffold-core/src/apply.rs`: dry_run_reports_a_guard_refusal_too | 1.00 |
| `apply.test.ts` | an untracked orphan is never deleted | `scaffold-core/src/apply.rs`: an_untracked_orphan_is_never_deleted | 1.00 |
| `apply.test.ts` | an orphan edited by hand and committed is refused, and --force deletes it | `scaffold-core/src/apply.rs`: an_orphan_edited_by_hand_and_committed_is_refused_and_force_deletes_it | 1.00 |
| `apply.test.ts` | an orphan that is already gone is simply dropped from the manifest | `scaffold-core/src/apply.rs`: an_orphan_that_is_already_gone_is_simply_dropped_from_the_manifest | 1.00 |
| `apply.test.ts` | a seed or external file dropped from the contract is left alone and dropped from the manifest | `scaffold-core/src/apply.rs`: a_seed_or_external_file_dropped_from_the_contract_is_left_alone_and_dropped_from | 1.00 |
| `apply.test.ts` | without orphans git is not needed at all | `scaffold-core/src/apply.rs`: without_orphans_git_is_not_needed_at_all | 1.00 |
| `apply.test.ts` | an orphan in a directory that is not a git repository is refused | `scaffold-core/src/apply.rs`: an_orphan_in_a_directory_that_is_not_a_git_repository_is_refused | 1.00 |
| `apply.test.ts` | a file outside the repo cannot be deleted through a path in the manifest | `scaffold-core/src/apply.rs`: a_file_outside_the_repo_cannot_be_deleted_through_a_path_in_the_manifest | 1.00 |
| `apply.test.ts` | the exec bit is set when the manifest's contract says so and the content is unchanged | `scaffold-core/src/apply.rs`: the_exec_bit_is_set_when_the_contract_says_so_and_the_content_is_unchanged | 0.89 |
| `apply.test.ts` | the exec bit is cleared when the contract drops it | `scaffold-core/src/apply.rs`: the_exec_bit_is_cleared_when_the_contract_drops_it | 1.00 |
| `apply.test.ts` | an exec bit removed by hand is restored on the next render | `scaffold-core/src/apply.rs`: an_exec_bit_removed_by_hand_is_restored_on_the_next_render | 1.00 |
| `apply.test.ts` | --dry-run reports the mode change and does not make it | `scaffold-core/src/apply.rs`: dry_run_reports_the_mode_change_and_does_not_make_it | 1.00 |
| `apply.test.ts` | a seed file keeps the mode the user gave it | `scaffold-core/src/apply.rs`: a_seed_file_keeps_the_mode_the_user_gave_it | 1.00 |
| `apply.test.ts` | a render refuses to run while another run holds the lock, and writes nothing | `scaffold-core/src/apply.rs`: a_render_refuses_to_run_while_another_run_holds_the_lock_and_writes_nothing | 1.00 |
| `apply.test.ts` | the lock is released after a run and after a refusal | `scaffold-core/src/apply.rs`: the_lock_is_released_after_a_run_and_after_a_refusal | 1.00 |
| `apply.test.ts` | --dry-run does not take the lock | `scaffold-core/src/apply.rs`: dry_run_does_not_take_the_lock | 1.00 |
| `apply.test.ts` | the package version is recorded in the manifest and kept when a later run has none | `scaffold-core/src/apply.rs`: the_package_version_is_recorded_in_the_manifest_and_kept_when_a_later_run_has_no | 1.00 |
| `apply.test.ts` | the detach flag of an owned file is recorded in the manifest | `scaffold-core/src/apply.rs`: the_detach_flag_of_an_owned_file_is_recorded_in_the_manifest | 1.00 |
| `apply.test.ts` | --repair keeps the package version and the detach flag | `scaffold-core/src/apply.rs`: repair_keeps_the_package_version_and_the_detach_flag | 1.00 |
| `apply.test.ts` | a case-only rename is refused, with nothing written and the old file intact | `scaffold-core/src/apply.rs`: a_case_only_rename_is_refused_with_nothing_written_and_the_old_file_intact | 1.00 |
| `apply.test.ts` | --force does not override a case-only rename | `scaffold-core/src/apply.rs`: force_does_not_override_a_case_only_rename | 1.00 |
| `apply.test.ts` | --dry-run reports a case-only rename as a refusal too | `scaffold-core/src/apply.rs`: dry_run_reports_a_case_only_rename_as_a_refusal_too | 1.00 |
| `apply.test.ts` | a rename to a different name is still an ordinary orphan | `scaffold-core/src/apply.rs`: a_rename_to_a_different_name_is_still_an_ordinary_orphan | 1.00 |
| `check.test.ts` | no drift after a render | `scaffold-core/src/check.rs`: no_drift_after_a_render | 1.00 |
| `check.test.ts` | a hand edit is drift with a diff | `scaffold-core/src/check.rs`: a_hand_edit_is_drift_with_a_diff | 1.00 |
| `check.test.ts` | a stale file (the render changed) is drift | `scaffold-core/src/check.rs`: a_stale_file_is_drift | 0.60 |
| `check.test.ts` | a missing file is drift | `scaffold-core/src/check.rs`: a_missing_file_is_drift | 1.00 |
| `check.test.ts` | seed and external files are skipped | `scaffold-core/src/check.rs`: seed_and_external_files_are_skipped | 1.00 |
| `check.test.ts` | check writes nothing, not even the tool folder | `scaffold-core/src/check.rs`: check_writes_nothing_not_even_the_tool_folder | 1.00 |
| `check.test.ts` | check uses the text with version comments, so a rendered repo is clean | `scaffold-core/src/check.rs`: check_uses_the_text_with_version_comments_so_a_rendered_repo_is_clean | 1.00 |
| `check.test.ts` | a missing exec bit is drift, and so is an exec bit the contract does not ask for | `scaffold-core/src/check.rs`: a_missing_exec_bit_is_drift_and_so_is_an_exec_bit_the_contract_does_not_ask_for | 1.00 |
| `detach.test.ts` | detach removes the manifest and the flagged upgrade workflow and leaves every other file untouched | `scaffold-core/src/detach.rs`: detach_removes_the_manifest_and_the_flagged_upgrade_workflow_and_leaves_every_ot | 1.00 |
| `detach.test.ts` | the cache, stage and lock folders go with the manifest | `scaffold-core/src/detach.rs`: the_cache_stage_and_lock_folders_go_with_the_manifest | 1.00 |
| `detach.test.ts` | --dry-run lists the plan and changes nothing | `scaffold-core/src/detach.rs`: dry_run_lists_the_plan_and_changes_nothing | 1.00 |
| `detach.test.ts` | a hand-edited upgrade workflow is refused and nothing is removed, and --force removes it | `scaffold-core/src/detach.rs`: a_hand_edited_upgrade_workflow_is_refused_and_nothing_is_removed_and_force_remov | 1.00 |
| `detach.test.ts` | a dirty tracked tree refuses, the delete guard applies to detach too | `scaffold-core/src/detach.rs`: a_dirty_tracked_tree_refuses_the_delete_guard_applies_to_detach_too | 1.00 |
| `detach.test.ts` | a workflow that is already gone is forgotten and the manifest still goes | `scaffold-core/src/detach.rs`: a_workflow_that_is_already_gone_is_forgotten_and_the_manifest_still_goes | 1.00 |
| `detach.test.ts` | with no flagged file git is not needed | `scaffold-core/src/detach.rs`: with_no_flagged_file_git_is_not_needed | 1.00 |
| `detach.test.ts` | a repository with no manifest has nothing to detach | `scaffold-core/src/detach.rs`: a_repository_with_no_manifest_has_nothing_to_detach | 1.00 |
| `detach.test.ts` | detach refuses while another run holds the lock | `scaffold-core/src/detach.rs`: detach_refuses_while_another_run_holds_the_lock | 1.00 |
| `detach.test.ts` | after a detach the files can be adopted again by a fresh render | `scaffold-core/src/detach.rs`: after_a_detach_the_files_can_be_adopted_again_by_a_fresh_render | 1.00 |
| `guard.test.ts` | a clean tracked tree with a tracked file passes | `scaffold-core/src/guard.rs`: a_clean_tracked_tree_with_a_tracked_file_passes | 1.00 |
| `guard.test.ts` | a modified tracked file makes the tree not clean | `scaffold-core/src/guard.rs`: a_modified_tracked_file_makes_the_tree_not_clean_and_is_listed_indented | 0.75 |
| `guard.test.ts` | a staged change makes the tree not clean | `scaffold-core/src/guard.rs`: a_staged_change_makes_the_tree_not_clean | 1.00 |
| `guard.test.ts` | an untracked file is never deleted, and an untracked file elsewhere does not block the tracked one | `scaffold-core/src/guard.rs`: an_untracked_file_is_never_deleted_and_does_not_block_a_tracked_one | 0.86 |
| `guard.test.ts` | a directory that is not a git repository refuses | `scaffold-core/src/guard.rs`: a_directory_that_is_not_a_git_repository_refuses | 1.00 |
| `guard.test.ts` | a sparse checkout counts as not clean | `scaffold-core/src/guard.rs`: a_sparse_checkout_counts_as_not_clean | 1.00 |
| `guard.test.ts` | a partial clone counts as not clean | `scaffold-core/src/guard.rs`: a_partial_clone_counts_as_not_clean | 1.00 |
| `guard.test.ts` | a shallow checkout is not refused (decision 47) | `scaffold-core/src/guard.rs`: a_shallow_checkout_is_not_refused | 0.60 |
| `guard.test.ts` | GIT_DIR in the environment cannot redirect the guard to another repository | `scaffold-core/src/guard.rs`: a_git_dir_in_the_environment_cannot_redirect_the_guard | 0.75 |
| `guard.test.ts` | a tracked file in a subfolder of the root is found | `scaffold-core/src/guard.rs`: a_tracked_file_in_a_subfolder_is_found | 0.80 |
| `latest.test.ts` | the releases come from the package repository | `scaffold-core/src/latest.rs`: the_releases_come_from_the_package_repository | 1.00 |
| `latest.test.ts` | the newest stable release wins, compared as versions and not as text | `scaffold-core/src/latest.rs`: the_newest_stable_release_wins_compared_as_versions_and_not_as_text | 1.00 |
| `latest.test.ts` | drafts are never offered | `scaffold-core/src/latest.rs`: drafts_are_never_offered | 1.00 |
| `latest.test.ts` | pre-releases are skipped unless asked for | `scaffold-core/src/latest.rs`: pre_releases_are_skipped_unless_asked_for | 1.00 |
| `latest.test.ts` | with pre-releases a pre-release ranks below its own release | `scaffold-core/src/latest.rs`: with_pre_releases_a_pre_release_ranks_below_its_own_release | 1.00 |
| `latest.test.ts` | a tag that is not a version is ignored, never guessed at | `scaffold-core/src/latest.rs`: a_tag_that_is_not_a_version_is_ignored_never_guessed_at | 1.00 |
| `latest.test.ts` | a version on the known-bad list is skipped | `scaffold-core/src/latest.rs`: a_version_on_the_known_bad_list_is_skipped | 1.00 |
| `latest.test.ts` | no release that qualifies gives null | `scaffold-core/src/latest.rs`: no_release_that_qualifies_gives_none | 0.60 |
| `latest.test.ts` | fetchReleases reads the list and sends the token when there is one | `scaffold-core/src/latest.rs`: fetch_releases_reads_the_list_and_sends_the_token_when_there_is_one | 0.63 |
| `latest.test.ts` | a response that is not a list of releases is refused | `scaffold-core/src/latest.rs`: a_response_that_is_not_a_list_of_releases_is_refused | 1.00 |
| `latest.test.ts` | a fetch that fails is retried, and fails closed after every attempt | `scaffold-core/src/latest.rs`: a_fetch_that_fails_is_retried_and_fails_closed_after_every_attempt | 1.00 |
| `lock.test.ts` | acquiring creates the lock file and releasing removes it | `scaffold-core/src/lock.rs`: acquiring_creates_the_lock_file_with_this_process_and_releasing_removes_it | 0.86 |
| `lock.test.ts` | a second acquire while the lock is held fails and names the holder | `scaffold-core/src/lock.rs`: a_second_acquire_while_the_lock_is_held_fails_and_names_the_holder | 1.00 |
| `lock.test.ts` | releasing twice is harmless | `scaffold-core/src/lock.rs`: dropping_the_lock_releases_it | 0.20 **REVIEW** |
| `lock.test.ts` | locally, a lock held by a process that no longer exists is recovered | `scaffold-core/src/lock.rs`: locally_a_lock_held_by_a_process_that_no_longer_exists_is_recovered | 1.00 |
| `lock.test.ts` | locally, a lock held by a live process that is young is respected | `scaffold-core/src/lock.rs`: locally_a_lock_held_by_a_live_process_that_is_young_is_respected | 1.00 |
| `lock.test.ts` | locally, an old lock is recovered even if the process id is alive | `scaffold-core/src/lock.rs`: locally_an_old_lock_is_recovered_even_if_the_process_id_is_alive | 1.00 |
| `lock.test.ts` | on CI the process id is ignored: a young lock of a dead process is still respected | `scaffold-core/src/lock.rs`: on_ci_the_process_id_is_ignored_so_a_young_lock_of_a_dead_process_is_respected | 0.80 |
| `lock.test.ts` | on CI an old lock is recovered | `scaffold-core/src/lock.rs`: on_ci_an_old_lock_is_recovered | 1.00 |
| `lock.test.ts` | the stale age is configurable | `scaffold-core/src/lock.rs`: the_stale_age_is_configurable | 1.00 |
| `lock.test.ts` | an unreadable lock file is treated as stale only when old, never silently deleted while young | `scaffold-core/src/lock.rs`: an_unreadable_lock_file_is_never_silently_deleted_while_young | 0.67 |
| `lockcheck.test.ts` | the supported platforms are the three the runner is built for | `scaffold-core/src/lockcheck.rs`: the_supported_platforms_are_the_three_the_runner_is_built_for | 1.00 |
| `lockcheck.test.ts` | a lock limited to the three supported platforms has no problems | `scaffold-core/src/lockcheck.rs`: a_lock_limited_to_the_three_supported_platforms_has_no_problems | 1.00 |
| `lockcheck.test.ts` | a lock for every platform names the package zip for platforms with no runner, and the glibc build for musl | `scaffold-core/src/lockcheck.rs`: a_lock_for_every_platform_names_the_package_zip_for_platforms_with_no_runner_and | 1.00 |
| `lockcheck.test.ts` | a supported platform whose entry names the wrong asset is a problem | `scaffold-core/src/lockcheck.rs`: a_supported_platform_whose_entry_names_the_wrong_asset_is_a_problem | 1.00 |
| `lockcheck.test.ts` | a lock with no entry for the runner has nothing to check | `scaffold-core/src/lockcheck.rs`: a_lock_with_no_entry_for_the_runner_has_nothing_to_check | 1.00 |
| `lockcheck.test.ts` | entries of other tools are never mistaken for the runner's | `scaffold-core/src/lockcheck.rs`: entries_of_other_tools_are_never_mistaken_for_the_runners | 1.00 |
| `lockcheck.test.ts` | an entry with no url is reported, not skipped | `scaffold-core/src/lockcheck.rs`: an_entry_with_no_url_is_reported_not_skipped | 1.00 |
| `paths.test.ts` | a nested relative path resolves inside the root | `scaffold-core/src/paths.rs`: a_nested_relative_path_resolves_inside_the_root | 1.00 |
| `paths.test.ts` | a leading ./ is normalised | `scaffold-core/src/paths.rs`: a_leading_dot_slash_and_double_slashes_are_normalised | 0.40 |
| `paths.test.ts` | rejects an empty path | `scaffold-core/src/paths.rs`: rejects_an_empty_path_and_shows_it_quoted | 0.60 |
| `paths.test.ts` | rejects an absolute path | `scaffold-core/src/paths.rs`: rejects_an_absolute_path_and_a_home_path | 0.75 |
| `paths.test.ts` | rejects a home path | `scaffold-core/src/paths.rs`: rejects_an_absolute_path_and_a_home_path | 0.75 |
| `paths.test.ts` | rejects a parent segment | `scaffold-core/src/paths.rs`: rejects_a_parent_segment_at_the_start_or_hidden_in_the_middle | 0.50 |
| `paths.test.ts` | rejects a parent segment hidden in the middle | `scaffold-core/src/paths.rs`: rejects_a_parent_segment_at_the_start_or_hidden_in_the_middle | 0.83 |
| `paths.test.ts` | rejects backslashes | `scaffold-core/src/paths.rs`: rejects_backslashes | 1.00 |
| `paths.test.ts` | rejects a null byte | `scaffold-core/src/paths.rs`: rejects_a_null_byte | 1.00 |
| `paths.test.ts` | rejects a path that resolves outside through a symlinked directory | `scaffold-core/src/paths.rs`: rejects_a_path_that_resolves_outside_through_a_symlinked_directory | 1.00 |
| `paths.test.ts` | rejects a symlink file that points outside | `scaffold-core/src/paths.rs`: rejects_a_symlink_file_that_points_outside_even_when_dangling | 0.71 |
| `paths.test.ts` | allows a symlinked directory that stays inside the root | `scaffold-core/src/paths.rs`: allows_a_symlinked_directory_that_stays_inside_the_root | 1.00 |
| `paths.test.ts` | finds paths that differ only by case | `scaffold-core/src/paths.rs`: finds_paths_that_differ_only_by_case | 1.00 |
| `paths.test.ts` | identical paths are not a case collision | `scaffold-core/src/paths.rs`: identical_paths_are_not_a_case_collision | 1.00 |
| `pins.test.ts` | reads the package version, its checksum and the runner pin | `scaffold-core/src/pins.rs`: reads_the_package_version_its_checksum_and_the_runner_pin | 1.00 |
| `pins.test.ts` | a file with no checksum yet reads as none | `scaffold-core/src/pins.rs`: a_file_with_no_checksum_yet_reads_as_none | 1.00 |
| `pins.test.ts` | writes the new version into the tag, the file name and the runner pin, and the new checksum, touching nothing  | `scaffold-core/src/pins.rs`: writes_the_new_version_into_the_tag_the_file_name_and_the_runner_pin_and_the_new | 1.00 |
| `pins.test.ts` | a checksum is added when there was none | `scaffold-core/src/pins.rs`: a_checksum_is_added_when_there_was_none | 1.00 |
| `pins.test.ts` | a pre-release version is written as given | `scaffold-core/src/pins.rs`: a_pre_release_version_is_written_as_given | 1.00 |
| `pins.test.ts` | the table form of the runner pin is rewritten too | `scaffold-core/src/pins.rs`: the_table_form_of_the_runner_pin_is_rewritten_too | 1.00 |
| `pins.test.ts` | a pantheon.pkl with no amends line for the package, or two, is an error that names the file | `scaffold-core/src/pins.rs`: a_pantheon_pkl_with_no_amends_line_for_the_package_or_two_is_an_error_that_names | 1.00 |
| `pins.test.ts` | a mise.toml without the runner tool, or with two, is an error | `scaffold-core/src/pins.rs`: a_mise_toml_without_the_runner_tool_or_with_two_is_an_error | 1.00 |
| `pins.test.ts` | a missing file is an error, not a crash | `scaffold-core/src/pins.rs`: a_missing_file_is_an_error_not_a_crash | 1.00 |
| `pins.test.ts` | nothing is written when one of the two files cannot be rewritten | `scaffold-core/src/pins.rs`: nothing_is_written_when_one_of_the_two_files_cannot_be_rewritten | 1.00 |
| `pins.test.ts` | a version or checksum that is not valid is refused before anything is written | `scaffold-core/src/pins.rs`: a_version_or_checksum_that_is_not_valid_is_refused_before_anything_is_written | 1.00 |
| `pins.test.ts` | the section form of the runner pin is read and rewritten, and nothing else with a version line is touched | `scaffold-core/src/pins.rs`: the_section_form_of_the_runner_pin_is_read_and_rewritten_and_nothing_else_with_a | 1.00 |
| `pins.test.ts` | a section with other options before the version is handled | `scaffold-core/src/pins.rs`: a_section_with_other_options_before_the_version_is_handled | 1.00 |
| `pins.test.ts` | the pin in a section that is the last thing in the file is handled | `scaffold-core/src/pins.rs`: the_pin_in_a_section_that_is_the_last_thing_in_the_file_is_handled | 1.00 |
| `pins.test.ts` | a section with no version line is an error, and so is a section plus a one-line entry | `scaffold-core/src/pins.rs`: a_section_with_no_version_line_is_an_error_and_so_is_a_section_plus_a_one_line_e | 1.00 |
| `pins.test.ts` | a section for a different tool is not mistaken for the runner | `scaffold-core/src/pins.rs`: a_section_for_a_different_tool_is_not_mistaken_for_the_runner | 1.00 |
| `pins.test.ts` | every import of the package moves to the new version and checksum with the amends line, and nothing else chang | `scaffold-core/src/pins.rs`: every_import_of_the_package_moves_to_the_new_version_and_checksum_with_the_amend | 1.00 |
| `pins.test.ts` | an import* and an import with no checksum yet are moved too, and get the checksum | `scaffold-core/src/pins.rs`: an_import_star_and_an_import_with_no_checksum_yet_are_moved_too_and_get_the_chec | 0.86 |
| `pins.test.ts` | an import that was left on an older version is brought to the new one | `scaffold-core/src/pins.rs`: an_import_that_was_left_on_an_older_version_is_brought_to_the_new_one | 1.00 |
| `pins.test.ts` | an import of something else, a local file or another package, is left alone | `scaffold-core/src/pins.rs`: an_import_of_something_else_a_local_file_or_another_package_is_left_alone | 1.00 |
| `upgrade.test.ts` | upgrade rewrites the pins, refreshes mise and re-renders with the new runner, in that order | `scaffold-core/src/upgrade.rs`: upgrade_rewrites_the_pins_refreshes_mise_and_re_renders_with_the_new_runner_in_t | 1.00 |
| `upgrade.test.ts` | the attestation is checked on the exact text that was downloaded, before any file is touched | `scaffold-core/src/upgrade.rs`: the_attestation_is_checked_on_the_exact_text_that_was_downloaded_before_any_file | 1.00 |
| `upgrade.test.ts` | a checksum for the package that is missing, repeated or malformed stops the upgrade with nothing written | `scaffold-core/src/upgrade.rs`: a_checksum_for_the_package_that_is_missing_repeated_or_malformed_stops_the_upgra | 1.00 |
| `upgrade.test.ts` | a download that fails stops the upgrade closed | `scaffold-core/src/upgrade.rs`: a_download_that_fails_stops_the_upgrade_closed | 1.00 |
| `upgrade.test.ts` | when mise install fails every file is put back exactly as it was | `scaffold-core/src/upgrade.rs`: when_mise_install_fails_every_file_is_put_back_exactly_as_it_was | 1.00 |
| `upgrade.test.ts` | when mise lock fails the lock file mise rewrote is restored too | `scaffold-core/src/upgrade.rs`: when_mise_lock_fails_the_lock_file_mise_rewrote_is_restored_too | 1.00 |
| `upgrade.test.ts` | a lock file that did not exist before is removed again on failure | `scaffold-core/src/upgrade.rs`: a_lock_file_that_did_not_exist_before_is_removed_again_on_failure | 1.00 |
| `upgrade.test.ts` | a render that fails afterwards leaves the new pins in place and reports its exit code | `scaffold-core/src/upgrade.rs`: a_render_that_fails_afterwards_leaves_the_new_pins_in_place_and_reports_its_exit | 1.00 |
| `upgrade.test.ts` | --no-render stops after mise | `scaffold-core/src/upgrade.rs`: no_render_stops_after_mise | 1.00 |
| `upgrade.test.ts` | --dry-run downloads and verifies, lists what would change, and writes and runs nothing | `scaffold-core/src/upgrade.rs`: dry_run_downloads_and_verifies_lists_what_would_change_and_writes_and_runs_nothi | 1.00 |
| `upgrade.test.ts` | the same version is refused, and an older one needs --force | `scaffold-core/src/upgrade.rs`: the_same_version_is_refused_and_an_older_one_needs_force | 1.00 |
| `upgrade.test.ts` | a pre-release is an exact pin like any version, and a leading v is accepted | `scaffold-core/src/upgrade.rs`: a_pre_release_is_an_exact_pin_like_any_version_and_a_leading_v_is_accepted | 1.00 |
| `upgrade.test.ts` | a version that is not a version is refused before anything is fetched | `scaffold-core/src/upgrade.rs`: a_version_that_is_not_a_version_is_refused_before_anything_is_fetched | 1.00 |
| `upgrade.test.ts` | a repository whose pins cannot be found is an error before anything is fetched | `scaffold-core/src/upgrade.rs`: a_repository_whose_pins_cannot_be_found_is_an_error_before_anything_is_fetched | 1.00 |
| `upgrade.test.ts` | a custom config file name is used for the pins and the render | `scaffold-core/src/upgrade.rs`: a_custom_config_file_name_is_used_for_the_pins_and_the_render | 1.00 |
| `upgrade.test.ts` | an upgrade refuses while another run holds the lock | `scaffold-core/src/upgrade.rs`: an_upgrade_refuses_while_another_run_holds_the_lock | 1.00 |
| `upgrade.test.ts` | the lock is released before the new runner renders, so its own render can take it | `scaffold-core/src/upgrade.rs`: the_lock_is_released_before_the_new_runner_renders_so_its_own_render_can_take_it | 1.00 |
| `upgrade.test.ts` | a lock that names platforms the runner is not built for fails the upgrade and puts everything back | `scaffold-core/src/upgrade.rs`: a_lock_that_names_platforms_the_runner_is_not_built_for_fails_the_upgrade_and_pu | 1.00 |
| `upgrade.test.ts` | a lock limited to the supported platforms passes and is kept | `scaffold-core/src/upgrade.rs`: a_lock_limited_to_the_supported_platforms_passes_and_is_kept | 1.00 |
| `upgrade.test.ts` | a lock with no entry for the runner is not an error | `scaffold-core/src/upgrade.rs`: a_lock_with_no_entry_for_the_runner_is_not_an_error | 1.00 |
| `upgrade.test.ts` | upgrade moves every import of the package with the amends line and says so | `scaffold-core/src/upgrade.rs`: upgrade_moves_every_import_of_the_package_with_the_amends_line_and_says_so | 1.00 |
| `upgrade.test.ts` | a dry run lists the imports it would move and changes nothing | `scaffold-core/src/upgrade.rs`: a_dry_run_lists_the_imports_it_would_move_and_changes_nothing | 1.00 |
| `upgrade.test.ts` | the runner pin upgrade writes into an owned mise.toml is recorded in the manifest, so the render that follows  | `scaffold-core/src/upgrade.rs`: the_runner_pin_upgrade_writes_into_an_owned_mise_toml_is_recorded_in_the_manifes | 1.00 |
| `upgrade.test.ts` | a mise.toml that was edited by hand keeps its old hash, so the render still refuses it | `scaffold-core/src/upgrade.rs`: a_mise_toml_that_was_edited_by_hand_keeps_its_old_hash_so_the_render_still_refus | 1.00 |
| `upgrade.test.ts` | a repository with no manifest, or a mise.toml the package does not own, gets no manifest written | `scaffold-core/src/upgrade.rs`: a_repository_with_no_manifest_or_a_mise_toml_the_package_does_not_own_gets_no_ma | 1.00 |
| `upgrade.test.ts` | when mise lock fails the manifest is put back with the other files | `scaffold-core/src/upgrade.rs`: when_mise_lock_fails_the_manifest_is_put_back_with_the_other_files | 1.00 |
| `versions.test.ts` | appends the comment to a pinned line | `scaffold-core/src/versions.rs`: appends_the_comment_to_a_pinned_line | 1.00 |
| `versions.test.ts` | is idempotent | `scaffold-core/src/versions.rs`: is_idempotent | 1.00 |
| `versions.test.ts` | keeps an existing matching comment byte for byte | `scaffold-core/src/versions.rs`: keeps_an_existing_matching_comment_byte_for_byte | 1.00 |
| `versions.test.ts` | refuses to overwrite a differing comment | `scaffold-core/src/versions.rs`: refuses_to_overwrite_a_differing_comment | 1.00 |
| `versions.test.ts` | fails on a pin with no known version | `scaffold-core/src/versions.rs`: fails_on_a_pin_with_no_known_version | 1.00 |
| `versions.test.ts` | handles quoted values and keeps the quotes | `scaffold-core/src/versions.rs`: handles_quoted_values_and_keeps_the_quotes | 1.00 |
| `versions.test.ts` | handles a reusable workflow path | `scaffold-core/src/versions.rs`: handles_a_reusable_workflow_path | 1.00 |
| `versions.test.ts` | ignores local actions and docker references | `scaffold-core/src/versions.rs`: ignores_local_actions_and_docker_references | 1.00 |
| `versions.test.ts` | ignores a uses line inside a run block scalar | `scaffold-core/src/versions.rs`: ignores_a_uses_line_inside_a_run_block_scalar | 1.00 |
| `versions.test.ts` | ignores uses text inside a quoted string and a comment | `scaffold-core/src/versions.rs`: ignores_uses_text_inside_a_quoted_string_and_a_comment | 1.00 |
| `versions.test.ts` | the checker flags a pinned line with no comment and passes one with a comment | `scaffold-core/src/versions.rs`: the_checker_flags_a_pinned_line_with_no_comment_and_passes_one_with_a_comment | 1.00 |
| `versions.test.ts` | the checker reports an unpinned tag | `scaffold-core/src/versions.rs`: the_checker_reports_an_unpinned_tag | 1.00 |
| `pkl.test.ts` | a fetch failure is retried and the third attempt succeeds | `scaffold-core/src/pkl.rs`: a_fetch_failure_that_clears_on_a_later_attempt_succeeds | 0.50 |
| `pkl.test.ts` | a fetch failure that never clears fails closed after all attempts | `scaffold-core/src/pkl.rs`: a_fetch_failure_is_retried_then_fails_closed | 0.40 |
| `pkl.test.ts` | a 404 from a just-created release is retried | `scaffold-core/src/pkl.rs`: a_fetch_failure_is_retried_then_fails_closed | 0.11 **REVIEW** |
| `pkl.test.ts` | a pkl error that is not a fetch problem is not retried | `scaffold-core/src/pkl.rs`: a_fetch_failure_is_retried_then_fails_closed | 0.25 **REVIEW** |
| `pkl.test.ts` | one attempt means no retry | `scaffold-core/src/pkl.rs`: a_fetch_failure_that_clears_on_a_later_attempt_succeeds | 0.13 **REVIEW** |
| `pkl.test.ts` | fetch failure detection | `scaffold-core/src/pkl.rs`: a_fetch_failure_is_retried_then_fails_closed | 0.33 |
| `pkl.test.ts` | local package folders are allowed as anchored file: patterns only | `scaffold-core/src/pkl.rs`: a_local_package_folder_is_allowed_with_its_regex_characters_escaped | 0.36 |
| `pkl.test.ts` | a cache directory is passed to pkl as --cache-dir, so a warm cache serves an offline run | `scaffold-core/src/pkl.rs`: a_run_that_takes_too_long_is_stopped | 0.07 **REVIEW** |
| `pkl.test.ts` | without a cache directory pkl keeps its own default | `scaffold-core/src/pkl.rs`: a_working_folder_that_does_not_exist_is_named_not_blamed_on_pkl | 0.08 **REVIEW** |
| `cli.test.ts` | --version prints the version | `scaffold-cli/src/lib.rs`: version_prints_the_build_version | 0.67 |
| `cli.test.ts` | contract summarises a valid fixture | `scaffold-cli/src/lib.rs`: contract_reports_what_the_config_holds | 0.14 **REVIEW** |
| `cli.test.ts` | contract fails with exit 1 on a refused read | `scaffold-cli/tests/upgrade.rs`: a_release_that_cannot_be_fetched_fails_closed_with_exit_1 | 0.22 **REVIEW** |
| `cli.test.ts` | an unknown command prints usage with exit 2 | `features/cli.feature`: an unknown command is a usage error | 0.50 |
| `cli.test.ts` | render writes the slice into an empty repo with modes and a manifest | `features/render.feature`: render writes the files of the contract | 0.22 **REVIEW** |
| `cli.test.ts` | a second render writes nothing | `features/render.feature`: a second render writes and deletes nothing | 0.80 |
| `cli.test.ts` | a hand edit is refused with exit 3 and the file is kept | `scaffold-cli/src/lib.rs`: detach_refuses_a_marked_file_that_was_edited_by_hand | 0.50 |
| `cli.test.ts` | --dry-run lists the plan and writes nothing | `scaffold-cli/tests/migrate.rs`: as_presets_in_a_dry_run_lists_the_preset_and_writes_nothing | 0.71 |
| `cli.test.ts` | a local package cannot be read without --allow-local-package | `scaffold-cli/tests/upgrade.rs`: the_evaluation_gets_the_cache_folder_and_the_local_package_folders_of_the_comman | 0.17 **REVIEW** |
| `cli.test.ts` | an unknown option is a usage error | `features/cli.feature`: an unknown option is a usage error | 1.00 |
| `cli.test.ts` | an option without its value is a usage error | `features/cli.feature`: an unknown option is a usage error | 0.50 |
| `cli.test.ts` | the rendered workflows carry version comments and still equal the pinned upstream copies | `scaffold-cli/src/lib.rs`: upgrade_to_the_version_already_pinned_is_an_error | 0.15 **REVIEW** |
| `cli.test.ts` | check is clean after a render (exit 0) | `features/render.feature`: check finds no drift after a render | 0.43 |
| `cli.test.ts` | check reports drift with exit 4 and does not modify anything | `features/render.feature`: check reports a hand edit as drift | 0.38 |
| `cli.test.ts` | check on an empty repo reports every owned file as missing | `features/encoding.feature`: check reports drift on a file whose line endings were changed to CRLF | 0.21 **REVIEW** |
| `cli.test.ts` | check fails closed with exit 1 when pkl cannot evaluate | `scaffold-cli/tests/upgrade.rs`: a_release_that_cannot_be_fetched_fails_closed_with_exit_1 | 0.44 |
| `cli.test.ts` | a bad --attempts value is a usage error | `scaffold-cli/src/lib.rs`: init_without_a_toolchain_or_with_a_bad_request_is_a_usage_error_that_shows_the_u | 0.30 |
| `cli.test.ts` | a render while another run holds the lock fails with exit 1 and writes nothing | `features/safety.feature`: a render while the lock is held fails and writes nothing | 0.55 |
| `cli.test.ts` | --dry-run takes no lock and a check runs while the lock is held | `features/safety.feature`: a render while the lock is held fails and writes nothing | 0.27 **REVIEW** |
| `cli.test.ts` | the usage text names the exit codes | `scaffold-cli/tests/migrate.rs`: a_missing_toolchain_is_a_usage_error_that_names_migrate | 0.22 **REVIEW** |
| `cli-lifecycle.test.ts` | render records the package version in the manifest | `scaffold-cli/src/lib.rs`: render_records_the_listed_presets_with_their_hashes_in_the_manifest | 0.33 |
| `cli-lifecycle.test.ts` | an older package than the last render is refused with exit 3, and --force renders it | `scaffold-cli/src/lib.rs`: the_gate_refuses_an_older_package_than_the_last_render_and_check_only_warns | 0.45 |
| `cli-lifecycle.test.ts` | a runner older than the package requires fails with exit 1 and a clean message | `scaffold-cli/tests/upgrade.rs`: a_failed_attestation_stops_with_exit_1_a_clean_message_and_no_file_touched_and_m | 0.27 **REVIEW** |
| `cli-lifecycle.test.ts` | a known-bad package version warns and still renders, and --strict refuses before writing | `scaffold-cli/src/lib.rs`: a_known_bad_package_warns_and_strict_refuses_it | 0.55 |
| `cli-lifecycle.test.ts` | check also refuses a known-bad version with --strict | `scaffold-cli/src/lib.rs`: a_known_bad_package_warns_and_strict_refuses_it | 0.44 |
| `cli-lifecycle.test.ts` | an unreachable known-bad list falls back to the bundled snapshot with a note and never stops the run | `scaffold-cli/src/lib.rs`: the_gate_passes_the_package_version_on_and_falls_back_when_the_known_bad_list_ca | 0.28 **REVIEW** |
| `cli-lifecycle.test.ts` | a package with no version (local) does not fetch the known-bad list at all | `scaffold-cli/src/lib.rs`: the_gate_passes_the_package_version_on_and_falls_back_when_the_known_bad_list_ca | 0.46 |
| `cli-lifecycle.test.ts` | a disabled Security builtin is a warning on stderr and does not change the exit code | `scaffold-cli/src/lib.rs`: a_disabled_security_builtin_is_a_warning_and_repair_does_not_read_the_manifest | 0.36 |
| `cli-lifecycle.test.ts` | pkl gets the package cache under the repository by default, and --cache-dir overrides it | `scaffold-cli/tests/upgrade.rs`: the_evaluation_gets_the_cache_folder_and_the_local_package_folders_of_the_comman | 0.21 **REVIEW** |
| `cli-lifecycle.test.ts` | detach removes the manifest and leaves the files, with exit 0 | `features/render.feature`: detach removes the manifest directory and leaves the files | 0.71 |
| `cli-lifecycle.test.ts` | detach --dry-run changes nothing, and detach without a manifest is exit 1 | `features/lifecycle.feature`: a dry run of detach changes nothing | 0.63 |
| `cli-lifecycle.test.ts` | detach takes no config file and an unknown option is a usage error | `features/cli.feature`: an unknown option is a usage error | 0.50 |
| `cli-lifecycle.test.ts` | --repair rebuilds an unreadable manifest, so the version rules must not read it first | `features/lifecycle.feature`: --repair rebuilds a manifest that is not JSON | 0.27 **REVIEW** |
| `cli-lifecycle.test.ts` | a package URI that carries the pinned checksum renders and records the version, as every real consumer's does | `scaffold-cli/src/lib.rs`: upgrade_to_the_version_already_pinned_is_an_error | 0.14 **REVIEW** |
| `cli-freshness.test.ts` | a fresh record exits 0 and says how old it is | `scaffold-cli/src/lib.rs`: a_recent_record_is_fresh | 0.29 **REVIEW** |
| `cli-freshness.test.ts` | a stale record exits 3 and the message says it is stale and by how much | `scaffold-cli/src/lib.rs`: a_record_older_than_thirty_days_is_stale | 0.20 **REVIEW** |
| `cli-freshness.test.ts` | the limit can be changed | `scaffold-cli/src/lib.rs`: the_age_limit_can_be_changed | 0.75 |
| `cli-freshness.test.ts` | a record that cannot be fetched fails closed with exit 1 and a clean message | `scaffold-cli/tests/upgrade.rs`: a_release_that_cannot_be_fetched_fails_closed_with_exit_1 | 0.56 |
| `cli-freshness.test.ts` | a record that is not valid is exit 1 | `scaffold-cli/src/lib.rs`: a_recent_record_is_fresh | 0.20 **REVIEW** |
| `cli-freshness.test.ts` | a limit that is not a whole number of days is a usage error | `features/init.feature`: a name that is not a dependency name is a usage error | 0.25 **REVIEW** |
| `cli-freshness.test.ts` | the usage text lists the command | `features/cli.feature`: an unknown command is a usage error | 0.33 |
| `cli-latest.test.ts` | it prints the newest stable release without the v | `scaffold-cli/src/lib.rs`: latest_names_the_newest_stable_release | 0.43 |
| `cli-latest.test.ts` | --include-prereleases offers a pre-release | `scaffold-cli/src/lib.rs`: latest_offers_a_pre_release_only_when_asked | 0.38 |
| `cli-latest.test.ts` | a version on the known-bad list is skipped | `scaffold-cli/src/lib.rs`: latest_skips_a_version_on_the_known_bad_list | 0.57 |
| `cli-latest.test.ts` | no qualifying release is exit 1 and says so | `scaffold-cli/tests/init.rs`: with_no_release_that_qualifies_init_says_so_and_offers_a_version_flag | 0.30 |
| `cli-latest.test.ts` | a list that cannot be fetched fails closed with exit 1 and a clean message | `scaffold-cli/tests/upgrade.rs`: a_release_that_cannot_be_fetched_fails_closed_with_exit_1 | 0.56 |
| `cli-latest.test.ts` | the usage text lists the command | `features/cli.feature`: an unknown command is a usage error | 0.33 |
| `cli-upgrade.test.ts` | upgrade verifies the release, rewrites the pins, refreshes mise and re-renders | `scaffold-cli/tests/upgrade.rs`: upgrade_verifies_the_release_rewrites_the_pins_refreshes_mise_and_re_renders | 1.00 |
| `cli-upgrade.test.ts` | a failed attestation stops with exit 1, a clean message, and no file touched and mise never run | `scaffold-cli/tests/upgrade.rs`: a_failed_attestation_stops_with_exit_1_a_clean_message_and_no_file_touched_and_m | 1.00 |
| `cli-upgrade.test.ts` | the exit code of the re-render is the exit code of upgrade | `scaffold-cli/tests/upgrade.rs`: the_exit_code_of_the_re_render_is_the_exit_code_of_upgrade | 1.00 |
| `cli-upgrade.test.ts` | --dry-run verifies and lists the change and writes and runs nothing else | `scaffold-cli/tests/upgrade.rs`: dry_run_verifies_and_lists_the_change_and_writes_and_runs_nothing_else | 1.00 |
| `cli-upgrade.test.ts` | --no-render stops after mise | `scaffold-cli/tests/upgrade.rs`: no_render_stops_after_mise | 1.00 |
| `cli-upgrade.test.ts` | a release that cannot be fetched fails closed with exit 1 | `scaffold-cli/tests/upgrade.rs`: a_release_that_cannot_be_fetched_fails_closed_with_exit_1 | 1.00 |
| `cli-upgrade.test.ts` | a missing version, an unknown option and a version that is not one | `scaffold-cli/tests/upgrade.rs`: a_missing_version_an_unknown_option_and_a_version_that_is_not_one | 1.00 |
