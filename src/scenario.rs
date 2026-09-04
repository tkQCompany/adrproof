use crate::Error;
use crate::evidence::{EvidenceValidity, InputFingerprint, VerificationStatus};
use crate::project::{ArtifactId, EvidenceId, GraphEdge, GraphNode, LinkKind, ProofObligationId};
use crate::roots::{SemanticInput, VerificationRoots};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PROVIDER_NAME: &str = "deterministic_failure_scenario";
pub const PROVIDER_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FaultPoint {
    BeforeMeiliRequest,
    MeiliRequestTimeout,
    AfterMeiliAcceptedBeforeResponse,
    AfterTaskUidBeforeLedgerTransaction,
    AfterLedgerInsertBeforeProcessedAt,
    BeforeLedgerTransactionCommit,
    AfterLedgerTransactionCommit,
    BeforeTaskStatusUpdate,
    BeforeWatermarkAdvance,
    ConcurrentFetchBarrier,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioCoverage {
    pub fault_class: String,
    pub fault_point: FaultPoint,
    pub state_space_scope: String,
    pub concurrency_scope: String,
    pub covered: Vec<String>,
    pub not_covered: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputRoot {
    Project,
    Specification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioInput {
    pub root: InputRoot,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioCommand {
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    pub runner_version: String,
}

fn default_timeout() -> u64 {
    120_000
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioDefinition {
    pub id: String,
    pub version: String,
    pub description: String,
    pub claim: String,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub coverage: ScenarioCoverage,
    pub runner: ScenarioCommand,
    pub expected_postconditions: BTreeMap<String, Value>,
    pub inputs: Vec<ScenarioInput>,
    #[serde(skip)]
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunnerOutput {
    #[serde(default)]
    pub postconditions: BTreeMap<String, Value>,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    #[serde(default)]
    pub trace: Vec<String>,
    #[serde(default)]
    pub infrastructure_error: Option<String>,
    #[serde(default)]
    pub tool_versions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostconditionAssessment {
    pub id: String,
    pub expected: Value,
    pub observed: Option<Value>,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioEvidence {
    pub id: EvidenceId,
    pub obligation: ProofObligationId,
    pub scenario_id: String,
    pub scenario_version: String,
    pub provider: String,
    pub provider_version: String,
    pub runner_version: String,
    pub fault_point: FaultPoint,
    pub coverage: ScenarioCoverage,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub implementation_fingerprint: String,
    pub fixture_fingerprint: String,
    pub configuration_sha256: String,
    pub inputs: Vec<InputFingerprint>,
    pub result_at_execution: VerificationStatus,
    pub postconditions: Vec<PostconditionAssessment>,
    pub diagnostics: Vec<String>,
    pub trace: Vec<String>,
    pub tool_versions: BTreeMap<String, String>,
    pub recorded_at_unix_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioAssessment {
    pub evidence: ScenarioEvidence,
    pub current_validity: EvidenceValidity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredChild {
    pub obligation_id: String,
    pub evidence_kind: ChildEvidenceKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChildEvidenceKind {
    Relational,
    Scenario,
    NativeTest,
    Model,
    ModelValidation,
    Correspondence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentObligation {
    pub id: String,
    pub claim: String,
    pub authority: String,
    pub required_children: Vec<RequiredChild>,
    #[serde(skip)]
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildStatus {
    pub obligation_id: String,
    pub status: VerificationStatus,
    pub validity: Option<EvidenceValidity>,
    pub evidence_id: Option<EvidenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentAssessment {
    pub parent_id: String,
    pub claim: String,
    pub authority: String,
    pub status: VerificationStatus,
    pub children: Vec<ChildStatus>,
}

pub fn discover(root: &Path) -> Result<Vec<ScenarioDefinition>, Error> {
    let directory = root.join("scenarios");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = json_files(&directory)?;
    paths.retain(|path| !path.file_name().is_some_and(|name| name == "parents.json"));
    paths
        .into_iter()
        .map(|path| {
            let mut definition: ScenarioDefinition =
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

pub fn discover_parents(root: &Path) -> Result<Vec<ParentObligation>, Error> {
    let path = root.join("scenarios/parents.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let mut parents: Vec<ParentObligation> =
        serde_json::from_slice(&fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?)
        .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))?;
    for parent in &mut parents {
        parent.source = path.clone();
        parent.required_children.sort_by(|a, b| {
            (&a.obligation_id, &a.evidence_kind).cmp(&(&b.obligation_id, &b.evidence_kind))
        });
        parent.required_children.dedup();
    }
    parents.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(parents)
}

fn validate_definition(definition: &ScenarioDefinition) -> Result<(), Error> {
    if definition.id.is_empty()
        || definition.version.is_empty()
        || definition.runner.runner_version.is_empty()
        || definition.expected_postconditions.is_empty()
    {
        return Err(Error::ProviderFailure(format!(
            "{}: scenario id, version, runner version, and postconditions are required",
            definition.source.display()
        )));
    }
    Ok(())
}

fn json_files(directory: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut paths = fs::read_dir(directory)
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?
        .map(|entry| entry.map(|value| value.path()))
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
    Ok(paths)
}

pub fn run(
    roots: &VerificationRoots,
    definition: &ScenarioDefinition,
) -> Result<ScenarioEvidence, Error> {
    let inputs = current_inputs(roots, definition)?;
    let implementation_fingerprint = fingerprint_group(
        &inputs
            .iter()
            .filter(|input| input.source.starts_with("project:"))
            .cloned()
            .collect::<Vec<_>>(),
    );
    let fixture_fingerprint = fingerprint_group(
        &inputs
            .iter()
            .filter(|input| !input.source.starts_with("project:"))
            .cloned()
            .collect::<Vec<_>>(),
    );
    let configuration_sha256 = hash(
        serde_json::to_vec(&(
            PROVIDER_VERSION,
            &definition.runner,
            &definition.coverage,
            &definition.expected_postconditions,
        ))
        .expect("scenario configuration serialization")
        .as_slice(),
    );
    let started = Instant::now();
    let runner_program = if definition.runner.program.is_absolute() {
        definition.runner.program.clone()
    } else {
        roots.specification_root.join(&definition.runner.program)
    };
    let mut command = Command::new(runner_program);
    command
        .args(&definition.runner.args)
        .env("ADRPROOF_PROJECT_ROOT", &roots.project_root)
        .env("ADRPROOF_SPEC_ROOT", &roots.specification_root)
        .env("ADRPROOF_STATE_ROOT", &roots.state_root)
        .env("ADRPROOF_SCENARIO_ID", &definition.id)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        Error::ProviderFailure(format!("scenario runner could not start: {error}"))
    })?;
    let timeout = Duration::from_millis(definition.runner.timeout_ms);
    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            Error::ProviderFailure(format!("scenario runner wait failed: {error}"))
        })? {
            let output = child.wait_with_output().map_err(|error| {
                Error::ProviderFailure(format!("scenario runner output failed: {error}"))
            })?;
            let parsed = serde_json::from_slice::<RunnerOutput>(&output.stdout);
            let runner = match parsed {
                Ok(value) => value,
                Err(error) => RunnerOutput {
                    infrastructure_error: Some(format!(
                        "runner exited {status}; invalid JSON output: {error}; stderr: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )),
                    ..empty_runner_output()
                },
            };
            return Ok(build_evidence(
                definition,
                inputs,
                implementation_fingerprint,
                fixture_fingerprint,
                configuration_sha256,
                runner,
            ));
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(build_evidence(
                definition,
                inputs,
                implementation_fingerprint,
                fixture_fingerprint,
                configuration_sha256,
                RunnerOutput {
                    infrastructure_error: Some(format!(
                        "scenario runner timed out after {} ms",
                        definition.runner.timeout_ms
                    )),
                    ..empty_runner_output()
                },
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn empty_runner_output() -> RunnerOutput {
    RunnerOutput {
        postconditions: BTreeMap::new(),
        diagnostics: Vec::new(),
        trace: Vec::new(),
        infrastructure_error: None,
        tool_versions: BTreeMap::new(),
    }
}

fn build_evidence(
    definition: &ScenarioDefinition,
    inputs: Vec<InputFingerprint>,
    implementation_fingerprint: String,
    fixture_fingerprint: String,
    configuration_sha256: String,
    runner: RunnerOutput,
) -> ScenarioEvidence {
    let postconditions = definition
        .expected_postconditions
        .iter()
        .map(|(id, expected)| {
            let observed = runner.postconditions.get(id).cloned();
            PostconditionAssessment {
                id: id.clone(),
                expected: expected.clone(),
                passed: observed.as_ref() == Some(expected),
                observed,
            }
        })
        .collect::<Vec<_>>();
    let result = if runner.infrastructure_error.is_some() {
        VerificationStatus::Error
    } else if postconditions.iter().all(|condition| condition.passed) {
        VerificationStatus::Pass
    } else {
        VerificationStatus::Fail
    };
    let mut diagnostics = runner.diagnostics;
    if let Some(error) = runner.infrastructure_error {
        diagnostics.push(error);
    }
    for condition in postconditions.iter().filter(|condition| !condition.passed) {
        diagnostics.push(format!(
            "postcondition `{}` expected {} but observed {}",
            condition.id,
            condition.expected,
            condition
                .observed
                .as_ref()
                .map_or_else(|| "<missing>".into(), Value::to_string)
        ));
    }
    ScenarioEvidence {
        id: EvidenceId("pending".into()),
        obligation: ProofObligationId(format!("SCENARIO:{}", definition.id)),
        scenario_id: definition.id.clone(),
        scenario_version: definition.version.clone(),
        provider: PROVIDER_NAME.into(),
        provider_version: PROVIDER_VERSION.into(),
        runner_version: definition.runner.runner_version.clone(),
        fault_point: definition.coverage.fault_point.clone(),
        coverage: definition.coverage.clone(),
        authority: definition.authority.clone(),
        does_not_prove: definition.does_not_prove.clone(),
        implementation_fingerprint,
        fixture_fingerprint,
        configuration_sha256,
        inputs,
        result_at_execution: result,
        postconditions,
        diagnostics,
        trace: runner.trace,
        tool_versions: runner.tool_versions,
        recorded_at_unix_nanos: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
    }
}

pub fn store(directory: &Path, mut evidence: ScenarioEvidence) -> Result<ScenarioEvidence, Error> {
    fs::create_dir_all(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let seed = serde_json::to_vec(&evidence).expect("scenario evidence serialization");
    evidence.id = EvidenceId(format!("SCENARIO-EVIDENCE:{}", &hash(&seed)[..24]));
    let stem = crate::evidence::storage_stem(&evidence.id);
    let target = directory.join(format!("{stem}.json"));
    if !target.exists() {
        let temporary = directory.join(format!(".{stem}.tmp"));
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&evidence).expect("scenario evidence serialization"),
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

pub fn load_all(directory: &Path) -> Result<Vec<ScenarioEvidence>, Error> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut evidence = json_files(directory)?
        .into_iter()
        .map(|path| {
            serde_json::from_slice::<ScenarioEvidence>(&fs::read(&path).map_err(|source| {
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
    definition: &ScenarioDefinition,
    evidence: &ScenarioEvidence,
) -> Result<EvidenceValidity, Error> {
    let inputs = current_inputs(roots, definition)?;
    let configuration_sha256 = hash(
        serde_json::to_vec(&(
            PROVIDER_VERSION,
            &definition.runner,
            &definition.coverage,
            &definition.expected_postconditions,
        ))
        .expect("scenario configuration serialization")
        .as_slice(),
    );
    Ok(
        if evidence.inputs == inputs
            && evidence.provider_version == PROVIDER_VERSION
            && evidence.runner_version == definition.runner.runner_version
            && evidence.configuration_sha256 == configuration_sha256
        {
            EvidenceValidity::Current
        } else {
            EvidenceValidity::Stale
        },
    )
}

pub fn latest_assessment(
    roots: &VerificationRoots,
    definition: &ScenarioDefinition,
) -> Result<Option<ScenarioAssessment>, Error> {
    let all = load_all(&roots.state_root.join("scenario-evidence"))?;
    let Some(evidence) = all
        .into_iter()
        .filter(|evidence| evidence.scenario_id == definition.id)
        .max_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)))
    else {
        return Ok(None);
    };
    Ok(Some(ScenarioAssessment {
        current_validity: assess(roots, definition, &evidence)?,
        evidence,
    }))
}

pub fn graph_edges(
    roots: &VerificationRoots,
    definitions: &[ScenarioDefinition],
    parents: &[ParentObligation],
) -> Result<Vec<GraphEdge>, Error> {
    let mut edges = Vec::new();
    for definition in definitions {
        let obligation =
            GraphNode::ProofObligation(ProofObligationId(format!("SCENARIO:{}", definition.id)));
        edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(roots.spec_identity(&definition.source))),
            kind: LinkKind::Defines,
            to: obligation.clone(),
        });
        for source in graph_input_identities(roots, definition)? {
            edges.push(GraphEdge {
                from: GraphNode::Artifact(ArtifactId(source)),
                kind: LinkKind::RelevantTo,
                to: obligation.clone(),
            });
        }
        for evidence in load_all(&roots.state_root.join("scenario-evidence"))?
            .into_iter()
            .filter(|evidence| evidence.scenario_id == definition.id)
        {
            edges.push(GraphEdge {
                from: obligation.clone(),
                kind: LinkKind::EvidenceFor,
                to: GraphNode::Evidence(evidence.id),
            });
        }
    }
    for parent in parents {
        for child in &parent.required_children {
            edges.push(GraphEdge {
                from: GraphNode::ProofObligation(ProofObligationId(parent.id.clone())),
                kind: LinkKind::Requires,
                to: GraphNode::ProofObligation(ProofObligationId(child.obligation_id.clone())),
            });
        }
    }
    edges.sort_by_key(|edge| serde_json::to_string(edge).expect("scenario graph serialization"));
    edges.dedup();
    Ok(edges)
}

fn graph_input_identities(
    roots: &VerificationRoots,
    definition: &ScenarioDefinition,
) -> Result<Vec<String>, Error> {
    fn collect(
        directory: &Path,
        roots: &VerificationRoots,
        root: &InputRoot,
        output: &mut Vec<String>,
    ) -> Result<(), Error> {
        let mut entries = fs::read_dir(directory)
            .map_err(|source| Error::Io {
                path: directory.to_path_buf(),
                source,
            })?
            .map(|entry| entry.map(|value| value.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|source| Error::Io {
                path: directory.to_path_buf(),
                source,
            })?;
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect(&path, roots, root, output)?;
            } else {
                output.push(match root {
                    InputRoot::Project => roots.project_identity(&path),
                    InputRoot::Specification => roots.spec_identity(&path),
                });
            }
        }
        Ok(())
    }

    let mut identities = vec![roots.spec_identity(&definition.source)];
    for input in &definition.inputs {
        let path = match input.root {
            InputRoot::Project => roots.project_root.join(&input.path),
            InputRoot::Specification => roots.specification_root.join(&input.path),
        };
        if path.is_dir() {
            collect(&path, roots, &input.root, &mut identities)?;
        } else {
            identities.push(match input.root {
                InputRoot::Project => roots.project_identity(&path),
                InputRoot::Specification => roots.spec_identity(&path),
            });
        }
    }
    identities.sort();
    identities.dedup();
    Ok(identities)
}

pub fn write_graph(
    roots: &VerificationRoots,
    definitions: &[ScenarioDefinition],
    parents: &[ParentObligation],
) -> Result<PathBuf, Error> {
    fs::create_dir_all(&roots.state_root).map_err(|source| Error::Io {
        path: roots.state_root.clone(),
        source,
    })?;
    let target = roots.state_root.join("scenario-graph.json");
    let temporary = roots.state_root.join(".scenario-graph.json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&graph_edges(roots, definitions, parents)?)
            .expect("scenario graph serialization"),
    )
    .map_err(|source| Error::Io {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, &target).map_err(|source| Error::Io {
        path: target.clone(),
        source,
    })?;
    Ok(target)
}

fn current_inputs(
    roots: &VerificationRoots,
    definition: &ScenarioDefinition,
) -> Result<Vec<InputFingerprint>, Error> {
    let mut semantic = Vec::new();
    semantic.push(SemanticInput {
        identity: roots.spec_identity(&definition.source),
        path: definition.source.clone(),
    });
    for input in &definition.inputs {
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
            collect_files(&path, base, &mut semantic, roots, &input.root)?;
        } else {
            semantic.push(SemanticInput { identity, path });
        }
    }
    let mut fingerprints =
        crate::evidence::fingerprint_semantic_files(&semantic).map_err(|source| Error::Io {
            path: roots.project_root.clone(),
            source,
        })?;
    fingerprints.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(fingerprints)
}

fn collect_files(
    directory: &Path,
    base: &Path,
    output: &mut Vec<SemanticInput>,
    roots: &VerificationRoots,
    root: &InputRoot,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(directory)
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_files(&path, base, output, roots, root)?;
        } else {
            let identity = match root {
                InputRoot::Project => roots.project_identity(&path),
                InputRoot::Specification => roots.spec_identity(&path),
            };
            let _ = base;
            output.push(SemanticInput { identity, path });
        }
    }
    Ok(())
}

fn fingerprint_group(inputs: &[InputFingerprint]) -> String {
    hash(&serde_json::to_vec(inputs).expect("fingerprint group serialization"))
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn aggregate(parent: &ParentObligation, children: Vec<ChildStatus>) -> ParentAssessment {
    let required = parent
        .required_children
        .iter()
        .map(|child| child.obligation_id.as_str())
        .collect::<BTreeSet<_>>();
    let supplied = children
        .iter()
        .map(|child| child.obligation_id.as_str())
        .collect::<BTreeSet<_>>();
    let missing = required.difference(&supplied).next().is_some();
    let status = if children
        .iter()
        .any(|child| child.status == VerificationStatus::Error)
    {
        VerificationStatus::Error
    } else if children.iter().any(|child| {
        child.status == VerificationStatus::Fail
            && child.validity == Some(EvidenceValidity::Current)
    }) {
        VerificationStatus::Fail
    } else if children
        .iter()
        .any(|child| child.validity == Some(EvidenceValidity::Stale))
    {
        VerificationStatus::Stale
    } else if missing
        || children.iter().any(|child| {
            matches!(
                child.status,
                VerificationStatus::Unverified
                    | VerificationStatus::Unknown
                    | VerificationStatus::NotApplicable
            )
        })
    {
        VerificationStatus::Unverified
    } else if !required.is_empty()
        && required == supplied
        && children.iter().all(|child| {
            child.status == VerificationStatus::Pass
                && child.validity == Some(EvidenceValidity::Current)
        })
    {
        VerificationStatus::Pass
    } else {
        VerificationStatus::Unverified
    };
    ParentAssessment {
        parent_id: parent.id.clone(),
        claim: parent.claim.clone(),
        authority: parent.authority.clone(),
        status,
        children,
    }
}
