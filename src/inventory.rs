//! Read-only declared requirement inventory. No approval or verification occurs.

use crate::evidence::{InputFingerprint, fingerprint_bytes};
use crate::project::{
    ArtifactId, ConstraintId, DecisionId, DeclaredMapping, GraphEdge, GraphNode, IntentRequirement,
    LinkKind, ProjectModel, Provenance, ProvenanceKind, RequirementId, RequirementKind,
};
use crate::{Error, SourceSpan, Status, diag};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const INPUT_SCHEMA: &str = "adrproof-requirements-v1alpha1";
pub const REPORT_SCHEMA: &str = "adrproof-inventory-report-v1alpha1";
const INVENTORY_FILE: &str = "requirements.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inventory {
    schema_version: String,
    adrs: Vec<DecisionEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionEntry {
    id: String,
    source: String,
    requirements: Vec<RequirementEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RequirementEntry {
    id: String,
    kind: RequirementKind,
    start_line: usize,
    end_line: usize,
    mapping: DeclaredMapping,
    constraints: Vec<ConstraintId>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DecisionInventory {
    pub id: String,
    pub source: String,
    pub inventoried: bool,
    pub present: bool,
    pub status: Option<Status>,
    pub active: bool,
    pub inactive_reason: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gap {
    pub code: String,
    pub decision: String,
    pub requirement: Option<String>,
    pub target: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InventoryReport {
    pub schema_version: &'static str,
    pub result: &'static str,
    pub review_status: &'static str,
    pub verification_status: &'static str,
    pub decisions: Vec<DecisionInventory>,
    pub gaps: Vec<Gap>,
    pub inputs: Vec<InputFingerprint>,
    pub model: ProjectModel,
}

impl InventoryReport {
    pub fn exit_code(&self) -> i32 {
        if self.gaps.is_empty() { 0 } else { 3 }
    }

    fn gap(&mut self, code: &str, decision: &str, requirement: Option<&str>, target: Option<&str>) {
        self.gaps.push(Gap {
            code: code.into(),
            decision: decision.into(),
            requirement: requirement.map(String::from),
            target: target.map(String::from),
        });
    }
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let metadata = fs::symlink_metadata(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(diag(
            path,
            1,
            1,
            "inventory inputs must be regular files, not symlinks or devices",
        ));
    }
    fs::read(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.trim() == id && !id.chars().any(char::is_control)
}

fn portable_source(source: &str) -> bool {
    source.ends_with(".md")
        && source.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !matches!(
                    part.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
                && !part.ends_with(['.', ' '])
                && !part
                    .chars()
                    .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
        })
}

fn validate(inventory: &Inventory, path: &Path) -> Result<(), Error> {
    let invalid = |message: &str| diag(path, 1, 1, message);
    if inventory.schema_version != INPUT_SCHEMA {
        return Err(invalid("unsupported requirement inventory schema_version"));
    }
    if inventory.adrs.is_empty() {
        return Err(invalid(
            "requirement inventory must contain at least one ADR",
        ));
    }
    let mut decisions = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut requirements = BTreeSet::new();
    for entry in &inventory.adrs {
        if !valid_id(&entry.id) || !decisions.insert(&entry.id) {
            return Err(invalid("invalid or duplicate inventory ADR ID"));
        }
        if !portable_source(&entry.source) || !sources.insert(&entry.source) {
            return Err(invalid("invalid or duplicate inventory source path"));
        }
        for req in &entry.requirements {
            if !valid_id(&req.id) || !requirements.insert(&req.id) {
                return Err(invalid("invalid or duplicate requirement ID"));
            }
            if req.start_line == 0 || req.end_line < req.start_line {
                return Err(invalid(
                    "requirement line range must be nonempty and one-based",
                ));
            }
            let targets: BTreeSet<_> = req.constraints.iter().collect();
            if targets.len() != req.constraints.len() || targets.iter().any(|id| !valid_id(&id.0)) {
                return Err(invalid("invalid or duplicate constraint target"));
            }
            if (req.mapping == DeclaredMapping::Unmapped) != req.constraints.is_empty() {
                return Err(invalid(
                    "unmapped requires no targets; mapped/partial require targets",
                ));
            }
            if req.kind == RequirementKind::Rationale && req.mapping != DeclaredMapping::Unmapped {
                return Err(invalid("rationale cannot declare formalization targets"));
            }
            if (req.kind == RequirementKind::Rationale || req.mapping == DeclaredMapping::Partial)
                && req
                    .reason
                    .as_ref()
                    .is_none_or(|reason| reason.trim().is_empty())
            {
                return Err(invalid("rationale and partial mapping require a reason"));
            }
        }
    }
    Ok(())
}

// Dedicated spec-tree discovery: do not follow file/directory symlinks or execute
// providers. Root aliases are allowed; logical identities remain root-relative.
fn collect(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    let entries = fs::read_dir(root).map_err(|source| Error::Io {
        path: root.into(),
        source,
    })?;
    let mut entries = entries
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: root.into(),
            source,
        })?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if matches!(
            entry.file_name().to_str(),
            Some(".git" | "target" | ".adrproof")
        ) {
            continue;
        }
        let ty = entry.file_type().map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if ty.is_symlink() {
            return Err(diag(
                &path,
                1,
                1,
                "symlinks are not accepted within an inventory spec root",
            ));
        }
        if ty.is_dir() {
            collect(&path, files)?;
        } else if ty.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            files.push(path);
        }
    }
    Ok(())
}

