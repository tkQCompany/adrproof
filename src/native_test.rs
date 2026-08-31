use crate::Error;
use crate::evidence::{EvidenceValidity, InputFingerprint, VerificationStatus};
use crate::project::{ArtifactId, EvidenceId, GraphEdge, GraphNode, LinkKind, ProofObligationId};
use crate::roots::{SemanticInput, VerificationRoots};
use crate::scenario::{InputRoot, ScenarioInput};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PROVIDER_NAME: &str = "native_test_report";
pub const PROVIDER_VERSION: &str = "1";
pub const REPORT_SCHEMA: &str = "nextest-summary-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeTestDefinition {
    pub id: String,
    pub version: String,
    pub claim: String,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub command: String,
    pub working_directory: String,
    pub minimum_passed: u64,
    pub maximum_skipped: u64,
    #[serde(default)]
    pub required_tests: Vec<String>,
    pub inputs: Vec<ScenarioInput>,
    #[serde(default)]
    pub excluded_inputs: Vec<ScenarioInput>,
    #[serde(skip)]
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum NativeTestCaseStatus {
    Pass,
    Fail,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTestCase {
    pub name: String,
    pub status: NativeTestCaseStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeTestReport {
    pub schema_version: String,
    pub runner: String,
    pub runner_version: String,
    pub command: String,
    pub working_directory: String,
    pub result: VerificationStatus,
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub duration_seconds: f64,
    #[serde(default)]
    pub tests: Vec<NativeTestCase>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeTestNonVacuity {
    pub executed_tests: u64,
    pub required_tests: u64,
    pub observed_required_tests: u64,
    pub non_empty_execution: bool,
    pub all_required_observed_pass: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeTestEvidence {
    pub id: EvidenceId,
    pub obligation: ProofObligationId,
    pub definition_id: String,
    pub definition_version: String,
    pub provider: String,
    pub provider_version: String,
    pub report_schema: String,
    pub runner: String,
    pub runner_version: String,
    pub command: String,
    pub working_directory: String,
    pub report_sha256: String,
    pub configuration_sha256: String,
    pub inputs: Vec<InputFingerprint>,
    pub result_at_execution: VerificationStatus,
    pub passed: u64,
    pub failed: u64,
    pub skipped: u64,
    pub duration_seconds: f64,
    pub required_tests: Vec<String>,
    pub non_vacuity: NativeTestNonVacuity,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub diagnostics: Vec<String>,
    pub recorded_at_unix_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeTestAssessment {
    pub evidence: NativeTestEvidence,
    pub current_validity: EvidenceValidity,
}

pub fn discover(root: &Path) -> Result<Vec<NativeTestDefinition>, Error> {
    let directory = root.join("native-tests/checks");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&directory)
        .map_err(|source| Error::Io {
            path: directory.clone(),
            source,
        })?
        .map(|entry| entry.map(|item| item.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory.clone(),
            source,
        })?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let mut definition: NativeTestDefinition =
                serde_json::from_slice(&fs::read(&path).map_err(|source| Error::Io {
                    path: path.clone(),
                    source,
                })?)
                .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))?;
            definition.source = path;
            validate_definition(&definition)?;
            Ok(definition)
        })
        .collect()
}

fn validate_definition(definition: &NativeTestDefinition) -> Result<(), Error> {
    if definition.id.is_empty()
        || definition.version.is_empty()
        || definition.command.is_empty()
        || definition.working_directory.is_empty()
        || definition.minimum_passed == 0
    {
        return Err(Error::ProviderFailure(format!(
            "{}: native test id, version, command, working directory, and a positive minimum_passed are required",
            definition.source.display()
        )));
    }
    let unique = definition.required_tests.iter().collect::<BTreeSet<_>>();
    if unique.len() != definition.required_tests.len() {
        return Err(Error::ProviderFailure(format!(
            "{}: required_tests contains duplicates",
            definition.source.display()
        )));
    }
    Ok(())
}

