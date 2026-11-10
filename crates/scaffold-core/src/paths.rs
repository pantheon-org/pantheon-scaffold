//! Turns a contract path into an absolute path inside the repository, or refuses it. Ported from `runner/src/paths.ts`.

use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct PathError(pub String);

impl std::fmt::Display for PathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for PathError {}

/// Collapses `.`, `..` and repeated slashes without touching the file system, like Node's `path.normalize`. A `..` with
/// nothing before it is kept.
fn normalise(path: &Path) -> Vec<std::ffi::OsString> {
    use std::path::Component;
    let mut parts: Vec<std::ffi::OsString> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_owned()),
            Component::ParentDir if parts.last().is_some_and(|last| last != "..") => {
                parts.pop();
            }
            Component::ParentDir => parts.push("..".into()),
            _ => {}
        }
    }
    parts
}

fn is_link(path: &Path) -> bool {
    path.symlink_metadata().is_ok_and(|m| m.is_symlink())
}

/// Where a path really points. A dangling symlink resolves to its target as written.
fn real_target(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    match (is_link(path), std::fs::read_link(path)) {
        (true, Ok(target)) => {
            let base = path.parent().unwrap_or(Path::new("/"));
            let joined = if target.is_absolute() {
                target
            } else {
                base.join(target)
            };
            let mut resolved = PathBuf::from("/");
            resolved.extend(normalise(&joined));
            resolved
        }
        _ => path.to_owned(),
    }
}

/// Resolves a contract path to an absolute path inside `root`, or fails. The root must exist.
pub fn resolve_inside(root: &Path, rel: &str) -> Result<PathBuf, PathError> {
    let fail = |message: String| Err(PathError(message));
    if rel.is_empty() || rel.contains('\0') {
        return fail(format!(
            "{}: empty or invalid path",
            serde_json::to_string(rel).expect("a string serialises")
        ));
    }
    if rel.starts_with('/') || rel.starts_with('~') {
        return fail(format!("{rel}: absolute paths are not allowed"));
    }
    if rel.contains('\\') {
        return fail(format!("{rel}: backslashes are not allowed"));
    }
    let clean = normalise(Path::new(rel));
    if clean.iter().any(|part| part == "..") {
        return fail(format!("{rel}: escapes the repository"));
    }
    let root_real = root.canonicalize().map_err(|e| {
        PathError(format!(
            "the repository root cannot be read: {}: {e}",
            root.display()
        ))
    })?;
    let mut abs = root_real.clone();
    abs.extend(&clean);
    // The deepest existing ancestor (or the link itself) must resolve, through symlinks, to somewhere inside the root.
    let mut probe = abs.clone();
    while !probe.exists() && !is_link(&probe) {
        match probe.parent() {
            Some(parent) => probe = parent.to_owned(),
            None => break,
        }
    }
    if !real_target(&probe).starts_with(&root_real) {
        return fail(if is_link(&probe) && probe == abs {
            format!("{rel}: is a symlink that points outside the repository")
        } else {
            format!("{rel}: resolves outside the repository through a symlink")
        });
    }
    Ok(abs)
}

