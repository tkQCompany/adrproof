use crate::Error;
use crate::project::{Artifact, FactCoverage, ProjectFact, Provenance, ProvenanceKind};
use crate::roots::{SemanticInput, VerificationRoots};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const PROTOCOL_VERSION: &str = "adrproof-external-provider-v1";
pub const REQUEST_SCHEMA_VERSION: &str = "adrproof-external-provider-request-v1";
pub const RESPONSE_SCHEMA_VERSION: &str = "adrproof-external-provider-response-v1";
const DEFAULT_TIMEOUT_MS: u64 = 10_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Deserialize)]
struct Configuration {
    timeout_ms: Option<u64>,
    #[serde(default)]
    external_providers: Vec<ExternalProviderDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalProviderDefinition {
    pub id: String,
    pub protocol: String,
    pub version: String,
    pub executable: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub parameters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalProviderRequest {
    pub schema_version: String,
    pub provider_id: String,
    pub provider_version: String,
    pub project_root: PathBuf,
    pub specification_root: PathBuf,
    pub state_root: PathBuf,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalProviderIdentity {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalProviderResponse {
    pub schema_version: String,
    pub provider: ExternalProviderIdentity,
    pub inputs: Vec<String>,
    pub artifacts: Vec<Artifact>,
    pub facts: Vec<ProjectFact>,
    pub coverage: Vec<FactCoverage>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExternalProviderRun {
    pub provider: ExternalProviderIdentity,
    pub executable: PathBuf,
    pub elapsed_ms: u128,
    pub inputs: Vec<SemanticInput>,
    pub artifacts: Vec<Artifact>,
    pub facts: Vec<ProjectFact>,
    pub coverage: Vec<FactCoverage>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug)]
struct CapturedStream {
    bytes: Vec<u8>,
    exceeded_limit: bool,
}

pub fn run_configured(roots: &VerificationRoots) -> Result<Vec<ExternalProviderRun>, Error> {
    run_selected(roots, None)
}

pub fn run_selected(
    roots: &VerificationRoots,
    selected: Option<&str>,
) -> Result<Vec<ExternalProviderRun>, Error> {
    let Some(config_path) = selected_config_path(roots) else {
        return Ok(Vec::new());
    };
    let bytes = fs::read(&config_path).map_err(|source| Error::Io {
        path: config_path.clone(),
        source,
    })?;
    let config: Configuration = serde_json::from_slice(&bytes).map_err(|error| {
        Error::ProviderFailure(format!(
            "{}: invalid configuration: {error}",
            config_path.display()
        ))
    })?;
    if config.external_providers.is_empty() {
        return Ok(Vec::new());
    }

    let config_input = semantic_input_for_path(roots, &config_path)?;
    let base_timeout = config.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    let mut definitions = config.external_providers;
    definitions.sort_by(|left, right| left.id.cmp(&right.id));
    let mut ids = BTreeSet::new();
    let mut runs = Vec::new();
    for definition in definitions {
        validate_definition(&definition, &mut ids)?;
        if selected.is_some_and(|selected| selected != definition.id) {
            continue;
        }
        let executable = resolve_executable(&config_path, &definition.executable)?;
        let executable_input = semantic_input_for_path(roots, &executable)?;
        let timeout_ms = definition.timeout_ms.unwrap_or(base_timeout);
        if timeout_ms == 0 || timeout_ms > MAX_TIMEOUT_MS {
            return Err(Error::ProviderFailure(format!(
                "external provider `{}` timeout must be between 1 and {MAX_TIMEOUT_MS} ms",
                definition.id
            )));
        }
        let mut run = run_one(roots, &definition, &executable, timeout_ms)?;
        run.inputs.push(config_input.clone());
        run.inputs.push(executable_input);
        run.inputs
            .sort_by(|left, right| left.identity.cmp(&right.identity));
        run.inputs
            .dedup_by(|left, right| left.identity == right.identity);
        runs.push(run);
    }
    if let Some(selected) = selected
        && runs.is_empty()
    {
        return Err(Error::ProviderFailure(format!(
            "unknown external provider `{selected}`"
        )));
    }
    Ok(runs)
}

fn selected_config_path(roots: &VerificationRoots) -> Option<PathBuf> {
    let specification = roots.specification_root.join("adrproof.json");
    if specification.is_file() {
        return Some(specification);
    }
    let project = roots.project_root.join("adrproof.json");
    project.is_file().then_some(project)
}

fn validate_definition(
    definition: &ExternalProviderDefinition,
    ids: &mut BTreeSet<String>,
) -> Result<(), Error> {
    if !valid_identifier(&definition.id) {
        return Err(Error::ProviderFailure(format!(
            "external provider id `{}` must contain only ASCII letters, digits, `.`, `_`, or `-`",
            definition.id
        )));
    }
    if !ids.insert(definition.id.clone()) {
        return Err(Error::ProviderFailure(format!(
            "duplicate external provider id `{}`",
            definition.id
        )));
    }
    if definition.protocol != PROTOCOL_VERSION {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` uses unsupported protocol `{}`",
            definition.id, definition.protocol
        )));
    }
    if definition.version.trim().is_empty() {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` has an empty version",
            definition.id
        )));
    }
    if definition.executable.as_os_str().is_empty() {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` has an empty executable path",
            definition.id
        )));
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_relation(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn resolve_executable(config_path: &Path, configured: &Path) -> Result<PathBuf, Error> {
    let path = if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        config_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(configured)
    };
    let path = fs::canonicalize(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    if !path.is_file() {
        return Err(Error::ProviderFailure(format!(
            "external provider executable {} is not a file",
            path.display()
        )));
    }
    Ok(path)
}

fn semantic_input_for_path(roots: &VerificationRoots, path: &Path) -> Result<SemanticInput, Error> {
    let canonical_path = fs::canonicalize(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let canonical_specification =
        fs::canonicalize(&roots.specification_root).map_err(|source| Error::Io {
            path: roots.specification_root.clone(),
            source,
        })?;
    if canonical_path.starts_with(&canonical_specification) {
        return Ok(SemanticInput {
            identity: roots.spec_identity(path),
            path: canonical_path,
        });
    }
    let canonical_project = fs::canonicalize(&roots.project_root).map_err(|source| Error::Io {
        path: roots.project_root.clone(),
        source,
    })?;
    if canonical_path.starts_with(&canonical_project) {
        return Ok(SemanticInput {
            identity: roots.project_identity(path),
            path: canonical_path,
        });
    }
    Err(Error::ProviderFailure(format!(
        "external provider executable/configuration {} must be inside project_root or specification_root",
        path.display()
    )))
}

fn run_one(
    roots: &VerificationRoots,
    definition: &ExternalProviderDefinition,
    executable: &Path,
    timeout_ms: u64,
) -> Result<ExternalProviderRun, Error> {
    fs::create_dir_all(&roots.state_root).map_err(|source| Error::Io {
        path: roots.state_root.clone(),
        source,
    })?;
    let request = ExternalProviderRequest {
        schema_version: REQUEST_SCHEMA_VERSION.into(),
        provider_id: definition.id.clone(),
        provider_version: definition.version.clone(),
        project_root: roots.project_root.clone(),
        specification_root: roots.specification_root.clone(),
        state_root: roots.state_root.clone(),
        parameters: definition.parameters.clone(),
    };
    let request = serde_json::to_vec(&request).expect("external provider request serialization");

    let mut command = Command::new(executable);
    command
        .args(&definition.args)
        .current_dir(&roots.state_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|error| {
        Error::ProviderFailure(format!(
            "external provider `{}` could not start: {error}",
            definition.id
        ))
    })?;
    let stdout = child.stdout.take().expect("piped provider stdout");
    let stderr = child.stderr.take().expect("piped provider stderr");
    let stdout_reader = thread::spawn(move || read_limited(stdout));
    let stderr_reader = thread::spawn(move || read_limited(stderr));
    if let Some(mut stdin) = child.stdin.take()
        && let Err(error) = stdin.write_all(&request)
    {
        kill_process_tree(&mut child);
        let _ = child.wait();
        let _ = stdout_reader.join();
        let _ = stderr_reader.join();
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` request write failed: {error}",
            definition.id
        )));
    }

    let started = Instant::now();
    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            Error::ProviderFailure(format!(
                "external provider `{}` wait failed: {error}",
                definition.id
            ))
        })? {
            break (status, false);
        }
        if started.elapsed() >= Duration::from_millis(timeout_ms) {
            kill_process_tree(&mut child);
            let status = child.wait().map_err(|error| {
                Error::ProviderFailure(format!(
                    "external provider `{}` timeout cleanup failed: {error}",
                    definition.id
                ))
            })?;
            break (status, true);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = join_reader(stdout_reader, &definition.id, "stdout")?;
    let stderr = join_reader(stderr_reader, &definition.id, "stderr")?;
    if timed_out {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` timed out after {timeout_ms} ms",
            definition.id
        )));
    }
    if stdout.exceeded_limit || stderr.exceeded_limit {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` exceeded the {MAX_OUTPUT_BYTES}-byte output limit",
            definition.id
        )));
    }
    let stderr = String::from_utf8(stderr.bytes).map_err(|_| {
        Error::ProviderFailure(format!(
            "external provider `{}` wrote non-UTF-8 stderr",
            definition.id
        ))
    })?;
    if !status.success() {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` exited with {}: {}",
            definition.id,
            status,
            stderr.trim()
        )));
    }
    let response_value: serde_json::Value =
        serde_json::from_slice(&stdout.bytes).map_err(|error| {
            Error::ProviderFailure(format!(
                "external provider `{}` returned invalid JSON: {error}",
                definition.id
            ))
        })?;
    validate_wire_shape(&response_value, &definition.id)?;
    let mut response: ExternalProviderResponse =
        serde_json::from_value(response_value).map_err(|error| {
            Error::ProviderFailure(format!(
                "external provider `{}` response does not match v1: {error}",
                definition.id
            ))
        })?;
    if !stderr.trim().is_empty() {
        response
            .diagnostics
            .push(format!("provider stderr: {}", stderr.trim()));
    }
    validate_response(roots, definition, executable, started.elapsed(), response)
}

fn validate_wire_shape(value: &serde_json::Value, provider: &str) -> Result<(), Error> {
    let response = exact_object(
        value,
        &[
            "schema_version",
            "provider",
            "inputs",
            "artifacts",
            "facts",
            "coverage",
            "diagnostics",
        ],
        provider,
        "response",
    )?;
    exact_object(
        &response["provider"],
        &["id", "version"],
        provider,
        "provider identity",
    )?;
    for artifact in exact_array(&response["artifacts"], provider, "artifacts")? {
        let artifact = exact_object(
            artifact,
            &["id", "kind", "provenance"],
            provider,
            "artifact",
        )?;
        validate_provenance_shape(&artifact["provenance"], provider)?;
    }
    for fact in exact_array(&response["facts"], provider, "facts")? {
        let fact = exact_object(
            fact,
            &[
                "id",
                "relation",
                "arguments",
                "value",
                "attributes",
                "provenance",
            ],
            provider,
            "fact",
        )?;
        validate_provenance_shape(&fact["provenance"], provider)?;
    }
    for coverage in exact_array(&response["coverage"], provider, "coverage")? {
        let coverage = exact_object(
            coverage,
            &[
                "relation",
                "provider",
                "world",
                "scope",
                "qualifiers",
                "statement",
                "diagnostics",
            ],
            provider,
            "coverage",
        )?;
        let scope = coverage["scope"].as_object().ok_or_else(|| {
            Error::ProviderFailure(format!(
                "external provider `{provider}` coverage scope must be an object"
            ))
        })?;
        let scope_fields = if scope.get("kind").and_then(|kind| kind.as_str()) == Some("global") {
            &["kind"][..]
        } else {
            &["kind", "name"][..]
        };
        exact_object(&coverage["scope"], scope_fields, provider, "coverage scope")?;
        for diagnostic in exact_array(&coverage["diagnostics"], provider, "coverage diagnostics")? {
            let diagnostic = exact_object(
                diagnostic,
                &["reason", "provenance"],
                provider,
                "coverage diagnostic",
            )?;
            validate_provenance_shape(&diagnostic["provenance"], provider)?;
        }
    }
    exact_array(&response["inputs"], provider, "inputs")?;
    exact_array(&response["diagnostics"], provider, "diagnostics")?;
    Ok(())
}

fn validate_provenance_shape(value: &serde_json::Value, provider: &str) -> Result<(), Error> {
    let provenance = exact_object(
        value,
        &["kind", "source", "span", "extractor"],
        provider,
        "provenance",
    )?;
    if !provenance["span"].is_null() {
        exact_object(
            &provenance["span"],
            &["filename", "line", "column"],
            provider,
            "source span",
        )?;
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a serde_json::Value,
    fields: &[&str],
    provider: &str,
    context: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Error> {
    let object = value.as_object().ok_or_else(|| {
        Error::ProviderFailure(format!(
            "external provider `{provider}` {context} must be an object"
        ))
    })?;
    let expected = fields.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let unknown = actual.difference(&expected).copied().collect::<Vec<_>>();
        return Err(Error::ProviderFailure(format!(
            "external provider `{provider}` {context} fields mismatch; missing={missing:?}, unknown={unknown:?}"
        )));
    }
    Ok(object)
}

fn exact_array<'a>(
    value: &'a serde_json::Value,
    provider: &str,
    context: &str,
) -> Result<&'a Vec<serde_json::Value>, Error> {
    value.as_array().ok_or_else(|| {
        Error::ProviderFailure(format!(
            "external provider `{provider}` {context} must be an array"
        ))
    })
}

fn read_limited(mut stream: impl Read) -> std::io::Result<CapturedStream> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    let mut exceeded_limit = false;
    loop {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        let remaining = MAX_OUTPUT_BYTES
            .saturating_add(1)
            .saturating_sub(bytes.len());
        if remaining > 0 {
            bytes.extend_from_slice(&buffer[..count.min(remaining)]);
        }
        if bytes.len() > MAX_OUTPUT_BYTES {
            exceeded_limit = true;
        }
    }
    Ok(CapturedStream {
        bytes,
        exceeded_limit,
    })
}

fn join_reader(
    reader: thread::JoinHandle<std::io::Result<CapturedStream>>,
    provider: &str,
    stream: &str,
) -> Result<CapturedStream, Error> {
    reader
        .join()
        .map_err(|_| {
            Error::ProviderFailure(format!(
                "external provider `{provider}` {stream} reader panicked"
            ))
        })?
        .map_err(|error| {
            Error::ProviderFailure(format!(
                "external provider `{provider}` {stream} read failed: {error}"
            ))
        })
}

fn kill_process_tree(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

fn validate_response(
    roots: &VerificationRoots,
    definition: &ExternalProviderDefinition,
    executable: &Path,
    elapsed: Duration,
    mut response: ExternalProviderResponse,
) -> Result<ExternalProviderRun, Error> {
    if response.schema_version != RESPONSE_SCHEMA_VERSION {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` returned unsupported schema `{}`",
            definition.id, response.schema_version
        )));
    }
    if response.provider.id != definition.id || response.provider.version != definition.version {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` response identity/version does not match configuration",
            definition.id
        )));
    }

    response.inputs.sort();
    if response.inputs.windows(2).any(|items| items[0] == items[1]) {
        return Err(Error::ProviderFailure(format!(
            "external provider `{}` returned duplicate semantic inputs",
            definition.id
        )));
    }
    let mut inputs = Vec::new();
    let mut input_ids = BTreeSet::new();
    for identity in &response.inputs {
        let input = resolve_declared_input(roots, identity)?;
        input_ids.insert(input.identity.clone());
        inputs.push(input);
    }

    response
        .artifacts
        .sort_by(|left, right| left.id.cmp(&right.id));
    response.facts.sort_by(|left, right| left.id.cmp(&right.id));
    response.coverage.sort_by_key(|coverage| {
        serde_json::to_string(coverage).expect("external coverage serialization")
    });
    response.diagnostics.sort();
    response.diagnostics.dedup();

    ensure_unique(
        response.artifacts.iter().map(|artifact| &artifact.id.0),
        &definition.id,
        "artifact",
    )?;
    ensure_unique(
        response.facts.iter().map(|fact| &fact.id.0),
        &definition.id,
        "fact",
    )?;
    let artifact_ids = response
        .artifacts
        .iter()
        .map(|artifact| artifact.id.0.clone())
        .collect::<BTreeSet<_>>();
    let extractor = format!("external:{}@{}", definition.id, definition.version);
    for artifact in &mut response.artifacts {
        validate_provenance(
            &mut artifact.provenance,
            &input_ids,
            &extractor,
            &definition.id,
        )?;
        if artifact.id.0.trim().is_empty() || artifact.kind.trim().is_empty() {
            return Err(Error::ProviderFailure(format!(
                "external provider `{}` returned an artifact with an empty id or kind",
                definition.id
            )));
        }
    }
    for fact in &mut response.facts {
        if !fact.id.0.starts_with(&format!("{}:", definition.id)) {
            return Err(Error::ProviderFailure(format!(
                "external provider `{}` fact id `{}` must start with `{}:`",
                definition.id, fact.id.0, definition.id
            )));
        }
        if !valid_relation(&fact.relation) || fact.arguments.is_empty() || !fact.value {
            return Err(Error::ProviderFailure(format!(
                "external provider `{}` facts must be positive, have arguments, and use ADRLogic relation names",
                definition.id
            )));
        }
        validate_provenance(&mut fact.provenance, &input_ids, &extractor, &definition.id)?;
        let source = fact.provenance.source.to_string_lossy();
        if !artifact_ids.contains(source.as_ref()) {
            return Err(Error::ProviderFailure(format!(
                "external provider `{}` must return a source artifact `{source}` for fact `{}`",
                definition.id, fact.id.0
            )));
        }
    }
    for coverage in &mut response.coverage {
        if coverage.provider != definition.id || !valid_relation(&coverage.relation) {
            return Err(Error::ProviderFailure(format!(
                "external provider `{}` returned invalid coverage ownership/relation",
                definition.id
            )));
        }
        if coverage.statement.trim().is_empty() {
            return Err(Error::ProviderFailure(format!(
                "external provider `{}` returned coverage without a statement",
                definition.id
            )));
        }
        for diagnostic in &mut coverage.diagnostics {
            validate_provenance(
                &mut diagnostic.provenance,
                &input_ids,
                &extractor,
                &definition.id,
            )?;
        }
    }

    Ok(ExternalProviderRun {
        provider: response.provider,
        executable: executable.to_path_buf(),
        elapsed_ms: elapsed.as_millis(),
        inputs,
        artifacts: response.artifacts,
        facts: response.facts,
        coverage: response.coverage,
        diagnostics: response.diagnostics,
    })
}

fn validate_provenance(
    provenance: &mut Provenance,
    inputs: &BTreeSet<String>,
    extractor: &str,
    provider: &str,
) -> Result<(), Error> {
    if !matches!(
        provenance.kind,
        ProvenanceKind::DeterministicallyExtracted | ProvenanceKind::Authoritative
    ) {
        return Err(Error::ProviderFailure(format!(
            "external provider `{provider}` may emit only deterministically_extracted or authoritative provenance"
        )));
    }
    let source = provenance.source.to_string_lossy();
    if !inputs.contains(source.as_ref()) {
        return Err(Error::ProviderFailure(format!(
            "external provider `{provider}` provenance source `{source}` is not a declared input"
        )));
    }
    if let Some(span) = &provenance.span
        && span.filename != provenance.source
    {
        return Err(Error::ProviderFailure(format!(
            "external provider `{provider}` span filename must equal its provenance source"
        )));
    }
    provenance.extractor = Some(match provenance.extractor.take() {
        Some(value) if !value.trim().is_empty() => format!("{extractor}; {value}"),
        _ => extractor.into(),
    });
    Ok(())
}

fn ensure_unique<'a>(
    values: impl IntoIterator<Item = &'a String>,
    provider: &str,
    kind: &str,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(Error::ProviderFailure(format!(
                "external provider `{provider}` returned duplicate {kind} id `{value}`"
            )));
        }
    }
    Ok(())
}

fn resolve_declared_input(
    roots: &VerificationRoots,
    identity: &str,
) -> Result<SemanticInput, Error> {
    let (root, relative) = if let Some(relative) = identity.strip_prefix("project:") {
        (&roots.project_root, relative)
    } else if let Some(relative) = identity.strip_prefix("spec:") {
        (&roots.specification_root, relative)
    } else {
        return Err(Error::ProviderFailure(format!(
            "external provider input `{identity}` must use project: or spec: namespace"
        )));
    };
    if relative.is_empty()
        || relative.contains('\\')
        || Path::new(relative)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(Error::ProviderFailure(format!(
            "external provider input `{identity}` is not a normalized logical path"
        )));
    }
    let root_canonical = fs::canonicalize(root).map_err(|source| Error::Io {
        path: root.clone(),
        source,
    })?;
    let path = root.join(relative);
    let canonical = fs::canonicalize(&path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    if !canonical.starts_with(&root_canonical) || !canonical.is_file() {
        return Err(Error::ProviderFailure(format!(
            "external provider input `{identity}` must resolve to a file inside its root"
        )));
    }
    Ok(SemanticInput {
        identity: identity.into(),
        path: canonical,
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::roots::VerificationRoots;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    fn test_roots() -> VerificationRoots {
        let root = std::env::temp_dir().join(format!(
            "adrproof-external-provider-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        let project = root.join("project");
        let specification = root.join("specification");
        let state = root.join("state");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&specification).unwrap();
        fs::write(project.join("input.txt"), "component=api\n").unwrap();
        VerificationRoots::explicit(&project, &specification, &state)
    }

    fn response(provenance_kind: &str, source: &str) -> String {
        format!(
            r#"{{
  "schema_version": "adrproof-external-provider-response-v1",
  "provider": {{"id": "fixture", "version": "1.0.0"}},
  "inputs": ["project:input.txt"],
  "artifacts": [{{
    "id": "project:input.txt",
    "kind": "fixture_manifest",
    "provenance": {{"kind": "{provenance_kind}", "source": "{source}", "span": null, "extractor": null}}
  }}],
  "facts": [{{
    "id": "fixture:component:api",
    "relation": "component",
    "arguments": ["api"],
    "value": true,
    "attributes": {{}},
    "provenance": {{"kind": "{provenance_kind}", "source": "{source}", "span": null, "extractor": "fixture-parser"}}
  }}],
  "coverage": [{{
    "relation": "component",
    "provider": "fixture",
    "world": "closed",
    "scope": {{"kind": "global"}},
    "qualifiers": {{}},
    "statement": "all components in input.txt are enumerated",
    "diagnostics": []
  }}],
  "diagnostics": []
}}"#
        )
    }

    fn configure(roots: &VerificationRoots, script_body: &str, timeout_ms: u64) {
        let executable = roots.specification_root.join("provider.sh");
        fs::write(&executable, format!("#!/bin/sh\n{script_body}\n")).unwrap();
        let mut permissions = fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&executable, permissions).unwrap();
        fs::write(
            roots.specification_root.join("adrproof.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "z3_version": "4.13.4",
                "timeout_ms": 10_000,
                "external_providers": [{
                    "id": "fixture",
                    "protocol": PROTOCOL_VERSION,
                    "version": "1.0.0",
                    "executable": "provider.sh",
                    "timeout_ms": timeout_ms,
                    "parameters": {"mode": "test"}
                }]
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn output_script(response: &str) -> String {
        format!(
            "cat >/dev/null\nprintf '%s\\n' '{}'",
            response.replace('\'', "'\\''")
        )
    }

    #[test]
    fn configured_provider_returns_normalized_facts_and_semantic_inputs() {
        let roots = test_roots();
        configure(
            &roots,
            &output_script(&response(
                "deterministically_extracted",
                "project:input.txt",
            )),
            1_000,
        );
        let runs = run_configured(&roots).unwrap();
        assert_eq!(runs.len(), 1);
        let run = &runs[0];
        assert_eq!(run.provider.id, "fixture");
        assert_eq!(run.facts.len(), 1);
        assert_eq!(
            run.facts[0].provenance.extractor.as_deref(),
            Some("external:fixture@1.0.0; fixture-parser")
        );
        assert_eq!(
            run.inputs
                .iter()
                .map(|input| input.identity.as_str())
                .collect::<Vec<_>>(),
            vec![
                "project:input.txt",
                "spec:adrproof.json",
                "spec:provider.sh"
            ]
        );
    }

    #[test]
    fn configured_provider_is_merged_into_the_project_model() {
        let roots = test_roots();
        fs::write(
            roots.specification_root.join("architecture.md"),
            "---\nid: TEST\nstatus: accepted\n---\n\n```adrlogic\nentity Component { api }; relation component(Component); rule C1 \"api exists\" { component(api); }\n```\n",
        )
        .unwrap();
        configure(
            &roots,
            &output_script(&response("authoritative", "project:input.txt")),
            1_000,
        );
        let (model, inputs) = crate::load_project_model_with_roots(&roots).unwrap();
        assert!(
            model
                .facts
                .contains_key(&crate::project::FactId("fixture:component:api".into()))
        );
        assert_eq!(
            model.coverage_for("component", &crate::project::CoverageScope::Global),
            Some(crate::project::WorldAssumption::Closed)
        );
        assert!(
            inputs
                .iter()
                .any(|input| input.identity == "spec:provider.sh")
        );
        let relevant = crate::relevant_semantic_inputs(&model, &inputs)
            .into_iter()
            .map(|input| input.identity)
            .collect::<BTreeSet<_>>();
        assert!(relevant.contains("project:input.txt"));
        assert!(relevant.contains("spec:adrproof.json"));
        assert!(relevant.contains("spec:provider.sh"));
    }

    #[test]
    fn provider_timeout_fails_closed() {
        let roots = test_roots();
        configure(&roots, "cat >/dev/null\nsleep 2", 20);
        let error = run_configured(&roots).unwrap_err().to_string();
        assert!(error.contains("timed out after 20 ms"), "{error}");
    }

    #[test]
    fn llm_derived_provider_fact_is_rejected() {
        let roots = test_roots();
        configure(
            &roots,
            &output_script(&response("llm_derived", "project:input.txt")),
            1_000,
        );
        let error = run_configured(&roots).unwrap_err().to_string();
        assert!(error.contains("may emit only"), "{error}");
    }

    #[test]
    fn undeclared_provenance_source_is_rejected() {
        let roots = test_roots();
        configure(
            &roots,
            &output_script(&response(
                "deterministically_extracted",
                "project:not-declared.txt",
            )),
            1_000,
        );
        let error = run_configured(&roots).unwrap_err().to_string();
        assert!(error.contains("is not a declared input"), "{error}");
    }

    #[test]
    fn unknown_response_fields_are_rejected() {
        let roots = test_roots();
        let changed = response("deterministically_extracted", "project:input.txt").replacen(
            "\"diagnostics\": []",
            "\"unknown\": true, \"diagnostics\": []",
            1,
        );
        configure(&roots, &output_script(&changed), 1_000);
        let error = run_configured(&roots).unwrap_err().to_string();
        assert!(error.contains("unknown=[\"unknown\"]"), "{error}");
    }

    #[test]
    fn provider_configuration_and_executable_control_semantic_fingerprints() {
        let roots = test_roots();
        fs::write(
            roots.specification_root.join("architecture.md"),
            "---\nid: TEST\nstatus: accepted\n---\n\n```adrlogic\nentity Component { api }; relation component(Component); rule C1 \"api exists\" { component(api); }\n```\n",
        )
        .unwrap();
        let body = output_script(&response(
            "deterministically_extracted",
            "project:input.txt",
        ));
        configure(&roots, &body, 1_000);

        let fingerprints = || {
            let (model, inputs) = crate::load_project_model_with_roots(&roots).unwrap();
            let relevant = crate::relevant_semantic_inputs(&model, &inputs);
            (
                model.facts,
                crate::evidence::fingerprint_semantic_files(&relevant).unwrap(),
            )
        };
        let (original_facts, original) = fingerprints();

        let executable = roots.specification_root.join("provider.sh");
        fs::write(&executable, format!("#!/bin/sh\n{body}\n# changed bytes\n")).unwrap();
        let (executable_changed_facts, executable_changed) = fingerprints();
        assert_eq!(original_facts, executable_changed_facts);
        assert_ne!(original, executable_changed);

        let config = roots.specification_root.join("adrproof.json");
        let changed = fs::read_to_string(&config)
            .unwrap()
            .replace("\"mode\": \"test\"", "\"mode\": \"changed\"");
        fs::write(config, changed).unwrap();
        let (configuration_changed_facts, configuration_changed) = fingerprints();
        assert_eq!(original_facts, configuration_changed_facts);
        assert_ne!(executable_changed, configuration_changed);
    }
}
