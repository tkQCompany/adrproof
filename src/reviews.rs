//! Unsigned, externally authorized formalization attestations; never proof evidence.
use crate::evidence::{InputFingerprint, fingerprint_bytes};
use crate::inventory::{InventoryReport, inspect};
use crate::project::{DeclaredMapping, IntentRequirement, RequirementId, RequirementKind};
use crate::{Error, diag};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const RECORD_SCHEMA: &str = "adrproof-formalization-review-v1alpha1";
pub const REPORT_SCHEMA: &str = "adrproof-review-report-v1alpha1";
const STORE: &str = "formalization-reviews";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub requirement: RequirementId,
    pub selection: IntentRequirement,
    pub mapping_sha256: String,
    pub formalization_sha256: String,
    pub inputs: Vec<InputFingerprint>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Draft,
    Approved,
    NoSemanticChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reviewer {
    pub kind: String,
    pub identity: String,
    pub approval_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub schema_version: String,
    pub binding: Binding,
    pub decision: Decision,
    pub reviewer: Option<Reviewer>,
    pub rationale: Option<String>,
    pub supersedes: Option<String>,
}

impl Review {
    pub fn id(&self) -> String {
        digest(self)
    }
}

#[derive(Debug, Serialize)]
pub struct Assessment {
    pub requirement: RequirementId,
    pub status: &'static str,
    pub latest_review: Option<String>,
    pub changed_inputs: Vec<String>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ReviewReport {
    pub schema_version: &'static str,
    pub result: &'static str,
    pub authority: &'static str,
    pub verification_status: &'static str,
    pub inventory_gaps: Vec<crate::inventory::Gap>,
    pub requirements: Vec<Assessment>,
    pub history: BTreeMap<String, Review>,
}
impl ReviewReport {
    pub fn exit_code(&self) -> i32 {
        if self.result == "REVIEWS_CURRENT" {
            0
        } else {
            3
        }
    }
}

fn digest(value: &impl Serialize) -> String {
    fingerprint_bytes(
        "review",
        &serde_json::to_vec(value).expect("review serialization"),
    )
    .sha256
}
fn invalid(message: impl Into<String>) -> Error {
    diag(Path::new("<review>"), 1, 1, message)
}
fn io(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.into(),
        source,
    }
}
fn nonempty(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}
fn sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn resolve(path: &Path) -> Result<PathBuf, Error> {
    // Resolve existing aliases even when the final state directory is not created.
    let absolute = crate::roots::VerificationRoots::legacy(path, path).project_root;
    let mut ancestor = absolute.as_path();
    let mut suffix = Vec::new();
    loop {
        match fs::canonicalize(ancestor) {
            Ok(mut resolved) => {
                for part in suffix.iter().rev() {
                    resolved.push(part);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // A dangling symlink is not a missing plain directory.
                if fs::symlink_metadata(ancestor).is_ok() {
                    return Err(invalid("dangling state alias"));
                }
                suffix.push(
                    ancestor
                        .file_name()
                        .ok_or_else(|| invalid("unresolvable state root"))?
                        .to_os_string(),
                );
                ancestor = ancestor
                    .parent()
                    .ok_or_else(|| invalid("unresolvable state root"))?;
            }
            Err(error) => return Err(io(ancestor, error)),
        }
    }
}

fn directory(spec: &Path, state: &Path) -> Result<PathBuf, Error> {
    let spec = fs::canonicalize(spec).map_err(|error| io(spec, error))?;
    let state = resolve(state)?;
    if state.starts_with(&spec) || spec.starts_with(&state) {
        return Err(invalid(
            "review state must be physically disjoint from specification root",
        ));
    }
    let directory = state.join(STORE);
    match fs::symlink_metadata(&directory) {
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            return Err(invalid("review store must be a plain directory"));
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(io(&directory, error));
        }
        _ => (),
    }
    Ok(directory)
}

fn parse(path: &Path) -> Result<Review, Error> {
    let meta = fs::symlink_metadata(path).map_err(|error| io(path, error))?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid("review record must be a regular file"));
    }
    let bytes = fs::read(path).map_err(|error| io(path, error))?;
    let record: Review = serde_json::from_slice(&bytes)
        .map_err(|error| diag(path, error.line(), error.column(), error.to_string()))?;
    // Reject extension fields even inside shared InputFingerprint types. Prepared
    // records carry explicit nulls; missing fields are not silently defaulted.
    let raw: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    if raw != serde_json::to_value(&record).expect("review serialization") {
        return Err(invalid("unknown or omitted review fields"));
    }
    validate(&record)?;
    Ok(record)
}

