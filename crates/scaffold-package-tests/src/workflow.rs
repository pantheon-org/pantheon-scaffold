//! A workflow file of this repository, parsed, for the tests that assert the properties that make it safe to run.

use serde_json::Value;

use crate::parse_yaml;
use crate::pkg::read;

pub struct Workflow {
    pub text: String,
    pub doc: Value,
}

impl Workflow {
    /// Reads and parses a workflow, named from the root of the repository.
    pub fn load(rel: &str) -> Self {
        let text = read(rel);
        let doc = parse_yaml(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
        Self { text, doc }
    }

    pub fn jobs(&self) -> Vec<(&str, &Value)> {
        self.doc["jobs"]
            .as_object()
            .expect("a workflow has jobs")
            .iter()
            .map(|(name, job)| (name.as_str(), job))
            .collect()
    }

    pub fn job(&self, name: &str) -> &Value {
        let job = &self.doc["jobs"][name];
        assert!(!job.is_null(), "no job {name}");
        job
    }

    pub fn steps(&self, job: &str) -> &[Value] {
        steps_of(self.job(job))
    }

    /// Every step of every job, with the name of its job.
    pub fn all_steps(&self) -> Vec<(&str, &Value)> {
        self.jobs()
            .into_iter()
            .flat_map(|(name, job)| steps_of(job).iter().map(move |s| (name, s)))
            .collect()
    }
}

pub fn steps_of(job: &Value) -> &[Value] {
    job["steps"].as_array().map_or(&[], Vec::as_slice)
}

/// The jobs a job needs, whether `needs` is one name or a list.
pub fn needs(job: &Value) -> Vec<String> {
    match &job["needs"] {
        Value::String(one) => vec![one.clone()],
        Value::Array(list) => list
            .iter()
            .map(|n| n.as_str().unwrap().to_owned())
            .collect(),
        _ => vec![],
    }
}

/// The `run` text of a step, empty when it has none.
pub fn run_of(step: &Value) -> &str {
    step["run"].as_str().unwrap_or("")
}

/// The `uses` of a step, empty when it has none.
pub fn uses_of(step: &Value) -> &str {
    step["uses"].as_str().unwrap_or("")
}

/// The permissions of a job that are `write`, as `(permission, value)` pairs.
pub fn writes(job: &Value) -> Vec<(String, String)> {
    job["permissions"]
        .as_object()
        .map(|p| {
            p.iter()
                .filter(|(_, v)| *v == "write")
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}