/// Two paths that differ only by case would collide on a case-insensitive filesystem.
pub fn find_case_collisions(paths: &[&str]) -> Vec<String> {
    let mut seen: std::collections::HashMap<String, &str> = std::collections::HashMap::new();
    let mut out = Vec::new();
    for &path in paths {
        match seen.get(&path.to_lowercase()) {
            Some(&previous) if previous != path => {
                out.push(format!("{previous} and {path} differ only by case"))
            }
            Some(_) => {}
            None => {
                seen.insert(path.to_lowercase(), path);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    fn err(root: &Path, rel: &str) -> String {
        resolve_inside(root, rel).unwrap_err().to_string()
    }

    #[test]
    fn a_nested_relative_path_resolves_inside_the_root() {
        let dir = root();
        let path = resolve_inside(dir.path(), ".github/workflows/ci.yml").unwrap();
        assert!(path.ends_with(".github/workflows/ci.yml"));
        assert!(path.starts_with(dir.path().canonicalize().unwrap()));
    }

    #[test]
    fn a_leading_dot_slash_and_double_slashes_are_normalised() {
        let dir = root();
        assert!(
            resolve_inside(dir.path(), "./a.txt")
                .unwrap()
                .ends_with("a.txt")
        );
        let path = resolve_inside(dir.path(), "a//b/./c.txt").unwrap();
        assert!(path.ends_with("a/b/c.txt"));
    }

    #[test]
    fn a_parent_segment_that_stays_inside_is_resolved() {
        let dir = root();
        let path = resolve_inside(dir.path(), "a/../b.txt").unwrap();
        assert_eq!(path, dir.path().canonicalize().unwrap().join("b.txt"));
    }

    #[test]
    fn rejects_an_empty_path_and_shows_it_quoted() {
        assert_eq!(err(root().path(), ""), "\"\": empty or invalid path");
    }

    #[test]
    fn rejects_a_null_byte() {
        assert_eq!(
            err(root().path(), "a\0b"),
            "\"a\\u0000b\": empty or invalid path"
        );
    }

    #[test]
    fn rejects_an_absolute_path_and_a_home_path() {
        assert_eq!(
            err(root().path(), "/etc/passwd"),
            "/etc/passwd: absolute paths are not allowed"
        );
        assert_eq!(
            err(root().path(), "~/x"),
            "~/x: absolute paths are not allowed"
        );
    }

    #[test]
    fn rejects_a_parent_segment_at_the_start_or_hidden_in_the_middle() {
        assert_eq!(err(root().path(), "../x"), "../x: escapes the repository");
        assert_eq!(
            err(root().path(), "a/../../x"),
            "a/../../x: escapes the repository"
        );
        assert_eq!(err(root().path(), ".."), "..: escapes the repository");
    }

    #[test]
    fn rejects_backslashes() {
        assert_eq!(
            err(root().path(), "a\\b"),
            "a\\b: backslashes are not allowed"
        );
    }

    #[test]
    fn rejects_a_path_that_resolves_outside_through_a_symlinked_directory() {
        let (dir, outside) = (root(), root());
        symlink(outside.path(), dir.path().join("link")).unwrap();
        assert_eq!(
            err(dir.path(), "link/file.txt"),
            "link/file.txt: resolves outside the repository through a symlink"
        );
    }

    #[test]
    fn rejects_a_symlink_file_that_points_outside_even_when_dangling() {
        let (dir, outside) = (root(), root());
        symlink(outside.path().join("x"), dir.path().join("f")).unwrap();
        assert_eq!(
            err(dir.path(), "f"),
            "f: is a symlink that points outside the repository"
        );
    }

    #[test]
    fn allows_a_symlinked_directory_that_stays_inside_the_root() {
        let dir = root();
        std::fs::create_dir(dir.path().join("real")).unwrap();
        symlink(dir.path().join("real"), dir.path().join("alias")).unwrap();
        assert!(resolve_inside(dir.path(), "alias/f.txt").is_ok());
    }

    #[test]
    fn a_root_that_does_not_exist_is_an_error() {
        assert!(
            err(Path::new("/no/such/root/anywhere"), "a.txt")
                .starts_with("the repository root cannot be read")
        );
    }

    #[test]
    fn accented_names_resolve() {
        let dir = root();
        assert!(
            resolve_inside(dir.path(), "café/ünï.txt")
                .unwrap()
                .ends_with("café/ünï.txt")
        );
    }

    #[test]
    fn finds_paths_that_differ_only_by_case() {
        assert_eq!(
            find_case_collisions(&["a/B.txt", "a/b.txt", "c"]),
            ["a/B.txt and a/b.txt differ only by case"]
        );
    }

    #[test]
    fn identical_paths_are_not_a_case_collision() {
        assert!(find_case_collisions(&["a", "a"]).is_empty());
    }
}
