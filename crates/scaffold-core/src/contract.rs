//! The contract: the one JSON document the pkl package renders and the runner reads.

use serde::{Deserialize, Serialize};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 2;

/// The schema the pkl package is held to, compiled in so the runner needs no file next to it.
const SCHEMA: &str = include_str!("../../../contract.schema.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileClass {
    Owned,
    Seed,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    #[serde(rename = "0644")]
    Regular,
    #[serde(rename = "0755")]
    Executable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractFile {
    pub path: String,
    pub class: FileClass,
    pub mode: Mode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Jinja source the runner renders with `context`. After `parse_contract` it is gone and `content` holds the result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// The values the template reads. Only allowed with `template`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove_on_detach: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    pub action: String,
    pub sha: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscapeHatch {
    pub uses: u64,
    pub constructs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    pub runner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityBuiltin {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contract {
    pub schema_version: u32,
    pub files: Vec<ContractFile>,
    pub pins: Vec<Pin>,
    pub escape_hatch: EscapeHatch,
    /// Where the package was loaded from, so the runner can read its version. Absent for a local package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_uri: Option<String>,
    /// The oldest runner that can render this package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<Requires>,
    /// Builtins owned by Security: the runner warns when one is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_builtins: Option<Vec<SecurityBuiltin>>,
    /// The Preset files the config applies, by path under `presets/`. Absent when it applies none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presets: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ContractError(pub String);

impl std::fmt::Display for ContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ContractError {}

/// Checks the schema version first, so an unknown version gets a clear message, then the structure and the rules the
/// schema cannot say.
pub fn parse_contract(json: &serde_json::Value) -> Result<Contract, ContractError> {
    let version = json.get("schemaVersion");
    let supported = f64::from(SUPPORTED_SCHEMA_VERSION);
    match version.and_then(serde_json::Value::as_f64) {
        Some(v) if v == supported => {}
        Some(v) if v > supported => {
            return Err(ContractError(format!(
                "contract schemaVersion {v} is newer than this runner supports ({SUPPORTED_SCHEMA_VERSION}): the package needs a newer runner"
            )));
        }
        _ => {
            let shown = version.map_or_else(|| "undefined".to_owned(), ToString::to_string);
            return Err(ContractError(format!(
                "unsupported contract schemaVersion {shown}, this runner supports {SUPPORTED_SCHEMA_VERSION}"
            )));
        }
    }
    let schema: serde_json::Value =
        serde_json::from_str(SCHEMA).expect("contract.schema.json is valid JSON");
    let problems = crate::schema::validate(json, &schema, "#");
    if !problems.is_empty() {
        return Err(ContractError(format!(
            "contract does not match the schema:\n  {}",
            problems.join("\n  ")
        )));
    }
    let mut contract: Contract = serde_json::from_value(json.clone())
        .map_err(|e| ContractError(format!("contract does not match the schema:\n  {e}")))?;
    for f in &mut contract.files {
        let has_text = f.content.is_some() || f.template.is_some();
        if f.class == FileClass::External && has_text {
            return Err(ContractError(format!(
                "{}: an external file must not carry content",
                f.path
            )));
        }
        if f.class != FileClass::External && !has_text {
            let article = "a";
            let class = if f.class == FileClass::Owned {
                "owned"
            } else {
                "seed"
            };
            return Err(ContractError(format!(
                "{}: {article} {class} file needs content",
                f.path
            )));
        }
        if f.content.is_some() && f.template.is_some() {
            return Err(ContractError(format!(
                "{}: a file carries content or a template, not both",
                f.path
            )));
        }
        if f.context.is_some() && f.template.is_none() {
            return Err(ContractError(format!(
                "{}: a context needs a template",
                f.path
            )));
        }
        if f.remove_on_detach == Some(true) && f.class != FileClass::Owned {
            return Err(ContractError(format!(
                "{}: removeOnDetach is only allowed on owned files",
                f.path
            )));
        }
    }
    for f in &mut contract.files {
        if let Some(source) = f.template.take() {
            let context = f.context.take().unwrap_or_default();
            f.content = Some(crate::template::render(&f.path, &source, &context)?);
        }
    }
    let mut seen = std::collections::HashSet::new();
    if let Some(dup) = contract
        .files
        .iter()
        .find(|f| !seen.insert(f.path.as_str()))
    {
        return Err(ContractError(format!("{}: listed twice", dup.path)));
    }
    let mut seen = std::collections::HashSet::new();
    if let Some(dup) = contract
        .presets
        .iter()
        .flatten()
        .find(|p| !seen.insert(p.as_str()))
    {
        return Err(ContractError(format!("{dup}: listed twice")));
    }
    Ok(contract)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn base() -> Value {
        json!({
            "schemaVersion": 2,
            "files": [
                { "path": ".github/workflows/ci.yml", "class": "owned", "mode": "0644", "content": "name: ci\n" },
                { "path": "scripts/run.sh", "class": "seed", "mode": "0755", "content": "#!/bin/sh\n" },
                { "path": ".github/CODEOWNERS", "class": "external", "mode": "0644" }
            ],
            "pins": [{ "action": "actions/checkout", "sha": "0123456789abcdef0123456789abcdef01234567", "version": "v4.2.2" }],
            "escapeHatch": { "uses": 0, "constructs": 0 }
        })
    }

    fn err(json: &Value) -> String {
        parse_contract(json).unwrap_err().to_string()
    }

    #[test]
    fn a_valid_contract_parses_and_keeps_every_file() {
        let c = parse_contract(&base()).unwrap();
        assert_eq!(c.files.len(), 3);
        assert_eq!(c.files[1].mode, Mode::Executable);
        assert_eq!(c.files[2].class, FileClass::External);
        assert_eq!(c.pins[0].version, "v4.2.2");
    }

    #[test]
    fn the_optional_fields_are_read() {
        let mut j = base();
        j["packageUri"] = json!("package://example.invalid/pantheon@0.2.0");
        j["requires"] = json!({ "runner": "0.2.0" });
        j["securityBuiltins"] = json!([{ "name": "codeql", "enabled": false }]);
        let c = parse_contract(&j).unwrap();
        assert_eq!(c.requires.unwrap().runner, "0.2.0");
        assert!(!c.security_builtins.unwrap()[0].enabled);
        assert!(c.package_uri.is_some());
    }

    #[test]
    fn a_contract_serialises_back_to_the_same_json() {
        let j = base();
        assert_eq!(
            serde_json::to_value(parse_contract(&j).unwrap()).unwrap(),
            j
        );
    }

    #[test]
    fn an_unknown_schema_version_is_refused_before_anything_else() {
        assert_eq!(
            err(&json!({ "schemaVersion": 3 })),
            "contract schemaVersion 3 is newer than this runner supports (2): the package needs a newer runner"
        );
        assert_eq!(
            err(&json!({})),
            "unsupported contract schemaVersion undefined, this runner supports 2"
        );
        assert_eq!(
            err(&Value::Null),
            "unsupported contract schemaVersion undefined, this runner supports 2"
        );
    }

    #[test]
    fn a_contract_that_does_not_match_the_schema_is_refused_with_every_problem_listed() {
        let mut j = base();
        j["files"][0]["class"] = json!("weird");
        j["files"][0]["mode"] = json!("0777");
        j["pins"][0]["sha"] = json!("abc");
        j["extra"] = json!(true);
        assert_eq!(
            err(&j),
            "contract does not match the schema:\n  \
             #/files/0/class: must be one of [\"owned\",\"seed\",\"external\"]\n  \
             #/files/0/mode: must be one of [\"0644\",\"0755\"]\n  \
             #/pins/0/sha: does not match ^[0-9a-f]{40}$\n  #: unexpected extra"
        );
    }

    #[test]
    fn the_schema_rejects_a_missing_required_key() {
        let mut j = base();
        j.as_object_mut().unwrap().remove("escapeHatch");
        assert_eq!(
            err(&j),
            "contract does not match the schema:\n  #: missing escapeHatch"
        );
    }

    #[test]
    fn an_external_file_must_not_carry_content() {
        let mut j = base();
        j["files"][2]["content"] = json!("x");
        assert_eq!(
            err(&j),
            ".github/CODEOWNERS: an external file must not carry content"
        );
    }

    #[test]
    fn an_owned_or_seed_file_needs_content() {
        let mut j = base();
        j["files"][0].as_object_mut().unwrap().remove("content");
        assert_eq!(
            err(&j),
            ".github/workflows/ci.yml: a owned file needs content"
        );
        let mut j = base();
        j["files"][1].as_object_mut().unwrap().remove("content");
        assert_eq!(err(&j), "scripts/run.sh: a seed file needs content");
    }

    #[test]
    fn remove_on_detach_is_only_allowed_on_owned_files() {
        let mut j = base();
        j["files"][1]["removeOnDetach"] = json!(true);
        assert_eq!(
            err(&j),
            "scripts/run.sh: removeOnDetach is only allowed on owned files"
        );
        let mut j = base();
        j["files"][1]["removeOnDetach"] = json!(false);
        assert!(parse_contract(&j).is_ok());
        let mut j = base();
        j["files"][0]["removeOnDetach"] = json!(true);
        assert!(parse_contract(&j).is_ok());
    }

    #[test]
    fn a_path_listed_twice_is_refused() {
        let mut j = base();
        j["files"][1]["path"] = json!(".github/workflows/ci.yml");
        assert_eq!(err(&j), ".github/workflows/ci.yml: listed twice");
    }
    #[test]
    fn the_presets_a_config_applies_are_read_and_written_back() {
        let mut j = base();
        j["presets"] = json!(["presets/ci.pkl", "presets/extra/more.pkl"]);
        let c = parse_contract(&j).unwrap();
        assert_eq!(
            c.presets.as_deref(),
            Some(
                &[
                    "presets/ci.pkl".to_owned(),
                    "presets/extra/more.pkl".to_owned()
                ][..]
            )
        );
        assert_eq!(serde_json::to_value(c).unwrap(), j);
    }

    #[test]
    fn a_contract_without_presets_has_none_and_writes_none() {
        let c = parse_contract(&base()).unwrap();
        assert_eq!(c.presets, None);
        assert!(serde_json::to_value(c).unwrap().get("presets").is_none());
    }

    #[test]
    fn a_preset_must_be_a_pkl_file_under_presets() {
        for bad in [
            "ci.pkl",
            "presets/ci.yml",
            "presets/",
            "other/presets/ci.pkl",
            "presets/../ci.pkl",
            "/presets/ci.pkl",
        ] {
            let mut j = base();
            j["presets"] = json!([bad]);
            let message = err(&j);
            assert!(
                message.starts_with(
                    "contract does not match the schema:\n  #/presets/0: does not match"
                ),
                "{bad}: {message}"
            );
        }
    }

    #[test]
    fn a_preset_listed_twice_is_refused() {
        let mut j = base();
        j["presets"] = json!(["presets/ci.pkl", "presets/ci.pkl"]);
        assert_eq!(err(&j), "presets/ci.pkl: listed twice");
    }

    #[test]
    fn presets_must_be_a_list_of_strings() {
        let mut j = base();
        j["presets"] = json!("presets/ci.pkl");
        assert_eq!(
            err(&j),
            "contract does not match the schema:\n  #/presets: must be array, got string"
        );
        j["presets"] = json!([3]);
        assert_eq!(
            err(&j),
            "contract does not match the schema:\n  #/presets/0: must be string, got integer"
        );
    }

    fn templated(extra: Value) -> Value {
        let mut j = base();
        let mut file = json!({ "path": "rust-toolchain.toml", "class": "owned", "mode": "0644" });
        for (k, v) in extra.as_object().unwrap() {
            file[k] = v.clone();
        }
        j["files"].as_array_mut().unwrap().push(file);
        j
    }

    #[test]
    fn a_template_is_rendered_with_its_context_when_the_contract_is_parsed() {
        let j = templated(json!({
            "template": "channel = \"{{ toolchain }}\"\n",
            "context": { "toolchain": "1.96.1" }
        }));
        let c = parse_contract(&j).unwrap();
        let f = c.files.last().unwrap();
        assert_eq!(f.content.as_deref(), Some("channel = \"1.96.1\"\n"));
        assert_eq!((f.template.as_ref(), f.context.as_ref()), (None, None));
    }

    #[test]
    fn a_template_without_a_context_renders_with_none() {
        let c = parse_contract(&templated(json!({ "template": "plain\n" }))).unwrap();
        assert_eq!(c.files.last().unwrap().content.as_deref(), Some("plain\n"));
    }

    #[test]
    fn a_file_with_content_and_a_template_is_refused() {
        let j = templated(json!({ "content": "x\n", "template": "y\n" }));
        assert_eq!(
            err(&j),
            "rust-toolchain.toml: a file carries content or a template, not both"
        );
    }

    #[test]
    fn a_context_without_a_template_is_refused() {
        let j = templated(json!({ "content": "x\n", "context": { "a": "b" } }));
        assert_eq!(err(&j), "rust-toolchain.toml: a context needs a template");
    }

    #[test]
    fn an_external_file_must_not_carry_a_template() {
        let mut j = base();
        j["files"][2]["template"] = json!("x\n");
        assert_eq!(
            err(&j),
            ".github/CODEOWNERS: an external file must not carry content"
        );
    }

    #[test]
    fn a_template_that_cannot_render_refuses_the_whole_contract() {
        let j = templated(json!({ "template": "a = {{ missing }}\n", "context": {} }));
        let e = err(&j);
        assert!(
            e.starts_with("rust-toolchain.toml: template error: undefined value"),
            "{e}"
        );
    }

    #[test]
    fn a_contract_from_an_older_version_is_refused() {
        let mut j = base();
        j["schemaVersion"] = json!(1);
        assert_eq!(
            err(&j),
            "unsupported contract schemaVersion 1, this runner supports 2"
        );
    }
}
