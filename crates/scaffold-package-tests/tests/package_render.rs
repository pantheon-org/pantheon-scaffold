//! Ported from `runner/test/package-render.test.ts` (which was ported from `cli.test.ts` when the TypeScript runner was retired): the
//! workflows the runner writes carry the version comment of their pin and still equal the pinned upstream copies under the P.1 rule.

use scaffold_package_tests::cli::run;
use scaffold_package_tests::compare_workflows;
use scaffold_package_tests::pkg::{fixture, read, root, unpin};
use scaffold_package_tests::repo::temp_dir;

#[test]
fn the_rendered_workflows_carry_version_comments_and_still_equal_the_pinned_upstream_copies() {
    let repo = temp_dir();
    let config = fixture("slice");
    let pkl = root().join("pkl");
    let rendered = run(
        &[
            "render",
            &config.to_string_lossy(),
            "--root",
            &repo.path().to_string_lossy(),
            "--allow-local-package",
            &pkl.to_string_lossy(),
        ],
        &root(),
    );
    assert_eq!(rendered.code, 0, "{}", rendered.err);
    let version_comment = regex::Regex::new(r"actions/checkout@[0-9a-f]{40} # v\d").unwrap();
    for name in ["cargo-deny", "inline-scripts", "rust-ci"] {
        let text =
            std::fs::read_to_string(repo.path().join(format!(".github/workflows/{name}.yml")))
                .unwrap();
        assert!(version_comment.is_match(&text), "{name}");
        let upstream = read(&format!(
            "crates/scaffold-package-tests/upstream/workflows/{name}.yml"
        ));
        let comparison = compare_workflows(&unpin(&upstream), &unpin(&text)).unwrap();
        assert!(comparison.equal, "{name}: {:?}", comparison.differences);
        assert!(comparison.duplicate_keys.is_empty(), "{name}");
    }
}
