//! The contract: the one JSON document the pkl package renders and the runner reads.

use serde::{Deserialize, Serialize};

pub const SUPPORTED_SCHEMA_VERSION: u32 = 1;

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
    if !version.is_some_and(|v| v.as_f64() == Some(f64::from(SUPPORTED_SCHEMA_VERSION))) {
        let shown = version.map_or_else(|| "undefined".to_owned(), ToString::to_string);
        return Err(ContractError(format!(
            "unsupported contract schemaVersion {shown}, this runner supports {SUPPORTED_SCHEMA_VERSION}"
        )));
    }
    let contract: Contract = serde_json::from_value(json.clone())
        .map_err(|e| ContractError(format!("contract does not match the schema:\n  {e}")))?;
    for f in &contract.files {
        match (f.class, &f.content) {
            (FileClass::External, Some(_)) => {
                return Err(ContractError(format!(
                    "{}: an external file must not carry content",
                    f.path
                )));
            }
            (FileClass::Owned, None) => {
                return Err(ContractError(format!(
                    "{}: a owned file needs content",
                    f.path
                )));
            }
            (FileClass::Seed, None) => {
                return Err(ContractError(format!(
                    "{}: a seed file needs content",
                    f.path
                )));
            }
            _ => {}
        }
        if f.remove_on_detach == Some(true) && f.class != FileClass::Owned {
            return Err(ContractError(format!(
                "{}: removeOnDetach is only allowed on owned files",
                f.path
            )));
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
    Ok(contract)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn base() -> Value {
        json!({
            "schemaVersion": 1,
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
            err(&json!({ "schemaVersion": 2 })),
            "unsupported contract schemaVersion 2, this runner supports 1"
        );
        assert_eq!(
            err(&json!({})),
            "unsupported contract schemaVersion undefined, this runner supports 1"
        );
        assert_eq!(
            err(&Value::Null),
            "unsupported contract schemaVersion undefined, this runner supports 1"
        );
    }

    #[test]
    fn a_contract_that_does_not_match_the_structure_is_refused() {
        let mut j = base();
        j["files"][0]["class"] = json!("weird");
        assert!(err(&j).starts_with("contract does not match the schema:"));
        let mut j = base();
        j["extra"] = json!(true);
        assert!(err(&j).starts_with("contract does not match the schema:"));
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
}