pub fn import(
    roots: &VerificationRoots,
    definition: &NativeTestDefinition,
    report_path: &Path,
) -> Result<NativeTestEvidence, Error> {
    let report_bytes = fs::read(report_path).map_err(|source| Error::Io {
        path: report_path.to_path_buf(),
        source,
    })?;
    let report: NativeTestReport = serde_json::from_slice(&report_bytes)
        .map_err(|error| Error::ProviderFailure(format!("{}: {error}", report_path.display())))?;
    let inputs = current_inputs(roots, definition)?;
    let configuration_sha256 = configuration_hash(definition);
    let mut diagnostics = report.diagnostics.clone();
    let observed_pass = report
        .tests
        .iter()
        .filter(|test| test.status == NativeTestCaseStatus::Pass)
        .map(|test| test.name.as_str())
        .collect::<BTreeSet<_>>();
    let observed_required = definition
        .required_tests
        .iter()
        .filter(|name| observed_pass.contains(name.as_str()))
        .count() as u64;
    let non_vacuity = NativeTestNonVacuity {
        executed_tests: report.passed + report.failed,
        required_tests: definition.required_tests.len() as u64,
        observed_required_tests: observed_required,
        non_empty_execution: report.passed + report.failed > 0,
        all_required_observed_pass: observed_required == definition.required_tests.len() as u64,
    };
    if report.schema_version != REPORT_SCHEMA {
        diagnostics.push(format!(
            "expected report schema {REPORT_SCHEMA}, observed {}",
            report.schema_version
        ));
    }
    if report.command != definition.command {
        diagnostics.push(format!(
            "expected command `{}`, observed `{}`",
            definition.command, report.command
        ));
    }
    if report.working_directory != definition.working_directory {
        diagnostics.push(format!(
            "expected working directory `{}`, observed `{}`",
            definition.working_directory, report.working_directory
        ));
    }
    if report.passed < definition.minimum_passed {
        diagnostics.push(format!(
            "expected at least {} passed tests, observed {}",
            definition.minimum_passed, report.passed
        ));
    }
    if report.skipped > definition.maximum_skipped {
        diagnostics.push(format!(
            "expected at most {} skipped tests, observed {}",
            definition.maximum_skipped, report.skipped
        ));
    }
    for required in definition
        .required_tests
        .iter()
        .filter(|name| !observed_pass.contains(name.as_str()))
    {
        diagnostics.push(format!("required test `{required}` did not report PASS"));
    }
    let valid = report.schema_version == REPORT_SCHEMA
        && report.command == definition.command
        && report.working_directory == definition.working_directory
        && report.result == VerificationStatus::Pass
        && report.failed == 0
        && report.passed >= definition.minimum_passed
        && report.skipped <= definition.maximum_skipped
        && non_vacuity.non_empty_execution
        && non_vacuity.all_required_observed_pass;
    Ok(NativeTestEvidence {
        id: EvidenceId("pending".into()),
        obligation: ProofObligationId(format!("NATIVE-TEST:{}", definition.id)),
        definition_id: definition.id.clone(),
        definition_version: definition.version.clone(),
        provider: PROVIDER_NAME.into(),
        provider_version: PROVIDER_VERSION.into(),
        report_schema: report.schema_version,
        runner: report.runner,
        runner_version: report.runner_version,
        command: report.command,
        working_directory: report.working_directory,
        report_sha256: hash(&report_bytes),
        configuration_sha256,
        inputs,
        result_at_execution: if valid {
            VerificationStatus::Pass
        } else {
            VerificationStatus::Fail
        },
        passed: report.passed,
        failed: report.failed,
        skipped: report.skipped,
        duration_seconds: report.duration_seconds,
        required_tests: definition.required_tests.clone(),
        non_vacuity,
        authority: definition.authority.clone(),
        does_not_prove: definition.does_not_prove.clone(),
        diagnostics,
        recorded_at_unix_nanos: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
    })
}

pub fn store(
    directory: &Path,
    mut evidence: NativeTestEvidence,
) -> Result<NativeTestEvidence, Error> {
    fs::create_dir_all(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let seed = serde_json::to_vec(&evidence).expect("native test evidence serialization");
    evidence.id = EvidenceId(format!("NATIVE-TEST-EVIDENCE:{}", &hash(&seed)[..24]));
    let target = directory.join(format!("{}.json", evidence.id.0));
    if !target.exists() {
        let temporary = directory.join(format!(".{}.tmp", evidence.id.0));
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&evidence).expect("native test evidence serialization"),
        )
        .map_err(|source| Error::Io {
            path: temporary.clone(),
            source,
        })?;
        fs::rename(&temporary, &target).map_err(|source| Error::Io {
            path: target,
            source,
        })?;
    }
    Ok(evidence)
}

pub fn load_all(directory: &Path) -> Result<Vec<NativeTestEvidence>, Error> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(directory)
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?
        .map(|entry| entry.map(|item| item.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    paths.sort();
    let mut evidence = paths
        .into_iter()
        .map(|path| {
            serde_json::from_slice::<NativeTestEvidence>(&fs::read(&path).map_err(|source| {
                Error::Io {
                    path: path.clone(),
                    source,
                }
            })?)
            .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    evidence
        .sort_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)));
    Ok(evidence)
}

