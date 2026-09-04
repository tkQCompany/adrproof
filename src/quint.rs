use crate::Error;
use crate::evidence::{EvidenceValidity, InputFingerprint, VerificationStatus};
use crate::project::{ArtifactId, EvidenceId, GraphEdge, GraphNode, LinkKind, ProofObligationId};
use crate::roots::{SemanticInput, VerificationRoots};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const PROVIDER_NAME: &str = "quint_model_evidence";
pub const PROVIDER_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelCheckerBackend {
    Tlc,
    Apalache,
}

impl ModelCheckerBackend {
    fn cli_name(&self) -> &'static str {
        match self {
            Self::Tlc => "tlc",
            Self::Apalache => "apalache",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPropertyKind {
    Invariant,
    Temporal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedModelCheckerOutcome {
    NoCounterexample,
    CounterexampleRequired,
}

fn default_expected_outcome() -> ExpectedModelCheckerOutcome {
    ExpectedModelCheckerOutcome::NoCounterexample
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExplorationSemantics {
    ExhaustiveFinite,
    Bounded { max_steps: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionSemantics {
    Complete,
    IncompleteTimeout,
    Unsupported,
    InfrastructureError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalModelAuthority {
    pub claim: String,
    pub scope: String,
    pub does_not_prove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCheckDefinition {
    pub id: String,
    pub model_id: String,
    pub model: PathBuf,
    pub property_id: String,
    pub property_name: String,
    pub property_kind: ModelPropertyKind,
    #[serde(default = "default_expected_outcome")]
    pub expected_outcome: ExpectedModelCheckerOutcome,
    pub backend: ModelCheckerBackend,
    pub quint_version: String,
    pub backend_version: String,
    #[serde(default)]
    pub main: Option<String>,
    #[serde(default = "default_init")]
    pub init: String,
    #[serde(default = "default_step")]
    pub step: String,
    #[serde(default)]
    pub constants: BTreeMap<String, Value>,
    #[serde(default)]
    pub bounds: BTreeMap<String, Value>,
    #[serde(default)]
    pub model_bindings: BTreeMap<String, String>,
    #[serde(default)]
    pub fairness: Vec<String>,
    #[serde(default)]
    pub max_steps: Option<u64>,
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub semantic_flags: Vec<String>,
    pub authority: FormalModelAuthority,
    #[serde(skip)]
    pub source: PathBuf,
}

fn default_init() -> String {
    "init".into()
}

fn default_step() -> String {
    "step".into()
}

fn default_timeout() -> u64 {
    120_000
}

impl ModelCheckDefinition {
    pub fn exploration(&self) -> ExplorationSemantics {
        match self.backend {
            ModelCheckerBackend::Tlc => ExplorationSemantics::ExhaustiveFinite,
            ModelCheckerBackend::Apalache => ExplorationSemantics::Bounded {
                max_steps: self.max_steps.unwrap_or(10),
            },
        }
    }

    fn configuration_sha256(&self) -> String {
        hash(
            &serde_json::to_vec(&(PROVIDER_VERSION, self))
                .expect("Quint configuration serialization"),
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExploredStateStats {
    pub generated_states: Option<u64>,
    pub distinct_states: Option<u64>,
    pub states_left: Option<u64>,
    pub depth: Option<u64>,
    pub duration_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuintModelEvidence {
    pub id: EvidenceId,
    pub obligation: ProofObligationId,
    pub model_check_id: String,
    pub model_id: String,
    pub property_id: String,
    pub property_name: String,
    pub property_kind: ModelPropertyKind,
    pub expected_outcome: ExpectedModelCheckerOutcome,
    pub provider: String,
    pub provider_version: String,
    pub backend: ModelCheckerBackend,
    pub backend_version: String,
    pub quint_version: String,
    pub constants: BTreeMap<String, Value>,
    pub bounds: BTreeMap<String, Value>,
    pub model_bindings: BTreeMap<String, String>,
    pub fairness: Vec<String>,
    pub exploration: ExplorationSemantics,
    pub completion: CompletionSemantics,
    pub authority: FormalModelAuthority,
    pub configuration_sha256: String,
    pub inputs: Vec<InputFingerprint>,
    pub model_fingerprint: String,
    pub property_fingerprint: String,
    pub result_at_execution: VerificationStatus,
    pub explored_state_stats: ExploredStateStats,
    pub counterexample: Vec<String>,
    pub diagnostics: Vec<String>,
    pub recorded_at_unix_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuintModelAssessment {
    pub evidence: QuintModelEvidence,
    pub current_validity: EvidenceValidity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioModelMapping {
    pub scenario_id: String,
    pub expected_scenario_result: VerificationStatus,
    pub model_check_id: String,
    pub trace_pattern: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelValidationDefinition {
    pub id: String,
    pub claim: String,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub mappings: Vec<ScenarioModelMapping>,
    #[serde(skip)]
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScenarioModelMappingAssessment {
    pub scenario_id: String,
    pub expected_scenario_result: VerificationStatus,
    pub scenario_evidence_id: Option<EvidenceId>,
    pub scenario_result: Option<VerificationStatus>,
    pub scenario_validity: Option<EvidenceValidity>,
    pub model_check_id: String,
    pub model_evidence_id: Option<EvidenceId>,
    pub model_result: Option<VerificationStatus>,
    pub model_validity: Option<EvidenceValidity>,
    pub trace_pattern: Vec<String>,
    pub scenario_trace_matches: bool,
    pub missing_trace_events: Vec<String>,
    pub status: VerificationStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelValidationEvidence {
    pub id: EvidenceId,
    pub obligation: ProofObligationId,
    pub validation_id: String,
    pub provider: String,
    pub provider_version: String,
    pub claim: String,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub definition_sha256: String,
    pub dependency_fingerprints: BTreeMap<String, String>,
    pub mappings: Vec<ScenarioModelMappingAssessment>,
    pub result_at_execution: VerificationStatus,
    pub diagnostics: Vec<String>,
    pub recorded_at_unix_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelValidationAssessment {
    pub evidence: ModelValidationEvidence,
    pub current_validity: EvidenceValidity,
}

pub fn discover(root: &Path) -> Result<Vec<ModelCheckDefinition>, Error> {
    let directory = root.join("models/checks");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&directory)
        .map_err(|source| Error::Io {
            path: directory.clone(),
            source,
        })?
        .map(|entry| entry.map(|value| value.path()))
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
            let mut definition: ModelCheckDefinition =
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

pub fn discover_validations(root: &Path) -> Result<Vec<ModelValidationDefinition>, Error> {
    let path = root.join("models/scenario-validation.json");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let mut definitions: Vec<ModelValidationDefinition> =
        serde_json::from_slice(&fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?)
        .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))?;
    for definition in &mut definitions {
        definition.source = path.clone();
        definition
            .mappings
            .sort_by(|a, b| a.scenario_id.cmp(&b.scenario_id));
        if definition.id.is_empty() || definition.mappings.is_empty() {
            return Err(Error::ProviderFailure(format!(
                "{}: validation id and mappings are required",
                path.display()
            )));
        }
    }
    definitions.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(definitions)
}

fn validate_definition(definition: &ModelCheckDefinition) -> Result<(), Error> {
    if definition.id.is_empty()
        || definition.model_id.is_empty()
        || definition.property_id.is_empty()
        || definition.property_name.is_empty()
        || definition.quint_version.is_empty()
        || definition.backend_version.is_empty()
    {
        return Err(Error::ProviderFailure(format!(
            "{}: model check IDs, property name, and pinned tool versions are required",
            definition.source.display()
        )));
    }
    if definition.property_kind == ModelPropertyKind::Temporal && definition.fairness.is_empty() {
        return Err(Error::ProviderFailure(format!(
            "{}: temporal checks must record explicit fairness assumptions",
            definition.source.display()
        )));
    }
    if definition.backend == ModelCheckerBackend::Tlc && definition.max_steps.is_some() {
        return Err(Error::ProviderFailure(format!(
            "{}: TLC exhaustive evidence must not declare max_steps",
            definition.source.display()
        )));
    }
    if definition.backend == ModelCheckerBackend::Apalache && definition.max_steps.is_none() {
        return Err(Error::ProviderFailure(format!(
            "{}: Apalache bounded evidence must explicitly declare max_steps",
            definition.source.display()
        )));
    }
    Ok(())
}

pub fn run(
    roots: &VerificationRoots,
    definition: &ModelCheckDefinition,
) -> Result<QuintModelEvidence, Error> {
    let executable = std::env::var_os("ADRPROOF_QUINT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("quint"));
    run_with_executable(roots, definition, &executable)
}

#[doc(hidden)]
pub fn run_with_executable(
    roots: &VerificationRoots,
    definition: &ModelCheckDefinition,
    executable: &Path,
) -> Result<QuintModelEvidence, Error> {
    let inputs = current_inputs(roots, definition)?;
    let model_identity = roots.spec_identity(&roots.specification_root.join(&definition.model));
    let model_fingerprint = inputs
        .iter()
        .find(|input| input.source == model_identity)
        .map(|input| input.sha256.clone())
        .unwrap_or_default();
    let property_fingerprint = hash(
        &serde_json::to_vec(&(
            &model_fingerprint,
            &definition.property_id,
            &definition.property_name,
            &definition.property_kind,
        ))
        .expect("property fingerprint serialization"),
    );
    let base = EvidenceBuilder {
        definition,
        inputs,
        model_fingerprint,
        property_fingerprint,
    };

    if let Err(diagnostic) = validate_model_bindings(roots, definition) {
        return Ok(base.finish(
            CompletionSemantics::InfrastructureError,
            VerificationStatus::Error,
            definition.backend_version.clone(),
            ExploredStateStats::default(),
            Vec::new(),
            vec![diagnostic],
        ));
    }

    if definition.backend == ModelCheckerBackend::Apalache
        && definition.property_kind == ModelPropertyKind::Temporal
    {
        return Ok(base.finish(
            CompletionSemantics::Unsupported,
            VerificationStatus::Unverified,
            definition.backend_version.clone(),
            ExploredStateStats::default(),
            Vec::new(),
            vec!["temporal checking is routed to TLC; Apalache temporal support is not used by ADRProof".into()],
        ));
    }

    let version = command_output(executable, &["--version"], definition.timeout_ms, roots)?;
    if version.timed_out {
        return Ok(base.finish(
            CompletionSemantics::IncompleteTimeout,
            VerificationStatus::Error,
            definition.backend_version.clone(),
            ExploredStateStats::default(),
            Vec::new(),
            vec![format!(
                "Quint version detection timed out after {} ms",
                definition.timeout_ms
            )],
        ));
    }
    let detected_quint = version.stdout.trim();
    if !version.success || detected_quint != definition.quint_version {
        return Ok(base.finish(
            CompletionSemantics::InfrastructureError,
            VerificationStatus::Error,
            definition.backend_version.clone(),
            ExploredStateStats::default(),
            Vec::new(),
            vec![format!(
                "Quint version mismatch or detection failure: expected {}, observed {:?}; {}",
                definition.quint_version, detected_quint, version.stderr
            )],
        ));
    }

    let model = roots.specification_root.join(&definition.model);
    let mut args = vec![
        "verify".into(),
        model.display().to_string(),
        "--backend".into(),
        definition.backend.cli_name().into(),
        "--init".into(),
        definition.init.clone(),
        "--step".into(),
        definition.step.clone(),
        "--verbosity".into(),
        "3".into(),
    ];
    if let Some(main) = &definition.main {
        args.extend(["--main".into(), main.clone()]);
    }
    match definition.property_kind {
        ModelPropertyKind::Invariant => {
            args.extend(["--invariant".into(), definition.property_name.clone()]);
        }
        ModelPropertyKind::Temporal => {
            args.extend(["--temporal".into(), definition.property_name.clone()]);
        }
    }
    if let Some(max_steps) = definition.max_steps {
        args.extend(["--max-steps".into(), max_steps.to_string()]);
    }
    if definition.backend == ModelCheckerBackend::Apalache {
        args.extend([
            "--apalache-version".into(),
            definition.backend_version.clone(),
        ]);
    }
    args.extend(definition.semantic_flags.iter().cloned());
    let borrowed = args.iter().map(String::as_str).collect::<Vec<_>>();
    let started = Instant::now();
    let output = command_output(executable, &borrowed, definition.timeout_ms, roots)?;
    let combined = format!("{}\n{}", output.stdout, output.stderr);
    let mut stats = parse_stats(&combined);
    stats.duration_ms = started.elapsed().as_millis();
    if output.timed_out {
        return Ok(base.finish(
            CompletionSemantics::IncompleteTimeout,
            VerificationStatus::Error,
            definition.backend_version.clone(),
            stats,
            Vec::new(),
            vec![format!(
                "model checker timed out after {} ms",
                definition.timeout_ms
            )],
        ));
    }

    let no_violation = combined.contains("[ok] No violation found");
    let violation = combined.contains("[violation]")
        || combined.contains("Invariant q_inv is violated")
        || combined.contains("Temporal properties were violated")
        || combined.contains("error: found a counterexample");
    let Some(observed_backend) = detect_backend_version(&definition.backend, &combined) else {
        return Ok(base.finish(
            CompletionSemantics::InfrastructureError,
            VerificationStatus::Error,
            "undetected".into(),
            stats,
            Vec::new(),
            vec![format!(
                "model checker did not identify the configured {} backend in its output",
                definition.backend.cli_name()
            )],
        ));
    };
    if observed_backend != definition.backend_version {
        return Ok(base.finish(
            CompletionSemantics::InfrastructureError,
            VerificationStatus::Error,
            observed_backend,
            stats,
            Vec::new(),
            vec![format!(
                "backend version mismatch: expected {}, observed output from another version",
                definition.backend_version
            )],
        ));
    }
    if !no_violation && !violation {
        return Ok(base.finish(
            CompletionSemantics::InfrastructureError,
            VerificationStatus::Error,
            observed_backend,
            stats,
            Vec::new(),
            vec![format!(
                "model checker did not emit an authoritative completion marker (exit success: {}): {}",
                output.success,
                compact(&combined)
            )],
        ));
    }
    let result = match (&definition.expected_outcome, no_violation, violation) {
        (ExpectedModelCheckerOutcome::NoCounterexample, true, false)
        | (ExpectedModelCheckerOutcome::CounterexampleRequired, false, true) => {
            VerificationStatus::Pass
        }
        (ExpectedModelCheckerOutcome::NoCounterexample, false, true)
        | (ExpectedModelCheckerOutcome::CounterexampleRequired, true, false) => {
            VerificationStatus::Fail
        }
        _ => VerificationStatus::Error,
    };
    let counterexample = if violation {
        combined.lines().map(str::to_owned).collect()
    } else {
        Vec::new()
    };
    let diagnostic = match (&definition.expected_outcome, &result) {
        (ExpectedModelCheckerOutcome::CounterexampleRequired, VerificationStatus::Pass) => {
            "PASS means the configured model admits the requested behavior witness".into()
        }
        (ExpectedModelCheckerOutcome::CounterexampleRequired, VerificationStatus::Fail) => {
            "FAIL means the configured model did not produce the required behavior witness".into()
        }
        (ExpectedModelCheckerOutcome::NoCounterexample, VerificationStatus::Fail) => {
            "FAIL means a counterexample falsified the checked formal-model property".into()
        }
        (_, VerificationStatus::Pass) => match definition.exploration() {
            ExplorationSemantics::ExhaustiveFinite => {
                "PASS applies to exhaustive exploration of this configured finite formal model"
                    .into()
            }
            ExplorationSemantics::Bounded { max_steps } => {
                format!("PASS means no counterexample was found within {max_steps} steps")
            }
        },
        _ => "model checking did not establish the configured claim".into(),
    };
    Ok(base.finish(
        CompletionSemantics::Complete,
        result,
        observed_backend,
        stats,
        counterexample,
        vec![diagnostic],
    ))
}

struct EvidenceBuilder<'a> {
    definition: &'a ModelCheckDefinition,
    inputs: Vec<InputFingerprint>,
    model_fingerprint: String,
    property_fingerprint: String,
}

impl EvidenceBuilder<'_> {
    fn finish(
        self,
        completion: CompletionSemantics,
        result: VerificationStatus,
        backend_version: String,
        stats: ExploredStateStats,
        counterexample: Vec<String>,
        diagnostics: Vec<String>,
    ) -> QuintModelEvidence {
        QuintModelEvidence {
            id: EvidenceId("pending".into()),
            obligation: ProofObligationId(format!("MODEL:{}", self.definition.id)),
            model_check_id: self.definition.id.clone(),
            model_id: self.definition.model_id.clone(),
            property_id: self.definition.property_id.clone(),
            property_name: self.definition.property_name.clone(),
            property_kind: self.definition.property_kind.clone(),
            expected_outcome: self.definition.expected_outcome.clone(),
            provider: PROVIDER_NAME.into(),
            provider_version: PROVIDER_VERSION.into(),
            backend: self.definition.backend.clone(),
            backend_version,
            quint_version: self.definition.quint_version.clone(),
            constants: self.definition.constants.clone(),
            bounds: self.definition.bounds.clone(),
            model_bindings: self.definition.model_bindings.clone(),
            fairness: self.definition.fairness.clone(),
            exploration: self.definition.exploration(),
            completion,
            authority: self.definition.authority.clone(),
            configuration_sha256: self.definition.configuration_sha256(),
            inputs: self.inputs,
            model_fingerprint: self.model_fingerprint,
            property_fingerprint: self.property_fingerprint,
            result_at_execution: result,
            explored_state_stats: stats,
            counterexample,
            diagnostics,
            recorded_at_unix_nanos: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos()),
        }
    }
}

struct ProcessOutput {
    success: bool,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

fn command_output(
    executable: &Path,
    args: &[&str],
    timeout_ms: u64,
    roots: &VerificationRoots,
) -> Result<ProcessOutput, Error> {
    fs::create_dir_all(&roots.state_root).map_err(|source| Error::Io {
        path: roots.state_root.clone(),
        source,
    })?;
    let mut command = Command::new(executable);
    command
        .args(args)
        .current_dir(&roots.state_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    if let Some(home) = std::env::var_os("ADRPROOF_QUINT_HOME") {
        command.env("QUINT_HOME", home);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Ok(ProcessOutput {
                success: false,
                timed_out: false,
                stdout: String::new(),
                stderr: format!("could not start {}: {error}", executable.display()),
            });
        }
    };
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            Error::ProviderFailure(format!("Quint process wait failed: {error}"))
        })? {
            let output = child.wait_with_output().map_err(|error| {
                Error::ProviderFailure(format!("Quint process output failed: {error}"))
            })?;
            return Ok(ProcessOutput {
                success: status.success(),
                timed_out: false,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        if started.elapsed() >= Duration::from_millis(timeout_ms) {
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-KILL", &format!("-{}", child.id())])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            #[cfg(not(unix))]
            let _ = child.kill();
            let output = child.wait_with_output().map_err(|error| {
                Error::ProviderFailure(format!("Quint timeout cleanup failed: {error}"))
            })?;
            return Ok(ProcessOutput {
                success: false,
                timed_out: true,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn store(
    directory: &Path,
    mut evidence: QuintModelEvidence,
) -> Result<QuintModelEvidence, Error> {
    fs::create_dir_all(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let seed = serde_json::to_vec(&evidence).expect("Quint evidence serialization");
    evidence.id = EvidenceId(format!("MODEL-EVIDENCE:{}", &hash(&seed)[..24]));
    let stem = crate::evidence::storage_stem(&evidence.id);
    let target = directory.join(format!("{stem}.json"));
    if !target.exists() {
        let temporary = directory.join(format!(".{stem}.tmp"));
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&evidence).expect("Quint evidence serialization"),
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

pub fn load_all(directory: &Path) -> Result<Vec<QuintModelEvidence>, Error> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
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
    let mut evidence = paths
        .into_iter()
        .map(|path| {
            serde_json::from_slice::<QuintModelEvidence>(&fs::read(&path).map_err(|source| {
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
    definition: &ModelCheckDefinition,
    evidence: &QuintModelEvidence,
) -> Result<EvidenceValidity, Error> {
    let inputs = current_inputs(roots, definition)?;
    Ok(
        if evidence.inputs == inputs
            && evidence.provider_version == PROVIDER_VERSION
            && evidence.configuration_sha256 == definition.configuration_sha256()
            && evidence.quint_version == definition.quint_version
            && evidence.backend_version == definition.backend_version
        {
            EvidenceValidity::Current
        } else {
            EvidenceValidity::Stale
        },
    )
}

pub fn latest_assessment(
    roots: &VerificationRoots,
    definition: &ModelCheckDefinition,
) -> Result<Option<QuintModelAssessment>, Error> {
    let Some(evidence) = load_all(&roots.state_root.join("model-evidence"))?
        .into_iter()
        .filter(|evidence| evidence.model_check_id == definition.id)
        .max_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)))
    else {
        return Ok(None);
    };
    Ok(Some(QuintModelAssessment {
        current_validity: assess(roots, definition, &evidence)?,
        evidence,
    }))
}

pub fn run_validation(
    roots: &VerificationRoots,
    definition: &ModelValidationDefinition,
) -> Result<ModelValidationEvidence, Error> {
    let evaluation = evaluate_validation(roots, definition)?;
    let diagnostics = evaluation
        .mappings
        .iter()
        .filter(|mapping| mapping.status != VerificationStatus::Pass)
        .map(|mapping| {
            format!(
                "{} -> {} is {:?} (scenario {:?}/{:?}, model {:?}/{:?}, trace_match={}, missing_trace_events={:?})",
                mapping.scenario_id,
                mapping.model_check_id,
                mapping.status,
                mapping.scenario_result,
                mapping.scenario_validity,
                mapping.model_result,
                mapping.model_validity,
                mapping.scenario_trace_matches,
                mapping.missing_trace_events,
            )
        })
        .collect();
    Ok(ModelValidationEvidence {
        id: EvidenceId("pending".into()),
        obligation: ProofObligationId(format!("MODEL-VALIDATION:{}", definition.id)),
        validation_id: definition.id.clone(),
        provider: "scenario_model_cross_validation".into(),
        provider_version: PROVIDER_VERSION.into(),
        claim: definition.claim.clone(),
        authority: definition.authority.clone(),
        does_not_prove: definition.does_not_prove.clone(),
        definition_sha256: definition_fingerprint(definition)?,
        dependency_fingerprints: evaluation.dependency_fingerprints,
        mappings: evaluation.mappings,
        result_at_execution: evaluation.status,
        diagnostics,
        recorded_at_unix_nanos: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
    })
}

struct ValidationEvaluation {
    mappings: Vec<ScenarioModelMappingAssessment>,
    dependency_fingerprints: BTreeMap<String, String>,
    status: VerificationStatus,
}

fn evaluate_validation(
    roots: &VerificationRoots,
    definition: &ModelValidationDefinition,
) -> Result<ValidationEvaluation, Error> {
    let scenario_definitions = crate::scenario::discover(&roots.specification_root)?;
    let model_definitions = discover(&roots.specification_root)?;
    let mut dependency_fingerprints = BTreeMap::new();
    let mut mappings = Vec::new();
    for mapping in &definition.mappings {
        let scenario = scenario_definitions
            .iter()
            .find(|candidate| candidate.id == mapping.scenario_id)
            .map(|candidate| crate::scenario::latest_assessment(roots, candidate))
            .transpose()?
            .flatten();
        let model = model_definitions
            .iter()
            .find(|candidate| candidate.id == mapping.model_check_id)
            .map(|candidate| latest_assessment(roots, candidate))
            .transpose()?
            .flatten();

        dependency_fingerprints.insert(
            format!("scenario:{}", mapping.scenario_id),
            scenario.as_ref().map_or_else(
                || "missing".into(),
                |assessment| {
                    hash(
                        &serde_json::to_vec(&(
                            &assessment.evidence.scenario_id,
                            &assessment.evidence.scenario_version,
                            &assessment.evidence.provider_version,
                            &assessment.evidence.runner_version,
                            &assessment.evidence.configuration_sha256,
                            &assessment.evidence.inputs,
                            &assessment.evidence.result_at_execution,
                            &assessment.current_validity,
                        ))
                        .expect("scenario dependency serialization"),
                    )
                },
            ),
        );
        dependency_fingerprints.insert(
            format!("model:{}", mapping.model_check_id),
            model.as_ref().map_or_else(
                || "missing".into(),
                |assessment| {
                    hash(
                        &serde_json::to_vec(&(
                            &assessment.evidence.model_check_id,
                            &assessment.evidence.provider_version,
                            &assessment.evidence.configuration_sha256,
                            &assessment.evidence.inputs,
                            &assessment.evidence.result_at_execution,
                            &assessment.current_validity,
                        ))
                        .expect("model dependency serialization"),
                    )
                },
            ),
        );

        let scenario_status = scenario
            .as_ref()
            .map(|assessment| assessment.evidence.result_at_execution.clone());
        let scenario_validity = scenario
            .as_ref()
            .map(|assessment| assessment.current_validity.clone());
        let model_status = model
            .as_ref()
            .map(|assessment| assessment.evidence.result_at_execution.clone());
        let model_validity = model
            .as_ref()
            .map(|assessment| assessment.current_validity.clone());
        let scenario_trace = scenario
            .as_ref()
            .map(|assessment| assessment.evidence.trace.as_slice())
            .unwrap_or_default();
        let (scenario_trace_matches, missing_trace_events) =
            trace_subsequence(scenario_trace, &mapping.trace_pattern);
        let status = mapping_status(
            &mapping.expected_scenario_result,
            scenario_status.as_ref(),
            scenario_validity.as_ref(),
            model_status.as_ref(),
            model_validity.as_ref(),
            scenario_trace_matches,
        );
        mappings.push(ScenarioModelMappingAssessment {
            scenario_id: mapping.scenario_id.clone(),
            expected_scenario_result: mapping.expected_scenario_result.clone(),
            scenario_evidence_id: scenario.as_ref().map(|value| value.evidence.id.clone()),
            scenario_result: scenario_status,
            scenario_validity,
            model_check_id: mapping.model_check_id.clone(),
            model_evidence_id: model.as_ref().map(|value| value.evidence.id.clone()),
            model_result: model_status,
            model_validity,
            trace_pattern: mapping.trace_pattern.clone(),
            scenario_trace_matches,
            missing_trace_events,
            status,
        });
    }
    let status = aggregate_validation_status(&mappings);
    Ok(ValidationEvaluation {
        mappings,
        dependency_fingerprints,
        status,
    })
}

fn mapping_status(
    expected_scenario: &VerificationStatus,
    scenario: Option<&VerificationStatus>,
    scenario_validity: Option<&EvidenceValidity>,
    model: Option<&VerificationStatus>,
    model_validity: Option<&EvidenceValidity>,
    scenario_trace_matches: bool,
) -> VerificationStatus {
    if scenario == Some(&VerificationStatus::Error) || model == Some(&VerificationStatus::Error) {
        VerificationStatus::Error
    } else if scenario_validity == Some(&EvidenceValidity::Stale)
        || model_validity == Some(&EvidenceValidity::Stale)
    {
        VerificationStatus::Stale
    } else if scenario.is_none() || model.is_none() {
        VerificationStatus::Unverified
    } else if scenario != Some(expected_scenario)
        || model != Some(&VerificationStatus::Pass)
        || !scenario_trace_matches
    {
        VerificationStatus::Fail
    } else if scenario_validity == Some(&EvidenceValidity::Current)
        && model_validity == Some(&EvidenceValidity::Current)
    {
        VerificationStatus::Pass
    } else {
        VerificationStatus::Unverified
    }
}

fn trace_subsequence(observed: &[String], expected: &[String]) -> (bool, Vec<String>) {
    let mut cursor = 0usize;
    for item in observed {
        if expected.get(cursor) == Some(item) {
            cursor += 1;
        }
    }
    (cursor == expected.len(), expected[cursor..].to_vec())
}

fn aggregate_validation_status(mappings: &[ScenarioModelMappingAssessment]) -> VerificationStatus {
    if mappings
        .iter()
        .any(|mapping| mapping.status == VerificationStatus::Error)
    {
        VerificationStatus::Error
    } else if mappings
        .iter()
        .any(|mapping| mapping.status == VerificationStatus::Fail)
    {
        VerificationStatus::Fail
    } else if mappings
        .iter()
        .any(|mapping| mapping.status == VerificationStatus::Stale)
    {
        VerificationStatus::Stale
    } else if !mappings.is_empty()
        && mappings
            .iter()
            .all(|mapping| mapping.status == VerificationStatus::Pass)
    {
        VerificationStatus::Pass
    } else {
        VerificationStatus::Unverified
    }
}

fn definition_fingerprint(definition: &ModelValidationDefinition) -> Result<String, Error> {
    let bytes = fs::read(&definition.source).map_err(|source| Error::Io {
        path: definition.source.clone(),
        source,
    })?;
    Ok(hash(&bytes))
}

pub fn store_validation(
    directory: &Path,
    mut evidence: ModelValidationEvidence,
) -> Result<ModelValidationEvidence, Error> {
    fs::create_dir_all(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let seed = serde_json::to_vec(&evidence).expect("model validation evidence serialization");
    evidence.id = EvidenceId(format!("MODEL-VALIDATION-EVIDENCE:{}", &hash(&seed)[..24]));
    let stem = crate::evidence::storage_stem(&evidence.id);
    let target = directory.join(format!("{stem}.json"));
    if !target.exists() {
        let temporary = directory.join(format!(".{stem}.tmp"));
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&evidence).expect("model validation evidence serialization"),
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

pub fn load_all_validations(directory: &Path) -> Result<Vec<ModelValidationEvidence>, Error> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
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
    let mut evidence = paths
        .into_iter()
        .map(|path| {
            serde_json::from_slice::<ModelValidationEvidence>(&fs::read(&path).map_err(
                |source| Error::Io {
                    path: path.clone(),
                    source,
                },
            )?)
            .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    evidence
        .sort_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)));
    Ok(evidence)
}

pub fn assess_validation(
    roots: &VerificationRoots,
    definition: &ModelValidationDefinition,
    evidence: &ModelValidationEvidence,
) -> Result<EvidenceValidity, Error> {
    let evaluation = evaluate_validation(roots, definition)?;
    Ok(
        if evidence.provider_version == PROVIDER_VERSION
            && evidence.definition_sha256 == definition_fingerprint(definition)?
            && evidence.dependency_fingerprints == evaluation.dependency_fingerprints
        {
            EvidenceValidity::Current
        } else {
            EvidenceValidity::Stale
        },
    )
}

pub fn latest_validation_assessment(
    roots: &VerificationRoots,
    definition: &ModelValidationDefinition,
) -> Result<Option<ModelValidationAssessment>, Error> {
    let Some(evidence) = load_all_validations(&roots.state_root.join("model-validation-evidence"))?
        .into_iter()
        .filter(|evidence| evidence.validation_id == definition.id)
        .max_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)))
    else {
        return Ok(None);
    };
    Ok(Some(ModelValidationAssessment {
        current_validity: assess_validation(roots, definition, &evidence)?,
        evidence,
    }))
}

pub fn graph_edges(
    roots: &VerificationRoots,
    definitions: &[ModelCheckDefinition],
    validations: &[ModelValidationDefinition],
) -> Result<Vec<GraphEdge>, Error> {
    let mut edges = Vec::new();
    let history = load_all(&roots.state_root.join("model-evidence"))?;
    for definition in definitions {
        let obligation =
            GraphNode::ProofObligation(ProofObligationId(format!("MODEL:{}", definition.id)));
        edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(roots.spec_identity(&definition.source))),
            kind: LinkKind::Defines,
            to: obligation.clone(),
        });
        edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(
                roots.spec_identity(&roots.specification_root.join(&definition.model)),
            )),
            kind: LinkKind::RelevantTo,
            to: obligation.clone(),
        });
        for evidence in history
            .iter()
            .filter(|evidence| evidence.model_check_id == definition.id)
        {
            edges.push(GraphEdge {
                from: obligation.clone(),
                kind: LinkKind::EvidenceFor,
                to: GraphNode::Evidence(evidence.id.clone()),
            });
        }
    }
    let validation_history =
        load_all_validations(&roots.state_root.join("model-validation-evidence"))?;
    for validation in validations {
        let obligation = GraphNode::ProofObligation(ProofObligationId(format!(
            "MODEL-VALIDATION:{}",
            validation.id
        )));
        edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(roots.spec_identity(&validation.source))),
            kind: LinkKind::Defines,
            to: obligation.clone(),
        });
        for mapping in &validation.mappings {
            edges.push(GraphEdge {
                from: obligation.clone(),
                kind: LinkKind::Requires,
                to: GraphNode::ProofObligation(ProofObligationId(format!(
                    "SCENARIO:{}",
                    mapping.scenario_id
                ))),
            });
            edges.push(GraphEdge {
                from: obligation.clone(),
                kind: LinkKind::Requires,
                to: GraphNode::ProofObligation(ProofObligationId(format!(
                    "MODEL:{}",
                    mapping.model_check_id
                ))),
            });
        }
        for evidence in validation_history
            .iter()
            .filter(|evidence| evidence.validation_id == validation.id)
        {
            edges.push(GraphEdge {
                from: obligation.clone(),
                kind: LinkKind::EvidenceFor,
                to: GraphNode::Evidence(evidence.id.clone()),
            });
        }
    }
    edges.sort_by_key(|edge| serde_json::to_string(edge).expect("model graph serialization"));
    edges.dedup();
    Ok(edges)
}

pub fn write_graph(
    roots: &VerificationRoots,
    definitions: &[ModelCheckDefinition],
    validations: &[ModelValidationDefinition],
) -> Result<PathBuf, Error> {
    fs::create_dir_all(&roots.state_root).map_err(|source| Error::Io {
        path: roots.state_root.clone(),
        source,
    })?;
    let target = roots.state_root.join("model-graph.json");
    let temporary = roots.state_root.join(".model-graph.json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&graph_edges(roots, definitions, validations)?)
            .expect("model graph serialization"),
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
    definition: &ModelCheckDefinition,
) -> Result<Vec<InputFingerprint>, Error> {
    let model = roots.specification_root.join(&definition.model);
    let semantic = vec![
        SemanticInput {
            identity: roots.spec_identity(&definition.source),
            path: definition.source.clone(),
        },
        SemanticInput {
            identity: roots.spec_identity(&model),
            path: model,
        },
    ];
    let mut inputs =
        crate::evidence::fingerprint_semantic_files(&semantic).map_err(|source| Error::Io {
            path: roots.specification_root.clone(),
            source,
        })?;
    inputs.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(inputs)
}

fn validate_model_bindings(
    roots: &VerificationRoots,
    definition: &ModelCheckDefinition,
) -> Result<(), String> {
    if definition.model_bindings.is_empty() {
        return Ok(());
    }
    let model_path = roots.specification_root.join(&definition.model);
    let model = fs::read_to_string(&model_path).map_err(|error| {
        format!(
            "could not read model bindings from {}: {error}",
            model_path.display()
        )
    })?;
    for (selector, quint_name) in &definition.model_bindings {
        let configured = selected_configuration_value(definition, selector).ok_or_else(|| {
            format!("model binding `{selector}` does not select a configured constant or bound")
        })?;
        let expected = quint_literal(configured).map_err(|error| {
            format!("model binding `{selector}` cannot be represented in Quint: {error}")
        })?;
        let observed = pure_value(&model, quint_name).ok_or_else(|| {
            format!(
                "model binding `{selector}` requires `pure val {quint_name} = ...` in {}",
                definition.model.display()
            )
        })?;
        if compact_expression(&observed) != compact_expression(&expected) {
            return Err(format!(
                "model binding mismatch for `{selector}`: configuration requires `{expected}`, but `{quint_name}` is `{observed}`"
            ));
        }
    }
    Ok(())
}

fn selected_configuration_value<'a>(
    definition: &'a ModelCheckDefinition,
    selector: &str,
) -> Option<&'a Value> {
    let (group, name) = selector.split_once('.')?;
    match group {
        "constants" => definition.constants.get(name),
        "bounds" => definition.bounds.get(name),
        _ => None,
    }
}

fn quint_literal(value: &Value) -> Result<String, &'static str> {
    match value {
        Value::Bool(value) => Ok(if *value { "true" } else { "false" }.into()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => serde_json::to_string(value).map_err(|_| "invalid string"),
        Value::Array(values) => values
            .iter()
            .map(quint_literal)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| format!("Set({})", values.join(", "))),
        Value::Null | Value::Object(_) => {
            Err("only booleans, numbers, strings, and arrays are supported")
        }
    }
}

fn pure_value(model: &str, name: &str) -> Option<String> {
    let prefix = format!("pure val {name} =");
    model.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix(&prefix)
            .map(|value| value.trim().to_owned())
    })
}

fn compact_expression(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn detect_backend_version(backend: &ModelCheckerBackend, output: &str) -> Option<String> {
    match backend {
        ModelCheckerBackend::Tlc => output.lines().find_map(|line| {
            line.strip_prefix("TLC2 Version ")
                .and_then(|rest| rest.split_whitespace().next())
                .map(str::to_owned)
        }),
        ModelCheckerBackend::Apalache => output.lines().find_map(|line| {
            let rest = line.strip_prefix("# APALACHE version: ")?;
            rest.split_whitespace().next().map(str::to_owned)
        }),
    }
}

fn parse_stats(output: &str) -> ExploredStateStats {
    let mut stats = ExploredStateStats::default();
    for line in output.lines() {
        if line.contains(" states generated, ") && line.contains(" distinct states found, ") {
            let values = line
                .split(|character: char| !character.is_ascii_digit())
                .filter(|part| !part.is_empty())
                .filter_map(|part| part.parse::<u64>().ok())
                .collect::<Vec<_>>();
            if values.len() >= 3 {
                stats.generated_states = Some(values[0]);
                stats.distinct_states = Some(values[1]);
                stats.states_left = Some(values[2]);
            }
        }
        if let Some(rest) = line.strip_prefix("The depth of the complete state graph search is ") {
            stats.depth = rest.trim_end_matches('.').parse().ok();
        }
    }
    stats
}

fn compact(value: &str) -> String {
    value
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(12)
        .collect::<Vec<_>>()
        .join(" | ")
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