fn validate(record: &Review) -> Result<(), Error> {
    if record.schema_version != RECORD_SCHEMA {
        return Err(invalid("unsupported review schema_version"));
    }
    if record.decision == Decision::Draft {
        return Err(invalid("draft is not an approval"));
    }
    let reviewer = record
        .reviewer
        .as_ref()
        .ok_or_else(|| invalid("reviewer attestation is required"))?;
    if reviewer.kind != "human_attestation"
        || !nonempty(&reviewer.identity)
        || !nonempty(&reviewer.approval_reference)
        || record
            .rationale
            .as_ref()
            .is_none_or(|value| value.trim().is_empty())
    {
        return Err(invalid(
            "human attestation, identity, approval reference and rationale are required",
        ));
    }
    if !nonempty(&record.binding.requirement.0)
        || record.binding.requirement != record.binding.selection.id
        || record.binding.selection.kind != RequirementKind::Normative
        || record.binding.selection.mapping != DeclaredMapping::Mapped
        || record.binding.selection.constraints.is_empty()
        || record.binding.mapping_sha256 != digest(&record.binding.selection)
        || !sha(&record.binding.mapping_sha256)
        || !sha(&record.binding.formalization_sha256)
        || record.supersedes.as_ref().is_some_and(|id| !sha(id))
    {
        return Err(invalid("invalid review identity or hash"));
    }
    let inputs = &record.binding.inputs;
    if inputs.is_empty()
        || !inputs.iter().any(|i| i.source == "spec:requirements.json")
        || inputs
            .windows(2)
            .any(|pair| pair[0].source >= pair[1].source)
        || inputs
            .iter()
            .any(|i| !sha(&i.sha256) || !i.source.starts_with("spec:") || !nonempty(&i.source))
    {
        return Err(invalid(
            "review inputs must be sorted unique spec identities with SHA-256 hashes",
        ));
    }
    Ok(())
}

fn load(directory: &Path) -> Result<BTreeMap<String, Review>, Error> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(io(directory, error)),
    };
    let mut paths = entries
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io(directory, error))?;
    paths.sort();
    let mut records = BTreeMap::new();
    for path in paths {
        let record = parse(&path)?;
        let id = record.id();
        if path.file_name().and_then(|name| name.to_str()) != Some(&format!("{id}.json")) {
            return Err(invalid("review filename/content hash mismatch"));
        }
        records.insert(id, record);
    }
    heads(&records)?;
    Ok(records)
}

// Explicit per-requirement linear histories: no mtime ordering, stale-head
// fallback, arbitrary branch selection or acceptance with a missing predecessor.
fn heads(records: &BTreeMap<String, Review>) -> Result<BTreeMap<RequirementId, String>, Error> {
    let mut children = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for (id, record) in records {
        if let Some(parent) = &record.supersedes {
            let previous = records
                .get(parent)
                .ok_or_else(|| invalid("missing review predecessor"))?;
            if previous.binding.requirement != record.binding.requirement {
                return Err(invalid("cross-requirement review predecessor"));
            }
            if children.insert(parent, id).is_some() {
                return Err(invalid("branched review history"));
            }
            if record.decision == Decision::NoSemanticChange
                && record.binding.formalization_sha256 != previous.binding.formalization_sha256
            {
                return Err(invalid(
                    "no_semantic_change requires unchanged formalization and target set",
                ));
            }
        } else if record.decision == Decision::NoSemanticChange
            || !roots.insert(&record.binding.requirement)
        {
            return Err(invalid(
                "review history requires one approved root per requirement",
            ));
        }
    }
    let mut result = BTreeMap::new();
    let mut reachable = BTreeSet::new();
    for (id, record) in records {
        if !children.contains_key(id) {
            let mut seen = BTreeSet::new();
            let mut cursor = Some(id);
            while let Some(key) = cursor {
                if !seen.insert(key) {
                    return Err(invalid("cyclic review history"));
                }
                reachable.insert(key);
                cursor = records[key].supersedes.as_ref();
            }
            if result
                .insert(record.binding.requirement.clone(), id.clone())
                .is_some()
            {
                return Err(invalid("multiple review heads"));
            }
        }
    }
    if reachable.len() != records.len()
        || result.len()
            != records
                .values()
                .map(|r| &r.binding.requirement)
                .collect::<BTreeSet<_>>()
                .len()
    {
        return Err(invalid("review history without a head"));
    }
    Ok(result)
}

fn active(inventory: &InventoryReport, id: &RequirementId) -> bool {
    inventory.model.requirements.get(id).is_some_and(|req| {
        inventory
            .decisions
            .iter()
            .any(|d| d.id == req.decision.0 && d.active && d.inventoried)
    })
}

fn binding(inventory: &InventoryReport, id: &RequirementId) -> Result<Binding, Error> {
    let req = inventory
        .model
        .requirements
        .get(id)
        .ok_or_else(|| invalid("unknown requirement"))?;
    if !active(inventory, id)
        || req.kind != RequirementKind::Normative
        || req.mapping != DeclaredMapping::Mapped
        || req.constraints.is_empty()
        || req
            .constraints
            .iter()
            .any(|id| !inventory.model.constraints.contains_key(id))
    {
        return Err(invalid(
            "review requires an active normative requirement with complete declared mapping",
        ));
    }
    let formalization = inventory
        .model
        .constraints
        .values()
        .map(|c| {
            (
                &c.id,
                &c.decision,
                &c.description,
                &c.formula,
                &c.applicability,
            )
        })
        .collect::<Vec<_>>();
    Ok(Binding {
        requirement: id.clone(),
        selection: req.clone(),
        mapping_sha256: digest(req),
        formalization_sha256: digest(&(
            &inventory.model.declarations,
            formalization,
            &req.constraints,
        )),
        inputs: inventory.inputs.clone(),
    })
}