/// Inspect one stable specification checkout without invoking any proof backend.
pub fn inspect(root: &Path) -> Result<InventoryReport, Error> {
    let manifest = root.join(INVENTORY_FILE);
    let mut files = Vec::new();
    collect(root, &mut files)?;
    let bytes = read(&manifest)?;
    let inventory: Inventory = serde_json::from_slice(&bytes)
        .map_err(|error| diag(&manifest, error.line(), error.column(), error.to_string()))?;
    validate(&inventory, &manifest)?;
    let mut inputs = vec![fingerprint_bytes("spec:requirements.json", &bytes)];
    let mut texts = BTreeMap::new();
    let mut adrs = Vec::new();
    let mut ids = BTreeSet::new();
    let mut clause_ids = BTreeSet::new();
    for path in files {
        let bytes = read(&path)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|error| diag(&path, 1, 1, error.to_string()))?;
        let adr = crate::parse_adr_text(&path, text)?;
        if !valid_id(&adr.id) || !ids.insert(adr.id.clone()) {
            return Err(diag(&path, 1, 1, "invalid or duplicate discovered ADR ID"));
        }
        for clause in &adr.clauses {
            if !clause_ids.insert(format!("{}:{}", adr.id, clause.id)) {
                return Err(diag(&path, clause.span.line, 1, "duplicate constraint ID"));
            }
        }
        let relative = path
            .strip_prefix(root)
            .expect("discovered child")
            .to_str()
            .ok_or_else(|| diag(&path, 1, 1, "inventory source must be UTF-8"))?
            .replace('\\', "/");
        if !portable_source(&relative) {
            return Err(diag(
                &path,
                1,
                1,
                "inventory source must be a portable relative path",
            ));
        }
        inputs.push(fingerprint_bytes(&format!("spec:{relative}"), &bytes));
        texts.insert(relative, text.to_string());
        adrs.push(adr);
    }
    let spec = crate::effective(&adrs, false)?;
    let mut model = crate::lower_to_project_model(&adrs, &spec);
    crate::namespace_model_artifacts(&mut model, "spec", root);
    inputs.sort_by(|a, b| a.source.cmp(&b.source));
    let mut report = InventoryReport {
        schema_version: REPORT_SCHEMA,
        result: "RECORDED_UNREVIEWED",
        review_status: "NOT_ASSESSED",
        verification_status: "NOT_RUN",
        decisions: Vec::new(),
        gaps: Vec::new(),
        inputs,
        model,
    };
    let declared: BTreeMap<_, _> = inventory
        .adrs
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    for adr in &adrs {
        let decision = &report.model.decisions[&DecisionId(adr.id.clone())];
        let source = decision.provenance.source.to_string_lossy().into_owned();
        let active = spec.active_adrs.contains(&adr.id);
        let inventoried = declared
            .get(adr.id.as_str())
            .is_some_and(|entry| source == format!("spec:{}", entry.source));
        report.decisions.push(DecisionInventory {
            id: adr.id.clone(),
            source,
            inventoried,
            present: true,
            status: Some(adr.status.clone()),
            active,
            inactive_reason: if active {
                None
            } else {
                Some(
                    match adr.status {
                        Status::Accepted => "superseded_by_accepted_adr",
                        Status::Proposed => "proposed",
                        Status::Deprecated => "deprecated",
                        Status::Superseded => "superseded",
                    }
                    .into(),
                )
            },
        });
        if !inventoried {
            report.gap("uninventoried_adr", &adr.id, None, None);
        }
    }
    for entry in &inventory.adrs {
        let found = report.model.decisions.get(&DecisionId(entry.id.clone()));
        let matches = found
            .is_some_and(|d| d.provenance.source == Path::new(&format!("spec:{}", entry.source)));
        if !matches {
            let present = texts.contains_key(&entry.source);
            report.gap(
                if present || found.is_some() {
                    "adr_source_mismatch"
                } else {
                    "missing_adr"
                },
                &entry.id,
                None,
                Some(&entry.source),
            );
            report.decisions.push(DecisionInventory {
                id: entry.id.clone(),
                source: format!("spec:{}", entry.source),
                inventoried: true,
                present,
                status: None,
                active: false,
                inactive_reason: Some("unresolved_inventory_entry".into()),
            });
        }
        let active = matches && spec.active_adrs.contains(&entry.id);
        if active
            && !entry
                .requirements
                .iter()
                .any(|req| req.kind == RequirementKind::Normative)
        {
            report.gap("no_normative_requirements", &entry.id, None, None);
        }
        for req in &entry.requirements {
            if let Some(text) = texts.get(&entry.source) {
                let lines = text.lines().collect::<Vec<_>>();
                if req.end_line > lines.len()
                    || lines[req.start_line - 1..req.end_line]
                        .iter()
                        .all(|line| line.trim().is_empty())
                {
                    return Err(diag(
                        &manifest,
                        1,
                        1,
                        format!("invalid or empty text selection for {}", req.id),
                    ));
                }
            }
            let id = RequirementId(req.id.clone());
            let source = PathBuf::from(format!("spec:{}", entry.source));
            let mut constraints = req.constraints.clone();
            constraints.sort();
            report.model.requirements.insert(
                id.clone(),
                IntentRequirement {
                    id: id.clone(),
                    decision: DecisionId(entry.id.clone()),
                    kind: req.kind.clone(),
                    provenance: Provenance {
                        kind: ProvenanceKind::HumanAuthored,
                        source: source.clone(),
                        span: Some(SourceSpan {
                            filename: source.clone(),
                            line: req.start_line,
                            column: 1,
                        }),
                        extractor: None,
                    },
                    end_line: req.end_line,
                    mapping: req.mapping.clone(),
                    constraints: constraints.clone(),
                    reason: req.reason.clone(),
                },
            );
            // Do not fabricate decision/artifact nodes for missing sources.
            if matches {
                report.model.edges.push(GraphEdge {
                    from: GraphNode::Decision(DecisionId(entry.id.clone())),
                    kind: LinkKind::Contains,
                    to: GraphNode::Requirement(id.clone()),
                });
                report.model.edges.push(GraphEdge {
                    from: GraphNode::Artifact(ArtifactId(source.to_string_lossy().into_owned())),
                    kind: LinkKind::Defines,
                    to: GraphNode::Requirement(id.clone()),
                });
            }
            for target in &constraints {
                if report.model.constraints.contains_key(target) {
                    report.model.edges.push(GraphEdge {
                        from: GraphNode::Requirement(id.clone()),
                        kind: LinkKind::DeclaredFormalization,
                        to: GraphNode::Constraint(target.clone()),
                    });
                } else if active {
                    report.gap(
                        "missing_active_constraint",
                        &entry.id,
                        Some(&req.id),
                        Some(&target.0),
                    );
                }
            }
            if active && req.kind == RequirementKind::Normative {
                match req.mapping {
                    DeclaredMapping::Unmapped => {
                        report.gap("unmapped_requirement", &entry.id, Some(&req.id), None)
                    }
                    DeclaredMapping::Partial => {
                        report.gap("partial_mapping", &entry.id, Some(&req.id), None)
                    }
                    DeclaredMapping::Mapped => (),
                }
            }
        }
    }
    report
        .decisions
        .sort_by(|a, b| (&a.id, &a.source, a.inventoried).cmp(&(&b.id, &b.source, b.inventoried)));
    report.gaps.sort();
    report.model.normalize();
    if !report.gaps.is_empty() {
        report.result = "INCOMPLETE";
    }
    Ok(report)
}
