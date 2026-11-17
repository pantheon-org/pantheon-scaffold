//! The `refresh-pins` mise task of `mise.toml`: the one command a person runs on the pin bot's branch (route (d) of the update-pins plan).
//! A pin that moves changes the files this repository renders from its own `scaffold.pkl`, and `Scaffold check` fails until they are
//! rendered again. The baseline no longer moves with a pin (`scaffold_core::pinmask`, added by pull request 166), so the task only has to render and check.

use scaffold_package_tests::pkg::read;
use toml::Value;

fn steps() -> Vec<String> {
    let doc: Value = toml::from_str(&read("mise.toml")).expect("mise.toml is TOML");
    doc["tasks"]["refresh-pins"]["run"]
        .as_array()
        .expect("the task has a list of steps")
        .iter()
        .map(|s| s.as_str().expect("a step is text").to_owned())
        .collect()
}

fn position(steps: &[String], needle: &str) -> usize {
    steps
        .iter()
        .position(|s| s.contains(needle))
        .unwrap_or_else(|| panic!("no step contains {needle}: {steps:?}"))
}

#[test]
fn the_task_exists_and_has_a_description() {
    let doc: Value = toml::from_str(&read("mise.toml")).unwrap();
    let description = doc["tasks"]["refresh-pins"]["description"]
        .as_str()
        .expect("the task has a description");
    assert!(description.contains("pin"), "{description}");
}

#[test]
fn it_builds_renders_then_checks_the_baseline_and_then_the_render() {
    let steps = steps();
    let build = position(&steps, "cargo build");
    let render = position(&steps, "render scaffold.pkl");
    let baseline = position(&steps, "baseline check");
    let check = position(&steps, " check scaffold.pkl");
    assert!(
        build < render && render < baseline && baseline < check,
        "{steps:?}"
    );
}

#[test]
fn it_builds_both_binaries_with_a_locked_cargo_and_names_the_local_package() {
    let steps = steps();
    let build = &steps[position(&steps, "cargo build")];
    assert!(
        build.contains("-p scaffold-cli") && build.contains("-p scaffold-release"),
        "{build}"
    );
    assert!(build.contains("--locked"), "{build}");
    let render = &steps[position(&steps, "render scaffold.pkl")];
    assert!(render.contains("--allow-local-package pkl"), "{render}");
}

#[test]
fn it_never_freezes_the_baseline_because_that_would_accept_any_change_as_expected() {
    for step in steps() {
        assert!(!step.contains("baseline freeze"), "{step}");
        assert!(!step.contains("baseline.json >"), "{step}");
    }
}
