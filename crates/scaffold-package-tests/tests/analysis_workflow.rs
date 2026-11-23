//! options.workflowLayout["analysis"] = "merged": zizmor, agnix, aislop and plumber as the jobs of one workflow, `analysis.yml`. features/analysis.feature
//! shows the behaviour through the runner. These tests compare the merged jobs with the four separate workflows, so a merge cannot change a step, a
//! job name (the required status checks) or a permission without a test failing.

use scaffold_package_tests::parse_yaml;
use scaffold_package_tests::pkg::{actionlint, content, eval, jobs, root};
use scaffold_package_tests::repo::temp_dir;
use serde_json::{Value, json};
use std::process::Command;

const TOOLS: [&str; 4] = ["zizmor", "agnix", "aislop", "plumber"];

fn render(options: &str) -> Value {
    let text = format!(
        "amends \"file://{root}/pkl/Config.pkl\"\nimport \"file://{root}/pkl/Builtins.pkl\"\n\
         options {{ toolchain = \"1.96.1\"; runnerVersion = \"0.4.0\"; skills = false; binary = false; {options} }}\n\
         workflows {{ ...Builtins.standardWorkflows(options) }}\nfiles {{ ...Builtins.standardFiles(options) }}\n",
        root = root().to_string_lossy()
    );
    let dir = temp_dir();
    let config = dir.path().join("analysis.pkl");
    std::fs::write(&config, text).unwrap();
    eval(&config).unwrap_or_else(|e| panic!("{options}: {e}"))
}

fn error_of(options: &str) -> String {
    let text = format!(
        "amends \"file://{root}/pkl/Config.pkl\"\nimport \"file://{root}/pkl/Builtins.pkl\"\n\
         options {{ toolchain = \"1.96.1\"; runnerVersion = \"0.4.0\"; skills = false; binary = false; {options} }}\n\
         workflows {{ ...Builtins.standardWorkflows(options) }}\n",
        root = root().to_string_lossy()
    );
    let dir = temp_dir();
    let config = dir.path().join("analysis.pkl");
    std::fs::write(&config, text).unwrap();
    eval(&config).expect_err("the options are not valid")
}

fn merged(extra: &str) -> Value {
    render(&format!(
        "workflowLayout {{ [\"analysis\"] = \"merged\" }}; {extra}"
    ))
}

