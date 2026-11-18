//! `options.reviewBot` and `options.prAgentProviders`: which pull request review bot is rendered, and how the pr-agent workflow is built from
//! the providers. `auto` is pr-agent unless the runner reports a `.coderabbit.yaml` in the repository.

use scaffold_package_tests::pkg::{content, paths};
use scaffold_package_tests::presence::{eval_config, eval_config_detecting};
use serde_json::Value;

const WORKFLOW: &str = ".github/workflows/pr-agent.yml";
const TOML: &str = ".pr_agent.toml";
const CODERABBIT: &str = ".coderabbit.yaml";

fn has(contract: &Value, path: &str) -> bool {
    paths(contract).iter().any(|p| p == path)
}

fn render(without: &[&str], extra: &str) -> Value {
    eval_config(
        without,
        extra,
        scaffold_package_tests::presence::COMPOSITION,
        "",
    )
    .unwrap_or_else(|e| panic!("{e}"))
}

fn providers(list: &[(&str, &str, &str)]) -> String {
    let entries: Vec<String> = list
        .iter()
        .map(|(m, s, e)| format!("new {{ model = \"{m}\"; secret = \"{s}\"; env = \"{e}\" }}"))
        .collect();
    format!(
        "prAgentProviders = new Listing<Provider> {{ {} }}",
        entries.join("; ")
    )
}

fn error(extra: &str) -> String {
    eval_config(
        &[],
        extra,
        scaffold_package_tests::presence::COMPOSITION,
        "",
    )
    .expect_err("the configuration is refused")
}

#[test]
fn a_repository_that_sets_nothing_gets_pr_agent_and_no_coderabbit_file() {
    let c = render(&[], "");
    assert!(has(&c, WORKFLOW) && has(&c, TOML));
    assert!(!has(&c, CODERABBIT));
}

#[test]
fn reviewbot_coderabbit_renders_the_coderabbit_file_and_nothing_of_pr_agent() {
    let c = render(&[], "reviewBot = \"coderabbit\"");
    assert!(has(&c, CODERABBIT));
    assert!(!has(&c, WORKFLOW) && !has(&c, TOML));
}

#[test]
fn reviewbot_pr_agent_is_pr_agent_even_when_the_repository_has_a_coderabbit_file() {
    let c = eval_config_detecting(true, "reviewBot = \"pr-agent\"").unwrap();
    assert!(has(&c, WORKFLOW) && !has(&c, CODERABBIT));
}

#[test]
fn auto_keeps_coderabbit_where_the_repository_already_has_the_file() {
    let c = eval_config_detecting(true, "").unwrap();
    assert!(has(&c, CODERABBIT));
    assert!(!has(&c, WORKFLOW) && !has(&c, TOML));
}

#[test]
fn auto_is_pr_agent_where_the_repository_has_no_coderabbit_file() {
    let c = eval_config_detecting(false, "").unwrap();
    assert!(has(&c, WORKFLOW) && !has(&c, CODERABBIT));
}

#[test]
fn leaving_out_the_chosen_bot_renders_no_bot() {
    let c = render(&["pr-agent"], "");
    assert!(!has(&c, WORKFLOW) && !has(&c, TOML) && !has(&c, CODERABBIT));
    let c = render(&["coderabbit"], "reviewBot = \"coderabbit\"");
    assert!(!has(&c, WORKFLOW) && !has(&c, TOML) && !has(&c, CODERABBIT));
}

#[test]
fn an_unknown_reviewbot_is_refused() {
    assert!(error("reviewBot = \"coderabbit \"").contains("reviewBot"));
}

#[test]
fn the_default_providers_are_the_primary_and_two_fallbacks_in_order() {
    let w = render(&[], "");
    let text = content(&w, WORKFLOW);
    assert!(
        text.contains("config.model: gemini/gemini-3.5-flash"),
        "{text}"
    );
    assert!(
        text.contains(
            r#"config.fallback_models: '["mistral/mistral-small-2603", "cerebras/gpt-oss-120b"]'"#
        ),
        "{text}"
    );
}

