use crate::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const POLICY_SCHEMA: &str = "adrproof-diagnostic-policy-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticPolicy {
    pub schema_version: String,
    #[serde(default)]
    pub waivers: Vec<Waiver>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Waiver {
    pub id: String,
    pub finding_id: String,
    pub owner: String,
    pub reason: String,
    pub expires_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedWaiver {
    pub waiver_id: String,
    pub finding_id: String,
    pub owner: String,
    pub reason: String,
    pub expires_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyAssessment {
    pub findings: Vec<Value>,
    pub applied_waivers: Vec<AppliedWaiver>,
    pub unwaived_finding_count: usize,
    pub diagnostics: Vec<String>,
}

pub fn load(path: &Path) -> Result<DiagnosticPolicy, Error> {
    let bytes = fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let policy: DiagnosticPolicy = serde_json::from_slice(&bytes)
        .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))?;
    validate(&policy)?;
    Ok(policy)
}

pub fn validate(policy: &DiagnosticPolicy) -> Result<(), Error> {
    if policy.schema_version != POLICY_SCHEMA {
        return Err(Error::ProviderFailure(format!(
            "expected policy schema {POLICY_SCHEMA}, observed {}",
            policy.schema_version
        )));
    }
    let mut ids = BTreeSet::new();
    let mut finding_ids = BTreeSet::new();
    for waiver in &policy.waivers {
        if waiver.id.trim().is_empty()
            || waiver.finding_id.trim().is_empty()
            || waiver.owner.trim().is_empty()
            || waiver.reason.trim().is_empty()
        {
            return Err(Error::ProviderFailure(
                "waiver id, finding_id, owner, and reason must be non-empty".into(),
            ));
        }
        if !ids.insert(&waiver.id) {
            return Err(Error::ProviderFailure(format!(
                "duplicate waiver id {}",
                waiver.id
            )));
        }
        if !finding_ids.insert(&waiver.finding_id) {
            return Err(Error::ProviderFailure(format!(
                "multiple waivers target finding {}",
                waiver.finding_id
            )));
        }
    }
    Ok(())
}

pub fn apply(
    findings: Vec<Value>,
    policy: &DiagnosticPolicy,
    now_unix_seconds: u64,
) -> PolicyAssessment {
    let by_finding = policy
        .waivers
        .iter()
        .filter(|waiver| waiver.expires_unix_seconds > now_unix_seconds)
        .map(|waiver| (waiver.finding_id.as_str(), waiver))
        .collect::<BTreeMap<_, _>>();
    let expired = policy
        .waivers
        .iter()
        .filter(|waiver| waiver.expires_unix_seconds <= now_unix_seconds)
        .map(|waiver| waiver.id.clone())
        .collect::<Vec<_>>();
    let finding_ids = findings
        .iter()
        .filter_map(|finding| finding.get("id").and_then(Value::as_str))
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut applied_waivers = Vec::new();
    let mut assessed = Vec::new();
    let mut unwaived_finding_count = 0;
    for mut finding in findings {
        let id = finding
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if let Some(waiver) = by_finding.get(id) {
            let applied = AppliedWaiver {
                waiver_id: waiver.id.clone(),
                finding_id: waiver.finding_id.clone(),
                owner: waiver.owner.clone(),
                reason: waiver.reason.clone(),
                expires_unix_seconds: waiver.expires_unix_seconds,
            };
            if let Some(object) = finding.as_object_mut() {
                object.insert(
                    "waiver".into(),
                    serde_json::to_value(&applied).expect("waiver serialization"),
                );
            }
            applied_waivers.push(applied);
        } else {
            unwaived_finding_count += 1;
        }
        assessed.push(finding);
    }
    let mut diagnostics = expired
        .into_iter()
        .map(|id| format!("waiver {id} is expired"))
        .collect::<Vec<_>>();
    diagnostics.extend(
        policy
            .waivers
            .iter()
            .filter(|waiver| {
                waiver.expires_unix_seconds > now_unix_seconds
                    && !finding_ids.contains(waiver.finding_id.as_str())
            })
            .map(|waiver| format!("waiver {} does not match a current finding", waiver.id)),
    );
    PolicyAssessment {
        findings: assessed,
        applied_waivers,
        unwaived_finding_count,
        diagnostics,
    }
}

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub fn sarif(findings: &[Value]) -> Value {
    let results = findings
        .iter()
        .map(|finding| {
            let id = finding.get("id").and_then(Value::as_str).unwrap_or("ADRPROOF");
            let kind = finding
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("verification");
            let source = finding
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("specification");
            let status = finding
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("ATTENTION");
            let waived = finding.get("waiver").is_some();
            let mut result = serde_json::json!({
                "ruleId": id,
                "level": if status == "FAIL" || status == "ERROR" { "error" } else { "warning" },
                "message": {"text": format!("{kind} {id}: {status}")},
                "locations": [{"physicalLocation":{"artifactLocation":{"uri":source}}}],
                "properties": {"adrproofFinding": finding},
            });
            if waived {
                result.as_object_mut().expect("SARIF result object").insert(
                    "suppressions".into(),
                    serde_json::json!([{"kind":"external","status":"accepted","justification":"ADRProof time-bounded waiver"}]),
                );
            }
            result
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "version": "2.1.0",
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "runs": [{
            "tool": {"driver": {"name":"ADRProof","semanticVersion":env!("CARGO_PKG_VERSION")}},
            "results": results
        }]
    })
}