pub fn assess(
    roots: &VerificationRoots,
    definition: &NativeTestDefinition,
    evidence: &NativeTestEvidence,
) -> Result<EvidenceValidity, Error> {
    Ok(
        if evidence.inputs == current_inputs(roots, definition)?
            && evidence.provider_version == PROVIDER_VERSION
            && evidence.definition_version == definition.version
            && evidence.configuration_sha256 == configuration_hash(definition)
        {
            EvidenceValidity::Current
        } else {
            EvidenceValidity::Stale
        },
    )
}

pub fn latest_assessment(
    roots: &VerificationRoots,
    definition: &NativeTestDefinition,
) -> Result<Option<NativeTestAssessment>, Error> {
    let Some(evidence) = load_all(&roots.state_root.join("native-test-evidence"))?
        .into_iter()
        .filter(|evidence| evidence.definition_id == definition.id)
        .max_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)))
    else {
        return Ok(None);
    };
    Ok(Some(NativeTestAssessment {
        current_validity: assess(roots, definition, &evidence)?,
        evidence,
    }))
}

pub fn graph_edges(
    roots: &VerificationRoots,
    definitions: &[NativeTestDefinition],
) -> Result<Vec<GraphEdge>, Error> {
    let mut edges = Vec::new();
    for definition in definitions {
        let obligation =
            GraphNode::ProofObligation(ProofObligationId(format!("NATIVE-TEST:{}", definition.id)));
        edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(roots.spec_identity(&definition.source))),
            kind: LinkKind::Defines,
            to: obligation.clone(),
        });
        for input in current_inputs(roots, definition)? {
            edges.push(GraphEdge {
                from: GraphNode::Artifact(ArtifactId(input.source)),
                kind: LinkKind::RelevantTo,
                to: obligation.clone(),
            });
        }
        for evidence in load_all(&roots.state_root.join("native-test-evidence"))?
            .into_iter()
            .filter(|evidence| evidence.definition_id == definition.id)
        {
            edges.push(GraphEdge {
                from: obligation.clone(),
                kind: LinkKind::EvidenceFor,
                to: GraphNode::Evidence(evidence.id),
            });
        }
    }
    edges.sort_by_key(|edge| serde_json::to_string(edge).expect("native test graph serialization"));
    edges.dedup();
    Ok(edges)
}

fn current_inputs(
    roots: &VerificationRoots,
    definition: &NativeTestDefinition,
) -> Result<Vec<InputFingerprint>, Error> {
    let mut semantic = vec![SemanticInput {
        identity: roots.spec_identity(&definition.source),
        path: definition.source.clone(),
    }];
    for input in &definition.inputs {
        collect_input(roots, input, &mut semantic)?;
    }
    let excluded = definition
        .excluded_inputs
        .iter()
        .map(|input| match input.root {
            InputRoot::Project => roots.project_identity(&roots.project_root.join(&input.path)),
            InputRoot::Specification => {
                roots.spec_identity(&roots.specification_root.join(&input.path))
            }
        })
        .collect::<Vec<_>>();
    semantic.retain(|input| {
        !excluded.iter().any(|prefix| {
            input.identity == *prefix
                || input
                    .identity
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
    });
    let mut values =
        crate::evidence::fingerprint_semantic_files(&semantic).map_err(|source| Error::Io {
            path: roots.project_root.clone(),
            source,
        })?;
    values.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(values)
}

fn collect_input(
    roots: &VerificationRoots,
    input: &ScenarioInput,
    output: &mut Vec<SemanticInput>,
) -> Result<(), Error> {
    let (base, identity) = match input.root {
        InputRoot::Project => (
            &roots.project_root,
            roots.project_identity(&roots.project_root.join(&input.path)),
        ),
        InputRoot::Specification => (
            &roots.specification_root,
            roots.spec_identity(&roots.specification_root.join(&input.path)),
        ),
    };
    let path = base.join(&input.path);
    if path.is_dir() {
        let mut entries = fs::read_dir(&path)
            .map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?
            .map(|entry| entry.map(|item| item.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
        entries.sort();
        for child in entries {
            let relative = child.strip_prefix(base).expect("input remains below root");
            collect_input(
                roots,
                &ScenarioInput {
                    root: input.root.clone(),
                    path: relative.to_path_buf(),
                },
                output,
            )?;
        }
    } else {
        output.push(SemanticInput { identity, path });
    }
    Ok(())
}

fn configuration_hash(definition: &NativeTestDefinition) -> String {
    hash(
        &serde_json::to_vec(&(
            PROVIDER_VERSION,
            &definition.version,
            &definition.command,
            &definition.working_directory,
            definition.minimum_passed,
            definition.maximum_skipped,
            &definition.required_tests,
            &definition.excluded_inputs,
            &definition.authority,
            &definition.does_not_prove,
        ))
        .expect("native test configuration serialization"),
    )
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