#[test]
fn the_order_of_the_providers_is_the_order_of_fallback() {
    let w = render(
        &[],
        &providers(&[
            ("b/second", "B_KEY", "B_KEY"),
            ("a/first", "A_KEY", "A_KEY"),
        ]),
    );
    let text = content(&w, WORKFLOW);
    assert!(text.contains("config.model: b/second"), "{text}");
    assert!(
        text.contains(r#"config.fallback_models: '["a/first"]'"#),
        "{text}"
    );
}

#[test]
fn one_provider_has_no_fallbacks() {
    let w = render(&[], &providers(&[("only/one", "ONE_KEY", "ONE_KEY")]));
    let text = content(&w, WORKFLOW);
    assert!(text.contains("config.model: only/one"), "{text}");
    assert!(text.contains("config.fallback_models: '[]'"), "{text}");
    assert!(text.contains("ONE_KEY: ${{ secrets.ONE_KEY }}"), "{text}");
}

#[test]
fn two_models_of_one_provider_share_one_key() {
    let w = render(
        &[],
        &providers(&[("p/big", "P_KEY", "P_KEY"), ("p/small", "P_KEY", "P_KEY")]),
    );
    let text = content(&w, WORKFLOW);
    assert_eq!(
        text.matches("P_KEY: ${{ secrets.P_KEY }}").count(),
        2,
        "once in the key check and once in the action step: {text}"
    );
    assert!(
        text.contains(r#"config.fallback_models: '["p/small"]'"#),
        "{text}"
    );
}

#[test]
fn the_workflow_is_advisory_and_runs_for_pull_requests_not_for_their_target() {
    let w = render(&[], "");
    let text = content(&w, WORKFLOW);
    assert!(text.contains("pull_request:"), "{text}");
    assert!(!text.contains("pull_request_target"), "{text}");
    assert!(text.contains("continue-on-error: true"), "{text}");
    assert!(
        text.contains("github_action_config.auto_improve: 'false'")
            || text.contains("github_action_config.auto_improve: \"false\""),
        "{text}"
    );
    assert!(text.contains("timeout-minutes: 10"), "{text}");
    assert!(text.contains("cancel-in-progress: true"), "{text}");
    assert!(text.contains("github.event.sender.type != 'Bot'"), "{text}");
    assert!(
        text.contains("github.event.pull_request.head.repo.fork != true"),
        "{text}"
    );
    assert!(
        text.contains("github.event.pull_request.draft != true"),
        "{text}"
    );
}

#[test]
fn the_image_is_pinned_by_a_digest() {
    let w = render(&[], "");
    let text = content(&w, WORKFLOW);
    let uses = text
        .lines()
        .find(|l| l.trim_start().starts_with("uses: docker://"))
        .unwrap_or_else(|| panic!("no docker step: {text}"));
    assert!(
        regex::Regex::new(r"uses: docker://pragent/pr-agent@sha256:[0-9a-f]{64}$")
            .unwrap()
            .is_match(uses.trim()),
        "{uses}"
    );
}

#[test]
fn the_key_check_never_prints_a_key() {
    let w = render(&[], "");
    let text = content(&w, WORKFLOW);
    assert!(
        !text.contains("echo \"$GEMINI") && !text.contains("echo \"${!name}\""),
        "{text}"
    );
    assert!(text.contains("[ -n \"${!name}\" ]"), "{text}");
}

#[test]
fn the_pr_agent_settings_carry_the_review_instructions_and_the_ignore_list() {
    let w = render(&[], "skills = false");
    let toml = content(&w, TOML);
    assert!(
        toml.contains("[pr_reviewer]") && toml.contains("extra_instructions"),
        "{toml}"
    );
    assert!(
        toml.contains("[ignore]")
            && toml.contains(r#"glob = ["target/**", "**/*.lock", "Cargo.lock"]"#),
        "{toml}"
    );
    let w = render(&[], "skills = true");
    assert!(content(&w, TOML).contains(r#""skills/demo-skill/**""#));
}

#[test]
fn the_providers_are_checked() {
    let refused = |extra: String, needle: &str| {
        let message = error(&extra);
        assert!(message.contains(needle), "{needle} not in {message}");
    };
    refused(
        "prAgentProviders = new Listing<Provider> {}".into(),
        "is empty",
    );
    refused(
        providers(&[("a/x", "lower_case", "A")]),
        "not a secret name",
    );
    refused(
        providers(&[("a/x", "A_KEY", "has space")]),
        "not a variable name",
    );
    refused(
        providers(&[("a/x\\\"y", "A_KEY", "A_KEY")]),
        "not a plain model id",
    );
    refused(
        providers(&[("a/x", "A_KEY", "A_KEY"), ("a/x", "A_KEY", "A_KEY")]),
        "same model twice",
    );
    refused(
        providers(&[("a/x", "A_KEY", "SHARED"), ("a/y", "B_KEY", "SHARED")]),
        "one env to two different secrets",
    );
}

#[test]
fn the_comment_trigger_is_ignored_by_zizmor_with_the_reason_on_the_on_line() {
    let w = render(&[], "");
    let text = content(&w, WORKFLOW);
    let on_line = text
        .lines()
        .find(|l| l.starts_with("'on':"))
        .unwrap_or_else(|| panic!("no on: line: {text}"));
    assert!(
        on_line.contains("zizmor: ignore[dangerous-triggers]"),
        "{on_line}"
    );
    assert!(
        on_line.contains("checks out no pull request code"),
        "{on_line}"
    );
}
