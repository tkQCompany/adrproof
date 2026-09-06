//! Snapshots from an externally trusted execution boundary, never a sandbox.
use crate::{
    Error, evidence,
    project::ProjectModel,
    roots::{SemanticInput, VerificationRoots},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

pub const SCHEMA: &str = "adrproof-fact-snapshot-v1alpha1";
const MAX_ENTRIES: usize = 100_000;
const MAX_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeEntry {
    pub source: String,
    pub kind: String,
    pub sha256: String,
    pub permissions: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FactSnapshot {
    pub schema_version: String,
    pub producer_context_sha256: String,
    pub tree: Vec<TreeEntry>,
    pub provider_policy: Vec<TreeEntry>,
    pub semantic_inputs: Vec<evidence::InputFingerprint>,
    pub model: ProjectModel,
}

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    pub snapshot_sha256: String,
    pub producer_context_sha256: String,
    pub authority: &'static str,
}

#[derive(Debug)]
pub struct ValidatedSnapshot {
    pub(crate) snapshot: FactSnapshot,
    pub(crate) receipt: Receipt,
}
impl ValidatedSnapshot {
    pub fn receipt(&self) -> &Receipt {
        &self.receipt
    }
    pub(crate) fn inputs(&self, roots: &VerificationRoots) -> Vec<SemanticInput> {
        self.snapshot
            .semantic_inputs
            .iter()
            .map(|i| SemanticInput {
                identity: i.source.clone(),
                path: roots
                    .resolve_identity(&i.source)
                    .expect("validated semantic identity"),
            })
            .collect()
    }
}

fn invalid(message: impl Into<String>) -> Error {
    crate::diag(Path::new("<fact-snapshot>"), 1, 1, message)
}
pub(crate) fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}

fn tree(roots: &VerificationRoots) -> Result<Vec<TreeEntry>, Error> {
    crate::gate::validate_roots(roots)?;
    let project = fs::canonicalize(&roots.project_root).map_err(|e| io(&roots.project_root, e))?;
    let spec = fs::canonicalize(&roots.specification_root)
        .map_err(|e| io(&roots.specification_root, e))?;
    if project.starts_with(&spec) || spec.starts_with(&project) {
        return Err(invalid(
            "snapshot project and specification roots must be disjoint",
        ));
    }
    let mut entries = Vec::new();
    let mut bytes = 0;
    for (namespace, root) in [
        ("project", &roots.project_root),
        ("spec", &roots.specification_root),
    ] {
        walk(root, root, namespace, 0, &mut entries, &mut bytes)?;
    }
    entries.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(entries)
}

