//! Read-only composition; authorization of the baseline and evidence store is external.
use crate::evidence::{self, EvidenceValidity, InputFingerprint, VerificationStatus};
use crate::project::{CoverageScope, FactCoverage, WorldAssumption};
use crate::reviews::{self, Reviewer};
use crate::roots::VerificationRoots;
use crate::{Error, native_test};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub const SET_SCHEMA: &str = "adrproof-required-set-v1alpha1";
pub const REPORT_SCHEMA: &str = "adrproof-gate-report-v1alpha1";
const GLOBAL: &str = "PO:project-consistency";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredCoverage {
    pub relation: String,
    pub scope: CoverageScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopeSnapshot {
    pub inventory_inputs: Vec<InputFingerprint>,
    pub constraints: Vec<String>,
    pub review_heads: BTreeMap<String, String>,
    pub native_definitions: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequiredSet {
    pub schema_version: String,
    pub decision: String,
    pub reviewer: Option<Reviewer>,
    pub rationale: Option<String>,
    pub scope: ScopeSnapshot,
    pub global_obligation: String,
    pub backend_version: String,
    pub timeout_ms: u64,
    pub required_coverage: Vec<RequiredCoverage>,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub obligation: String,
    pub status: VerificationStatus,
    pub evidence_id: Option<String>,
    pub freshness: Option<EvidenceValidity>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: &'static str,
    pub result: &'static str,
    pub baseline_sha256: String,
    pub authority: &'static str,
    pub scope_matches: bool,
    pub reviews: reviews::ReviewReport,
    pub coverage: Vec<FactCoverage>,
    pub checks: Vec<Check>,
    pub diagnostics: Vec<String>,
    pub does_not_prove: Vec<String>,
}
impl Report {
    pub fn exit_code(&self) -> i32 {
        match self.result {
            "PASS" => 0,
            "FAIL" => 1,
            "ERROR" => 2,
            _ => 3,
        }
    }
}

fn invalid(message: impl Into<String>) -> Error {
    crate::diag(Path::new("<gate>"), 1, 1, message)
}
fn hash(value: &impl Serialize) -> String {
    evidence::fingerprint_bytes(
        "gate",
        &serde_json::to_vec(value).expect("gate serialization"),
    )
    .sha256
}
fn is_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn nonempty(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}

fn validate_roots(roots: &VerificationRoots) -> Result<(), Error> {
    let state = reviews::resolve(&roots.state_root)?;
    for root in [&roots.project_root, &roots.specification_root] {
        let physical = fs::canonicalize(root).map_err(|source| Error::Io {
            path: root.clone(),
            source,
        })?;
        if !physical.is_dir() || state.starts_with(&physical) || physical.starts_with(&state) {
            return Err(invalid(
                "state must be physically disjoint from project and specification directories",
            ));
        }
    }
    for name in ["evidence", "native-test-evidence"] {
        let path = roots.state_root.join(name);
        if let Ok(meta) = fs::symlink_metadata(&path) {
            if !meta.is_dir() || meta.file_type().is_symlink() {
                return Err(invalid(format!("{name} must be a plain directory")));
            }
            for entry in fs::read_dir(&path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| Error::Io {
                    path: path.clone(),
                    source,
                })?;
                let kind = entry.file_type().map_err(|source| Error::Io {
                    path: entry.path(),
                    source,
                })?;
                if !kind.is_file() || entry.path().extension().is_none_or(|e| e != "json") {
                    return Err(invalid(format!(
                        "unexpected evidence entry {}",
                        entry.path().display()
                    )));
                }
            }
        } else if path
            .try_exists()
            .map_err(|source| Error::Io { path, source })?
        {
            return Err(invalid("cannot inspect evidence directory"));
        }
    }
    if roots
        .project_root
        .join("migrations")
        .try_exists()
        .map_err(|source| Error::Io {
            path: roots.project_root.join("migrations"),
            source,
        })?
    {
        validate_input_path(&roots.project_root, Path::new("migrations"))?;
    }
    Ok(())
}

fn snapshot(
    roots: &VerificationRoots,
    report: &reviews::ReviewReport,
    native_ids: &BTreeSet<String>,
) -> Result<ScopeSnapshot, Error> {
    let inventory = crate::inventory::inspect(&roots.specification_root)?;
    let mut native_definitions = BTreeMap::new();
    let definitions = native_test::discover(&roots.specification_root)?;
    let mut seen = BTreeSet::new();
    for definition in definitions {
        if !seen.insert(definition.id.clone()) {
            return Err(invalid("duplicate native-test definition"));
        }
        if native_ids.contains(&definition.id) {
            for input in &definition.inputs {
                let base = match input.root {
                    crate::scenario::InputRoot::Project => &roots.project_root,
                    crate::scenario::InputRoot::Specification => &roots.specification_root,
                };
                validate_input_path(base, &input.path)?;
            }
            native_definitions.insert(definition.id.clone(), hash(&definition));
        }
    }
    Ok(ScopeSnapshot {
        inventory_inputs: inventory.inputs,
        constraints: inventory
            .model
            .constraints
            .keys()
            .map(|id| id.0.clone())
            .collect(),
        review_heads: report
            .requirements
            .iter()
            .filter(|r| r.status == "CURRENT")
            .filter_map(|r| {
                r.latest_review
                    .as_ref()
                    .map(|id| (r.requirement.0.clone(), id.clone()))
            })
            .collect(),
        native_definitions,
    })
}

fn validate_input_path(base: &Path, relative: &Path) -> Result<(), Error> {
    if relative.is_absolute()
        || relative.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(invalid("gate inputs must remain below their declared root"));
    }
    let mut path = base.to_path_buf();
    for component in relative.components() {
        path.push(component);
        let meta = fs::symlink_metadata(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if meta.file_type().is_symlink() {
            return Err(invalid("gate input aliases are unsupported"));
        }
    }
    validate_plain_tree(&path, 0)
}

fn validate_plain_tree(path: &Path, depth: usize) -> Result<(), Error> {
    let meta = fs::symlink_metadata(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    if depth > 128 || meta.file_type().is_symlink() || (!meta.is_file() && !meta.is_dir()) {
        return Err(invalid("gate requires plain, bounded-depth input trees"));
    }
    if meta.is_dir() {
        for entry in fs::read_dir(path).map_err(|source| Error::Io {
            path: path.into(),
            source,
        })? {
            let entry = entry.map_err(|source| Error::Io {
                path: path.into(),
                source,
            })?;
            validate_plain_tree(&entry.path(), depth + 1)?;
        }
    }
    Ok(())
}

/// Produces only a draft. Does not execute processes or write state.
pub fn prepare(
    roots: &VerificationRoots,
    backend_version: &str,
    timeout_ms: u64,
    native_ids: &[String],
) -> Result<RequiredSet, Error> {
    validate_roots(roots)?;
    if !nonempty(backend_version) || timeout_ms == 0 {
        return Err(invalid(
            "explicit backend version and positive timeout required",
        ));
    }
    let ids = native_ids.iter().cloned().collect::<BTreeSet<_>>();
    if ids.len() != native_ids.len() {
        return Err(invalid("duplicate required native check"));
    }
    let reviews = reviews::status(&roots.specification_root, &roots.state_root)?;
    if reviews.exit_code() != 0 {
        return Err(invalid(
            "prepare requires current formalization reviews and a complete nonempty inventory",
        ));
    }
    let scope = snapshot(roots, &reviews, &ids)?;
    if scope.constraints.is_empty()
        || scope.review_heads.is_empty()
        || scope.native_definitions.len() != ids.len()
    {
        return Err(invalid(
            "nonempty constraints/reviews and existing required native checks are mandatory",
        ));
    }
    let (model, _) = crate::load_project_model_in_process(roots)?;
    let used = model
        .constraints
        .values()
        .flat_map(|c| crate::relations_in_formula(&c.formula))
        .collect::<BTreeSet<_>>();
    let required_coverage = model
        .fact_coverage
        .iter()
        .filter(|c| used.contains(&c.relation))
        .map(|c| RequiredCoverage {
            relation: c.relation.clone(),
            scope: c.scope.clone(),
        })
        .collect();
    Ok(RequiredSet {
        schema_version: SET_SCHEMA.into(),
        decision: "draft".into(),
        reviewer: None,
        rationale: None,
        scope,
        global_obligation: GLOBAL.into(),
        backend_version: backend_version.into(),
        timeout_ms,
        required_coverage,
    })
}

fn read_set(path: &Path, pin: &str) -> Result<RequiredSet, Error> {
    if !is_hash(pin) {
        return Err(invalid(
            "baseline SHA-256 must be 64 lowercase hexadecimal characters",
        ));
    }
    let bytes = fs::read(path).map_err(|source| Error::Io {
        path: path.into(),
        source,
    })?;
    if evidence::fingerprint_bytes("baseline", &bytes).sha256 != pin {
        return Err(invalid("BASELINE_PIN_MISMATCH"));
    }
    let set: RequiredSet = serde_json::from_slice(&bytes)
        .map_err(|e| invalid(format!("invalid required set: {e}")))?;
    // Also reject extensions/missing optional fields in shared nested model types.
    let original: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| invalid(e.to_string()))?;
    if serde_json::to_value(&set).unwrap() != original {
        return Err(invalid(
            "required set contains unsupported fields or omitted explicit fields",
        ));
    }
    let approved = set.reviewer.as_ref().is_some_and(|r| {
        r.kind == "human_attestation" && nonempty(&r.identity) && nonempty(&r.approval_reference)
    });
    if set.schema_version != SET_SCHEMA
        || set.decision != "approved"
        || !approved
        || !set.rationale.as_deref().is_some_and(nonempty)
        || set.global_obligation != GLOBAL
        || !nonempty(&set.backend_version)
        || set.timeout_ms == 0
        || set.scope.constraints.is_empty()
        || set.scope.review_heads.is_empty()
        || set.scope.inventory_inputs.is_empty()
        || set.scope.review_heads.values().any(|v| !is_hash(v))
        || set.scope.native_definitions.values().any(|v| !is_hash(v))
    {
        return Err(invalid(
            "required set must explicitly approve a nonempty supported scope and backend context",
        ));
    }
    Ok(set)
}

/// Assesses a protected snapshot using current read-only inputs and latest evidence.
pub fn evaluate(roots: &VerificationRoots, path: &Path, pin: &str) -> Result<Report, Error> {
    let set = read_set(path, pin)?;
    validate_roots(roots)?;
    let reviews = reviews::status(&roots.specification_root, &roots.state_root)?;
    let ids = set.scope.native_definitions.keys().cloned().collect();
    let scope_matches = snapshot(roots, &reviews, &ids)? == set.scope;
    let (model, inputs) = crate::load_project_model_in_process(roots)?;
    let mut diagnostics = Vec::new();
    if !scope_matches {
        diagnostics.push("REQUIRED_SCOPE_DRIFT: inventory, constraints, review heads or native definitions differ from the protected set".into());
    }
    if reviews.exit_code() != 0 {
        diagnostics.push("REVIEWS_INCOMPLETE".into());
    }
    let used = model
        .constraints
        .values()
        .flat_map(|c| crate::relations_in_formula(&c.formula))
        .collect::<BTreeSet<_>>();
    let coverage_ok = set
        .required_coverage
        .iter()
        .all(|c| model.coverage_for(&c.relation, &c.scope) == Some(WorldAssumption::Closed))
        && !model
            .fact_coverage
            .iter()
            .any(|c| used.contains(&c.relation) && c.world == WorldAssumption::Partial)
        && !model.constraints.values().any(|c| {
            crate::formula_requires_uncovered_absence(&model, &c.formula, false, &BTreeSet::new())
        });
    if !coverage_ok {
        diagnostics.push(
            "COVERAGE_INCOMPLETE: required scope is not Closed or absence is unsupported".into(),
        );
    }
    let coverage = model.fact_coverage.clone();
    let obligation = crate::relational_obligation(model);
    let inputs = crate::relevant_semantic_inputs(&obligation.model, &inputs);
    let mut fingerprints =
        evidence::fingerprint_semantic_files(&inputs).map_err(|source| Error::Io {
            path: roots.project_root.clone(),
            source,
        })?;
    fingerprints.push(evidence::fingerprint_bytes(
        "generated:effective.smt2",
        crate::obligation_to_smt(&obligation).as_bytes(),
    ));
    fingerprints.sort_by(|a, b| a.source.cmp(&b.source));
    let directory = roots.state_root.join("evidence");
    let all = evidence::load_all(&directory).map_err(|source| Error::Io {
        path: directory,
        source,
    })?;
    let latest = all.iter().rfind(|e| e.obligation.0 == GLOBAL);
    let mut global = Check {
        obligation: GLOBAL.into(),
        status: VerificationStatus::Unverified,
        evidence_id: None,
        freshness: None,
        diagnostics: vec![],
    };
    if let Some(item) = latest {
        let config = evidence::configuration_hash(set.timeout_ms, &["smt.core.minimize=true"]);
        let unique_inputs = item
            .inputs
            .iter()
            .map(|i| &i.source)
            .collect::<BTreeSet<_>>()
            .len()
            == item.inputs.len();
        let validity =
            if item.backend == "z3" && item.backend_version == set.backend_version && unique_inputs
            {
                evidence::assess(item, &fingerprints, &set.backend_version, &config)
            } else {
                EvidenceValidity::Stale
            };
        global.status = if validity == EvidenceValidity::Current {
            item.result_at_execution.clone()
        } else {
            VerificationStatus::Stale
        };
        global.evidence_id = Some(item.id.0.clone());
        global.freshness = Some(validity);
        global.diagnostics = item.diagnostics.clone();
    }
    let mut checks = vec![global];
    let mut does_not_prove = vec!["Program correctness, prose completeness, authenticated approval or independent proof replay".into(),
        "Unselected native-test, scenario, model, correspondence, functional and security checks".into()];
    let definitions = native_test::discover(&roots.specification_root)?;
    for id in set.scope.native_definitions.keys() {
        let mut check = Check {
            obligation: format!("NATIVE-TEST:{id}"),
            status: VerificationStatus::Unverified,
            evidence_id: None,
            freshness: None,
            diagnostics: vec![],
        };
        if let Some(definition) = definitions.iter().find(|d| &d.id == id) {
            does_not_prove.extend(definition.does_not_prove.clone());
            if let Some(assessment) = native_test::latest_assessment(roots, definition)? {
                let e = assessment.evidence;
                let validity = assessment.current_validity;
                check.status = if validity == EvidenceValidity::Current {
                    e.result_at_execution
                } else {
                    VerificationStatus::Stale
                };
                if check.status == VerificationStatus::Pass
                    && (e.provider != native_test::PROVIDER_NAME
                        || e.obligation.0 != check.obligation
                        || e.report_schema != native_test::REPORT_SCHEMA
                        || e.command != definition.command
                        || e.working_directory != definition.working_directory
                        || e.authority != definition.authority
                        || e.does_not_prove != definition.does_not_prove
                        || e.required_tests != definition.required_tests
                        || e.failed != 0
                        || e.passed < definition.minimum_passed
                        || e.skipped > definition.maximum_skipped
                        || e.passed == 0
                        || !e.non_vacuity.non_empty_execution
                        || !e.non_vacuity.all_required_observed_pass
                        || e.non_vacuity.executed_tests != e.passed
                        || e.non_vacuity.required_tests != definition.required_tests.len() as u64
                        || e.non_vacuity.observed_required_tests
                            != definition.required_tests.len() as u64)
                {
                    check.status = VerificationStatus::Error;
                    check.diagnostics.push("INVALID_NATIVE_PASS_CONTEXT".into());
                }
                check.evidence_id = Some(e.id.0);
                check.freshness = Some(validity);
                check.diagnostics.extend(e.diagnostics);
            }
        }
        checks.push(check);
    }
    let result = if checks.iter().any(|c| c.status == VerificationStatus::Error) {
        "ERROR"
    } else if checks.iter().any(|c| c.status == VerificationStatus::Fail) {
        "FAIL"
    } else if !scope_matches
        || reviews.exit_code() != 0
        || !coverage_ok
        || checks.iter().any(|c| c.status != VerificationStatus::Pass)
    {
        "INCOMPLETE"
    } else {
        "PASS"
    };
    Ok(Report {
        schema_version: REPORT_SCHEMA,
        result,
        baseline_sha256: pin.into(),
        authority: "externally_pinned_set_and_protected_evidence_store",
        scope_matches,
        reviews,
        coverage,
        checks,
        diagnostics,
        does_not_prove,
    })
}