pub fn prepare(spec: &Path, state: &Path, requirement: &str) -> Result<Review, Error> {
    let records = load(&directory(spec, state)?)?;
    let id = RequirementId(requirement.into());
    Ok(Review {
        schema_version: RECORD_SCHEMA.into(),
        binding: binding(&inspect(spec)?, &id)?,
        decision: Decision::Draft,
        reviewer: None,
        rationale: None,
        supersedes: heads(&records)?.get(&id).cloned(),
    })
}

pub fn import(spec: &Path, state: &Path, path: &Path) -> Result<String, Error> {
    let directory = directory(spec, state)?;
    let mut records = load(&directory)?;
    let record = parse(path)?;
    if record.binding != binding(&inspect(spec)?, &record.binding.requirement)? {
        return Err(invalid(
            "stale or altered review binding; prepare and review the current snapshot",
        ));
    }
    let id = record.id();
    if records.contains_key(&id) {
        return Ok(id);
    }
    if record.supersedes != heads(&records)?.get(&record.binding.requirement).cloned() {
        return Err(invalid(
            "review must explicitly supersede the latest record",
        ));
    }
    records.insert(id.clone(), record.clone());
    heads(&records)?;
    fs::create_dir_all(&directory).map_err(|error| io(&directory, error))?;
    let target = directory.join(format!("{id}.json"));
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&target)
    {
        Ok(mut file) => {
            file.write_all(&serde_json::to_vec_pretty(&record).expect("review serialization"))
                .map_err(|error| io(&target, error))?;
            file.sync_all().map_err(|error| io(&target, error))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            if parse(&target)? != record {
                return Err(invalid("review collision; refusing overwrite"));
            }
        }
        Err(error) => return Err(io(&target, error)),
    }
    // Concurrent different imports can leave a branch, never silently pick a head.
    load(&directory)?;
    Ok(id)
}

pub fn status(spec: &Path, state: &Path) -> Result<ReviewReport, Error> {
    let history = load(&directory(spec, state)?)?;
    let heads = heads(&history)?;
    let inventory = inspect(spec)?;
    let ids = inventory
        .model
        .requirements
        .keys()
        .chain(heads.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut required = 0;
    let mut complete = inventory.gaps.is_empty();
    let mut requirements = Vec::new();
    for id in ids {
        let latest = heads.get(&id);
        let previous = latest.map(|key| &history[key]);
        let mut assessment = Assessment {
            requirement: id.clone(),
            status: "MISSING",
            latest_review: latest.cloned(),
            changed_inputs: Vec::new(),
            diagnostics: Vec::new(),
        };
        match inventory.model.requirements.get(&id) {
            None => {
                assessment.status = "REMOVED";
                complete = false;
            }
            Some(req) if req.kind != RequirementKind::Normative || !active(&inventory, &id) => {
                assessment.status = "INACTIVE";
                assessment
                    .diagnostics
                    .push("not an active normative requirement; not scope-change approval".into());
            }
            Some(_) => {
                required += 1;
                match binding(&inventory, &id) {
                    Err(_) => {
                        assessment.status = "INELIGIBLE";
                        assessment
                            .diagnostics
                            .push("incomplete mapping or missing active target".into());
                    }
                    Ok(current) => {
                        if let Some(previous) = previous {
                            assessment.status = if previous.binding == current {
                                "CURRENT"
                            } else {
                                "STALE"
                            };
                            let old: BTreeMap<_, _> = previous
                                .binding
                                .inputs
                                .iter()
                                .map(|i| (&i.source, &i.sha256))
                                .collect();
                            let now: BTreeMap<_, _> = current
                                .inputs
                                .iter()
                                .map(|i| (&i.source, &i.sha256))
                                .collect();
                            assessment.changed_inputs = old
                                .keys()
                                .chain(now.keys())
                                .copied()
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .filter(|key| old.get(key) != now.get(key))
                                .cloned()
                                .collect();
                            if previous.binding.mapping_sha256 != current.mapping_sha256 {
                                assessment
                                    .diagnostics
                                    .push("requirement selection or mapping changed".into());
                            }
                            if previous.binding.formalization_sha256 != current.formalization_sha256
                            {
                                assessment
                                    .diagnostics
                                    .push("effective formalization or target set changed".into());
                            }
                        }
                    }
                }
                complete &= assessment.status == "CURRENT";
            }
        }
        requirements.push(assessment);
    }
    Ok(ReviewReport {
        schema_version: REPORT_SCHEMA,
        result: if complete && required > 0 {
            "REVIEWS_CURRENT"
        } else {
            "INCOMPLETE"
        },
        authority: "unsigned_human_attestation",
        verification_status: "NOT_RUN",
        inventory_gaps: inventory.gaps,
        requirements,
        history,
    })
}