fn walk(
    root: &Path,
    path: &Path,
    namespace: &str,
    depth: usize,
    entries: &mut Vec<TreeEntry>,
    bytes: &mut u64,
) -> Result<(), Error> {
    if depth > 128 || entries.len() >= MAX_ENTRIES {
        return Err(invalid("snapshot tree limit exceeded"));
    }
    // Root aliases are resolved by root validation; child aliases are never followed.
    let metadata = if depth == 0 {
        fs::metadata(path)
    } else {
        fs::symlink_metadata(path)
    }
    .map_err(|e| io(path, e))?;
    if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
        return Err(invalid(
            "snapshot requires plain files/directories without child aliases",
        ));
    }
    if depth > 0
        && path
            .file_name()
            .is_some_and(|n| n == ".git" || n == "target" || n == ".adrproof")
    {
        return Err(invalid(
            "snapshot requires a source export without .git, target or .adrproof",
        ));
    }
    let relative = path
        .strip_prefix(root)
        .expect("walk below root")
        .components()
        .map(|c| {
            c.as_os_str()
                .to_str()
                .ok_or_else(|| invalid("non-UTF8 snapshot path"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join("/");
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o7777
    };
    #[cfg(not(unix))]
    let permissions = u32::from(metadata.permissions().readonly());
    let sha256 = if metadata.is_file() {
        *bytes = bytes
            .checked_add(metadata.len())
            .ok_or_else(|| invalid("snapshot byte limit exceeded"))?;
        if *bytes > MAX_BYTES {
            return Err(invalid("snapshot byte limit exceeded"));
        }
        use std::io::Read;
        let mut data = Vec::new();
        fs::File::open(path)
            .map_err(|e| io(path, e))?
            .take(metadata.len() + 1)
            .read_to_end(&mut data)
            .map_err(|e| io(path, e))?;
        if data.len() as u64 != metadata.len() {
            return Err(invalid("input changed while scanning"));
        }
        evidence::fingerprint_bytes("file", &data).sha256
    } else {
        evidence::fingerprint_bytes("directory", b"").sha256
    };
    entries.push(TreeEntry {
        source: format!("{namespace}:{relative}"),
        kind: if metadata.is_dir() {
            "directory"
        } else {
            "file"
        }
        .into(),
        sha256,
        permissions,
    });
    if metadata.is_dir() {
        let mut paths = fs::read_dir(path)
            .map_err(|e| io(path, e))?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| io(path, e))?;
        paths.sort();
        for child in paths {
            walk(root, &child, namespace, depth + 1, entries, bytes)?;
        }
    }
    Ok(())
}

fn validate_semantic(snapshot: &FactSnapshot) -> Result<(), Error> {
    let tree = snapshot
        .tree
        .iter()
        .map(|e| (e.source.as_str(), e))
        .collect::<BTreeMap<_, _>>();
    let mut previous: Option<&str> = None;
    for input in &snapshot.semantic_inputs {
        if previous.is_some_and(|p| p >= input.source.as_str()) {
            return Err(invalid("semantic identities must be unique and sorted"));
        }
        previous = Some(&input.source);
        let entry = tree
            .get(input.source.as_str())
            .ok_or_else(|| invalid("semantic input escapes or is absent from source tree"))?;
        if entry.kind != "file" || entry.sha256 != input.sha256 {
            return Err(invalid("semantic input fingerprint mismatch"));
        }
    }
    if snapshot.semantic_inputs.is_empty() {
        return Err(invalid("empty semantic input set"));
    }
    Ok(())
}

fn provider_policy(roots: &VerificationRoots, tree: &[TreeEntry]) -> Result<Vec<TreeEntry>, Error> {
    let inputs = crate::external_provider::execution_inputs(roots)?;
    for input in &inputs {
        if !tree
            .iter()
            .any(|e| e.source == input.identity && e.kind == "file")
        {
            return Err(invalid(
                "provider configuration/executable must be inside the source export",
            ));
        }
    }
    Ok(tree
        .iter()
        .filter(|e| e.source.starts_with("spec:") || inputs.iter().any(|i| i.identity == e.source))
        .cloned()
        .collect())
}

/// EXECUTES configured providers. Caller must supply isolation and trusted context.
pub fn capture(
    roots: &VerificationRoots,
    producer_context_sha256: &str,
) -> Result<FactSnapshot, Error> {
    if !is_hash(producer_context_sha256) {
        return Err(invalid("explicit producer context SHA-256 required"));
    }
    let before = tree(roots)?;
    let provider_policy = provider_policy(roots, &before)?;
    let (model, inputs) = crate::load_project_model_with_roots(roots)?;
    for input in &inputs {
        if !before
            .iter()
            .any(|e| e.source == input.identity && e.kind == "file")
        {
            return Err(invalid(
                "semantic input escapes or is absent from source tree",
            ));
        }
        let resolved = roots
            .resolve_identity(&input.identity)
            .ok_or_else(|| invalid("invalid semantic namespace"))?;
        if fs::canonicalize(&resolved).map_err(|e| io(&resolved, e))?
            != fs::canonicalize(&input.path).map_err(|e| io(&input.path, e))?
        {
            return Err(invalid("semantic identity and physical input disagree"));
        }
    }
    let semantic_inputs =
        evidence::fingerprint_semantic_files(&inputs).map_err(|e| io(&roots.project_root, e))?;
    if before != tree(roots)? {
        return Err(invalid(
            "CAPTURE_INPUT_DRIFT: extraction changed its source export",
        ));
    }
    let snapshot = FactSnapshot {
        schema_version: SCHEMA.into(),
        producer_context_sha256: producer_context_sha256.into(),
        tree: before,
        provider_policy,
        semantic_inputs,
        model,
    };
    validate_semantic(&snapshot)?;
    Ok(snapshot)
}

/// Read-only validation. The pin must arrive through a trusted producer channel.
pub fn validate(
    roots: &VerificationRoots,
    path: &Path,
    pin: &str,
) -> Result<ValidatedSnapshot, Error> {
    if !is_hash(pin) {
        return Err(invalid("explicit snapshot SHA-256 required"));
    }
    let bytes = fs::read(path).map_err(|e| io(path, e))?;
    if evidence::fingerprint_bytes("snapshot", &bytes).sha256 != pin {
        return Err(invalid("SNAPSHOT_PIN_MISMATCH"));
    }
    let snapshot: FactSnapshot =
        serde_json::from_slice(&bytes).map_err(|e| invalid(format!("invalid snapshot: {e}")))?;
    let original: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    if snapshot.schema_version != SCHEMA
        || !is_hash(&snapshot.producer_context_sha256)
        || serde_json::to_value(&snapshot).unwrap() != original
    {
        return Err(invalid("unsupported snapshot schema or fields"));
    }
    if snapshot.tree != tree(roots)? {
        return Err(invalid(
            "SNAPSHOT_STALE: source tree contents, paths or permissions differ",
        ));
    }
    validate_semantic(&snapshot)?;
    if snapshot.provider_policy != provider_policy(roots, &snapshot.tree)? {
        return Err(invalid("SNAPSHOT_PROVIDER_POLICY_MISMATCH"));
    }
    let adrs = crate::load_adrs(&roots.specification_root)?;
    let effective = crate::effective(&adrs, false)?;
    let mut expected = crate::lower_to_project_model(&adrs, &effective);
    crate::namespace_model_artifacts(&mut expected, "spec", &roots.specification_root);
    if snapshot.model.constraints != expected.constraints
        || snapshot.model.decisions != expected.decisions
    {
        return Err(invalid(
            "SNAPSHOT_SPEC_MISMATCH: captured obligations differ from current specification",
        ));
    }
    let receipt = Receipt {
        snapshot_sha256: pin.into(),
        producer_context_sha256: snapshot.producer_context_sha256.clone(),
        authority: "externally_pinned_snapshot_and_declared_producer_context",
    };
    Ok(ValidatedSnapshot { snapshot, receipt })
}
