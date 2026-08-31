use crate::project::{EvidenceId, ProofObligationId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::roots::SemanticInput;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum VerificationStatus {
    Pass,
    Fail,
    Unknown,
    Unverified,
    Stale,
    NotApplicable,
    Error,
}
impl VerificationStatus {
    pub fn is_ci_pass(&self) -> bool {
        matches!(self, Self::Pass)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum EvidenceValidity {
    Current,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputFingerprint {
    pub source: String,
    pub sha256: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub id: EvidenceId,
    pub obligation: ProofObligationId,
    pub backend: String,
    pub backend_version: String,
    pub configuration_sha256: String,
    pub inputs: Vec<InputFingerprint>,
    pub result_at_execution: VerificationStatus,
    pub recorded_at_unix_nanos: u128,
    pub diagnostics: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAssessment {
    pub evidence: Evidence,
    pub validity: EvidenceValidity,
}

pub fn fingerprint_files(
    root: &Path,
    paths: &[PathBuf],
) -> Result<Vec<InputFingerprint>, std::io::Error> {
    let mut unique = paths.to_vec();
    unique.sort();
    unique.dedup();
    unique
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path)?;
            let source = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            Ok(InputFingerprint {
                source,
                sha256: hash(&bytes),
            })
        })
        .collect()
}
pub fn fingerprint_semantic_files(
    inputs: &[SemanticInput],
) -> Result<Vec<InputFingerprint>, std::io::Error> {
    let mut unique = inputs.to_vec();
    unique.sort_by(|a, b| (&a.identity, &a.path).cmp(&(&b.identity, &b.path)));
    unique.dedup_by(|a, b| a.identity == b.identity);
    unique
        .into_iter()
        .map(|input| {
            let bytes = fs::read(&input.path).map_err(|error| {
                std::io::Error::new(error.kind(), format!("{}: {error}", input.path.display()))
            })?;
            Ok(InputFingerprint {
                source: input.identity,
                sha256: hash(&bytes),
            })
        })
        .collect()
}
pub fn fingerprint_bytes(source: &str, bytes: &[u8]) -> InputFingerprint {
    InputFingerprint {
        source: source.into(),
        sha256: hash(bytes),
    }
}
pub fn configuration_hash(timeout_ms: u64, flags: &[&str]) -> String {
    hash(format!("timeout_ms={timeout_ms}\nflags={}", flags.join("\n")).as_bytes())
}
pub fn assess(
    evidence: &Evidence,
    current: &[InputFingerprint],
    backend_version: &str,
    configuration_sha256: &str,
) -> EvidenceValidity {
    let old: BTreeMap<_, _> = evidence
        .inputs
        .iter()
        .map(|item| (&item.source, &item.sha256))
        .collect();
    let now: BTreeMap<_, _> = current
        .iter()
        .map(|item| (&item.source, &item.sha256))
        .collect();
    if old == now
        && evidence.backend_version.contains(backend_version)
        && evidence.configuration_sha256 == configuration_sha256
    {
        EvidenceValidity::Current
    } else {
        EvidenceValidity::Stale
    }
}
pub fn store(directory: &Path, mut evidence: Evidence) -> Result<Evidence, std::io::Error> {
    fs::create_dir_all(directory)?;
    let seed = serde_json::to_vec(&evidence).map_err(std::io::Error::other)?;
    evidence.id = EvidenceId(format!("EVIDENCE:{}", &hash(&seed)[..24]));
    let target = directory.join(format!("{}.json", evidence.id.0));
    if !target.exists() {
        let temporary = directory.join(format!(".{}.tmp", evidence.id.0));
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&evidence).map_err(std::io::Error::other)?,
        )?;
        fs::rename(temporary, target)?;
    }
    Ok(evidence)
}
pub fn load_all(directory: &Path) -> Result<Vec<Evidence>, std::io::Error> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|item| item.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().is_some_and(|value| value == "json"));
    paths.sort();
    let mut evidence = paths
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path)?;
            serde_json::from_slice::<Evidence>(&bytes).map_err(std::io::Error::other)
        })
        .collect::<Result<Vec<_>, _>>()?;
    evidence
        .sort_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)));
    Ok(evidence)
}
pub fn latest(directory: &Path) -> Result<Option<Evidence>, std::io::Error> {
    Ok(load_all(directory)?.pop())
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