fn workflow(contract: &Value, name: &str) -> Value {
    parse_yaml(content(contract, &format!(".github/workflows/{name}.yml")))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn has_file(contract: &Value, path: &str) -> bool {
    contract["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["path"] == path)
}

/// The id of the job a tool has in the merged workflow. aislop's job is `quality-gate` in both layouts.
fn job_id(tool: &str) -> &str {
    if tool == "aislop" {
        "quality-gate"
    } else {
        tool
    }
}

#[test]
fn the_default_renders_the_four_separate_workflows_and_no_analysis_workflow() {
    let contract = render("");
    for tool in TOOLS {
        assert!(
            has_file(&contract, &format!(".github/workflows/{tool}.yml")),
            "{tool}"
        );
    }
    assert!(!has_file(&contract, ".github/workflows/analysis.yml"));
    // An empty layout and a layout that says `separate` are the same render.
    let explicit = render("workflowLayout { [\"analysis\"] = \"separate\" }");
    for tool in TOOLS {
        let path = format!(".github/workflows/{tool}.yml");
        assert_eq!(
            content(&contract, &path),
            content(&explicit, &path),
            "{tool}"
        );
    }
}

#[test]
fn the_merged_jobs_keep_the_ids_names_and_steps_of_the_separate_workflows() {
    let separate = render("");
    let both = merged("");
    let analysis = workflow(&both, "analysis");
    for tool in TOOLS {
        let alone = workflow(&separate, tool);
        let (id, before) = alone["jobs"].as_object().unwrap().iter().next().unwrap();
        assert_eq!(id, job_id(tool), "{tool}: the job id");
        let after = &analysis["jobs"][id];
        for key in ["name", "runs-on", "timeout-minutes", "steps"] {
            assert_eq!(after[key], before[key], "{tool}: {key}");
        }
    }
    assert_eq!(analysis["jobs"].as_object().unwrap().len(), 4);
}

#[test]
fn the_merged_job_names_are_the_names_of_the_required_status_checks() {
    let names: Vec<(String, Option<String>)> =
        jobs(content(&merged(""), ".github/workflows/analysis.yml"));
    let expected = [
        ("zizmor", "zizmor"),
        ("agnix", "agnix"),
        ("quality-gate", "aislop quality gate"),
        ("plumber", "Plumber Score"),
    ];
    assert_eq!(
        names,
        expected
            .iter()
            .map(|(id, name)| (id.to_string(), Some(name.to_string())))
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_workflow_holds_only_read_and_each_job_holds_the_scopes_its_tool_had() {
    let separate = render("");
    let analysis = workflow(&merged(""), "analysis");
    assert_eq!(analysis["permissions"], json!({ "contents": "read" }));
    assert!(analysis.get("env").is_none(), "no workflow-level env");
    // agnix wrote at workflow level, plumber on its job; the other two read.
    assert_eq!(
        analysis["jobs"]["agnix"]["permissions"],
        workflow(&separate, "agnix")["permissions"]
    );
    assert_eq!(
        analysis["jobs"]["plumber"]["permissions"],
        workflow(&separate, "plumber")["jobs"]["plumber"]["permissions"]
    );
    for id in ["zizmor", "quality-gate"] {
        assert_eq!(
            analysis["jobs"][id]["permissions"],
            json!({ "contents": "read" }),
            "{id}"
        );
    }
    // agnix read AGNIX_VERSION and AGNIX_SHA256 from the workflow; the job carries them now.
    assert_eq!(
        analysis["jobs"]["agnix"]["env"],
        workflow(&separate, "agnix")["env"]
    );
}

#[test]
fn the_schedule_runs_plumber_alone_and_agnix_keeps_its_fork_guard_in_brackets() {
    let analysis = workflow(&merged(""), "analysis");
    assert_eq!(
        analysis["on"]["schedule"],
        json!([{ "cron": "41 3 * * 1" }])
    );
    let guard = "github.event_name != 'schedule'";
    assert_eq!(analysis["jobs"]["zizmor"]["if"], guard);
    assert_eq!(analysis["jobs"]["quality-gate"]["if"], guard);
    assert_eq!(
        analysis["jobs"]["agnix"]["if"],
        format!(
            "({guard}) && (github.event_name == 'push' || github.event.pull_request.head.repo.fork == false)"
        )
    );
    assert!(analysis["jobs"]["plumber"].get("if").is_none());
}

#[test]
fn without_plumber_there_is_no_schedule_and_no_guard() {
    let analysis = workflow(&merged("without { \"plumber\" }"), "analysis");
    assert!(analysis["on"].get("schedule").is_none());
    assert!(analysis["jobs"]["zizmor"].get("if").is_none());
    assert!(analysis["jobs"]["quality-gate"].get("if").is_none());
    assert_eq!(
        analysis["jobs"]["agnix"]["if"],
        "github.event_name == 'push' || github.event.pull_request.head.repo.fork == false"
    );
    assert!(analysis["jobs"].get("plumber").is_none());
}

#[test]
fn leaving_every_tool_out_renders_no_analysis_workflow_and_no_separate_one() {
    let contract = merged("without { \"zizmor\"; \"agnix\"; \"aislop\"; \"plumber\" }");
    for name in ["analysis", "zizmor", "agnix", "aislop", "plumber"] {
        assert!(
            !has_file(&contract, &format!(".github/workflows/{name}.yml")),
            "{name}"
        );
    }
}

#[test]
fn the_merged_workflow_passes_actionlint() {
    let contract = merged("");
    let text = content(&contract, ".github/workflows/analysis.yml");
    assert_eq!(actionlint(&[(".github/workflows/analysis.yml", text)]), "");
}

#[test]
fn dependabot_counts_the_tools_left_out_of_a_merged_workflow() {
    let entry = |contract: &Value| -> Option<Value> {
        let doc = parse_yaml(content(contract, ".github/dependabot.yml")).unwrap();
        doc["updates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|u| u["package-ecosystem"] == "github-actions")
            .cloned()
    };
    // Nothing is left out: every workflow is rendered, so there is nothing for the entry to scan.
    assert!(entry(&merged("")).is_none());
    // zizmor is left out: its workflow is the repository's own, outside the exclude list.
    let kept = entry(&merged("without { \"zizmor\" }")).expect("the github-actions entry");
    let excluded = kept["exclude-paths"].as_array().unwrap();
    assert!(excluded.contains(&json!(".github/workflows/analysis.yml")));
    assert!(!excluded.contains(&json!(".github/workflows/zizmor.yml")));
}

#[test]
fn a_layout_key_that_is_not_a_recipe_names_it_and_lists_the_recipes() {
    let error = error_of("workflowLayout { [\"analyses\"] = \"merged\" }");
    assert!(
        error.contains("\"analyses\", which is not a workflow recipe of this package version"),
        "{error}"
    );
    assert!(error.contains("Recipes: analysis"), "{error}");
}

#[test]
fn a_layout_value_that_is_not_a_layout_is_refused() {
    let error = error_of("workflowLayout { [\"analysis\"] = \"both\" }");
    assert!(
        error.contains("separate") && error.contains("merged"),
        "{error}"
    );
}

/// The contract of a merged repository whose workflows are `workflows_expr`, an expression over `Builtins.standardWorkflows(options)` and `Presets`.
fn merged_with_preset(workflows_expr: &str) -> Result<Value, String> {
    let text = format!(
        "amends \"file://{root}/pkl/Config.pkl\"\nimport \"file://{root}/pkl/Builtins.pkl\"\nimport \"file://{root}/pkl/Presets.pkl\"\n\
         options {{ toolchain = \"1.96.1\"; runnerVersion = \"0.4.0\"; skills = false; binary = false; workflowLayout {{ [\"analysis\"] = \"merged\" }} }}\n\
         workflows = {workflows_expr}\n",
        root = root().to_string_lossy()
    );
    let dir = temp_dir();
    let config = dir.path().join("preset.pkl");
    std::fs::write(&config, text).unwrap();
    eval(&config)
}

#[test]
fn a_preset_amends_a_job_of_the_merged_workflow() {
    let contract = merged_with_preset(
        "Presets.amendWorkflow(Builtins.standardWorkflows(options), \"analysis\", (w) -> w.amendJob(\"zizmor\", (j) -> (j) { `timeout-minutes` = 33 }))",
    )
    .unwrap();
    let analysis = workflow(&contract, "analysis");
    assert_eq!(analysis["jobs"]["zizmor"]["timeout-minutes"], 33);
    assert_eq!(analysis["jobs"]["agnix"]["timeout-minutes"], 10);
}

#[test]
fn a_preset_that_names_a_tool_workflow_fails_because_the_merged_layout_moved_the_key() {
    let error = merged_with_preset(
        "Presets.amendWorkflow(Builtins.standardWorkflows(options), \"zizmor\", (w) -> w)",
    )
    .expect_err("zizmor is a job of analysis, not a workflow");
    assert!(
        error.contains("preset: workflow \"zizmor\" is not a builtin of this package version"),
        "{error}"
    );
    assert!(error.contains("analysis"), "{error}");
    assert!(
        !error.contains("plumber") && !error.contains("aislop"),
        "{error}"
    );
}

#[test]
fn compose_fails_naming_both_sources_on_a_duplicate_job_id() {
    let dir = temp_dir();
    let module = dir.path().join("compose.pkl");
    std::fs::write(
        &module,
        format!(
            "import \"file://{root}/pkl/Builtins/Shared.pkl\"\nimport \"file://{root}/pkl/Jobs.pkl\"\n\
             local jobs = new Mapping<String, Jobs.Job> {{ [\"build\"] = new Jobs.Job {{ steps = new Listing {{}} }} }}\n\
             merged = Shared.compose(List(Pair(\"zizmor\", jobs), Pair(\"agnix\", jobs)))\n",
            root = root().to_string_lossy()
        ),
    )
    .unwrap();
    let output = Command::new("pkl")
        .args(["eval", "--format", "json"])
        .arg(&module)
        .output()
        .expect("pkl runs");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("job \"build\" is in more than one part: zizmor and agnix"),
        "{stderr}"
    );
}
