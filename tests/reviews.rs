use adrproof::project::{IntentDeclaration, RelationalFormula, RelationalProofObligation};
use adrproof::reviews::{self, Decision, Review, Reviewer};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const ADR: &str = "---\nid: ADR-1\nstatus: accepted\n---\n\nThe boundary must hold.\n\n```adrlogic\nbool boundary;\nrule C1 \"boundary\" { boundary; }\n```\n";
struct Fixture {
    root: PathBuf,
    spec: PathBuf,
    state: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "adrproof-review-{}-{}-{time}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let spec = root.join("spec");
        let state = root.join("state");
        fs::create_dir_all(&spec).unwrap();
        let f = Self { root, spec, state };
        f.adr(ADR);
        f.manifest(&manifest());
        f
    }
    fn adr(&self, text: &str) {
        fs::write(self.spec.join("architecture.md"), text).unwrap();
    }
    fn manifest(&self, value: &Value) {
        fs::write(
            self.spec.join("requirements.json"),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
    fn draft(&self) -> Review {
        reviews::prepare(&self.spec, &self.state, "REQ-1").unwrap()
    }
    fn submit(&self, record: &Review) -> Result<String, adrproof::Error> {
        let path = self.root.join("submission.json");
        fs::write(&path, serde_json::to_vec_pretty(record).unwrap()).unwrap();
        reviews::import(&self.spec, &self.state, &path)
    }
    fn approve(&self) -> String {
        self.submit(&attest(self.draft(), Decision::Approved))
            .unwrap()
    }
    fn status(&self) -> reviews::ReviewReport {
        reviews::status(&self.spec, &self.state).unwrap()
    }
    fn record_path(&self, id: &str) -> PathBuf {
        self.state
            .join("formalization-reviews")
            .join(format!("{id}.json"))
    }
    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_adrproof"))
            .arg("review")
            .args(args)
            .arg("--spec-root")
            .arg(&self.spec)
            .arg("--state-root")
            .arg(&self.state)
            .env("PATH", "")
            .env("ADRPROOF_Z3", "nonexistent-solver")
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn manifest() -> Value {
    json!({"schema_version":"adrproof-requirements-v1alpha1","adrs":[{"id":"ADR-1","source":"architecture.md","requirements":[{
        "id":"REQ-1","kind":"normative","start_line":6,"end_line":6,"mapping":"mapped","constraints":["ADR-1:C1"]
    }]}]})
}
fn attest(mut draft: Review, decision: Decision) -> Review {
    draft.decision = decision;
    draft.reviewer = Some(Reviewer {
        kind: "human_attestation".into(),
        identity: "synthetic-test-reviewer".into(),
        approval_reference: "test-only:review-1".into(),
    });
    draft.rationale = Some("Synthetic fixture only; not a real project approval.".into());
    draft
}

// Portable, independently evaluated Boolean fixture backend, not a Z3 substitute
// or qualification. It enumerates assignments instead of returning a fixed PASS.
struct BoolBackend;
impl adrproof::ConstraintBackend for BoolBackend {
    fn check(
        &self,
        obligation: &RelationalProofObligation,
        artifact: &Path,
    ) -> Result<adrproof::BackendResult, adrproof::Error> {
        fs::write(artifact, adrproof::obligation_to_smt(obligation)).map_err(|source| {
            adrproof::Error::Io {
                path: artifact.into(),
                source,
            }
        })?;
        let names = obligation
            .model
            .declarations
            .iter()
            .map(|decl| match decl {
                IntentDeclaration::Bool(name) => name.clone(),
                _ => panic!("Boolean fixtures only"),
            })
            .collect::<Vec<_>>();
        fn eval(formula: &RelationalFormula, names: &[String], mask: usize) -> bool {
            match formula {
                RelationalFormula::Bool(value) => *value,
                RelationalFormula::Name(name) => {
                    mask & (1 << names.iter().position(|n| n == name).unwrap()) != 0
                }
                RelationalFormula::Not(value) => !eval(value, names, mask),
                _ => panic!("Boolean fixture subset only"),
            }
        }
        let sat = (0..1 << names.len()).any(|mask| {
            obligation
                .model
                .constraints
                .values()
                .all(|c| eval(&c.formula, &names, mask))
        });
        Ok(adrproof::BackendResult {
            verdict: if sat {
                adrproof::Verdict::Sat
            } else {
                adrproof::Verdict::Unsat
            },
            core: vec![],
            solver_version: "fixture-truth-table-v1".into(),
            elapsed: std::time::Duration::ZERO,
            timeout_ms: 1000,
        })
    }
}
fn verify(f: &Fixture) -> adrproof::CheckReport {
    let project = f.root.join("project");
    fs::create_dir_all(&project).unwrap();
    adrproof::run_check_with_roots(
        &adrproof::roots::VerificationRoots::explicit(&project, &f.spec, &f.state),
        &BoolBackend,
    )
    .unwrap()
}

#[test]
fn prepare_is_unsigned_draft_and_never_creates_or_renews_an_approval() {
    let f = Fixture::new();
    let draft = f.draft();
    assert_eq!(draft.decision, Decision::Draft);
    assert!(draft.reviewer.is_none());
    assert!(draft.rationale.is_none());
    assert!(!f.state.exists());
    assert_eq!(f.status().requirements[0].status, "MISSING");
    assert!(!f.state.exists());
    assert!(f.submit(&draft).is_err());
    assert!(!f.state.exists());
}

#[test]
fn import_preserves_history_is_idempotent_and_current_is_not_proof_pass() {
    let f = Fixture::new();
    let record = attest(f.draft(), Decision::Approved);
    let id = f.submit(&record).unwrap();
    let bytes = fs::read(f.record_path(&id)).unwrap();
    assert_eq!(f.submit(&record).unwrap(), id);
    assert_eq!(fs::read(f.record_path(&id)).unwrap(), bytes);
    let report = f.status();
    assert_eq!(report.result, "REVIEWS_CURRENT");
    assert_eq!(report.exit_code(), 0);
    assert_eq!(report.requirements[0].status, "CURRENT");
    assert_eq!(report.verification_status, "NOT_RUN");
    assert_eq!(report.authority, "unsigned_human_attestation");
    assert_eq!(report.history.len(), 1);
}

#[test]
fn changed_prose_with_unchanged_mtime_stales_review_even_after_fresh_consistency_pass() {
    let f = Fixture::new();
    let id = f.approve();
    let bytes = fs::read(f.record_path(&id)).unwrap();
    let old = f.draft().binding;
    assert_eq!(
        verify(&f).evidence_status,
        adrproof::evidence::VerificationStatus::Pass
    );
    let path = f.spec.join("architecture.md");
    let time = fs::metadata(&path).unwrap().modified().unwrap();
    f.adr(&ADR.replace("must hold", "must always hold"));
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(time))
        .unwrap();
    assert_eq!(
        old.formalization_sha256,
        f.draft().binding.formalization_sha256
    );
    assert_eq!(
        verify(&f).evidence_status,
        adrproof::evidence::VerificationStatus::Pass
    );
    let report = f.status();
    assert_eq!(report.requirements[0].status, "STALE");
    assert_eq!(report.exit_code(), 3);
    assert_eq!(
        report.requirements[0].changed_inputs,
        ["spec:architecture.md"]
    );
    let _ = f.draft();
    assert_eq!(f.status().requirements[0].status, "STALE");
    assert_eq!(fs::read(f.record_path(&id)).unwrap(), bytes);
}

#[test]
fn no_semantic_change_reapproval_appends_without_dummy_contract_edit() {
    let f = Fixture::new();
    let first = f.approve();
    let bytes = fs::read(f.record_path(&first)).unwrap();
    f.adr(&ADR.replace("must hold", "must always hold"));
    let draft = f.draft();
    assert_eq!(draft.supersedes.as_deref(), Some(first.as_str()));
    let next = f
        .submit(&attest(draft, Decision::NoSemanticChange))
        .unwrap();
    assert_ne!(first, next);
    let report = f.status();
    assert_eq!(report.requirements[0].status, "CURRENT");
    assert_eq!(report.history.len(), 2);
    assert_eq!(
        report.requirements[0].latest_review.as_deref(),
        Some(next.as_str())
    );
    assert_eq!(fs::read(f.record_path(&first)).unwrap(), bytes);
    f.adr(ADR); // An older record matches again: must not bypass the latest head.
    let old_record: Review = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(f.submit(&old_record).unwrap(), first); // Idempotent replay cannot move the head.
    assert_eq!(f.status().requirements[0].status, "STALE");
}

#[test]
fn current_review_does_not_make_missing_or_stale_proof_evidence_current() {
    let f = Fixture::new();
    f.approve();
    assert_eq!(f.status().requirements[0].status, "CURRENT");
    assert!(!f.state.join("evidence").exists());
    verify(&f);
    let evidence = adrproof::evidence::latest(&f.state.join("evidence"))
        .unwrap()
        .unwrap();
    assert_eq!(
        adrproof::evidence::assess(
            &evidence,
            &evidence.inputs,
            "different-backend-version",
            &evidence.configuration_sha256
        ),
        adrproof::evidence::EvidenceValidity::Stale
    );
    assert_eq!(f.status().requirements[0].status, "CURRENT");
    assert_eq!(f.status().verification_status, "NOT_RUN");
}

#[test]
fn byte_only_inventory_change_stales_review_without_faking_formal_change() {
    let f = Fixture::new();
    f.approve();
    let old = f.draft().binding.formalization_sha256;
    let path = f.spec.join("requirements.json");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push('\n');
    fs::write(&path, text).unwrap();
    let report = f.status();
    assert_eq!(report.requirements[0].status, "STALE");
    assert_eq!(
        report.requirements[0].changed_inputs,
        ["spec:requirements.json"]
    );
    assert_eq!(old, f.draft().binding.formalization_sha256);
    f.submit(&attest(f.draft(), Decision::NoSemanticChange))
        .unwrap();
    assert_eq!(f.status().requirements[0].status, "CURRENT");
}

#[test]
fn changed_formalization_and_targets_require_full_review() {
    let f = Fixture::new();
    f.approve();
    f.adr(&ADR.replace("{ boundary; }", "{ !boundary; }"));
    assert_eq!(f.status().requirements[0].status, "STALE");
    assert!(
        f.submit(&attest(f.draft(), Decision::NoSemanticChange))
            .is_err()
    );
    f.approve();
    assert_eq!(f.status().requirements[0].status, "CURRENT");
    f.adr(&ADR.replace("```\n", "rule C2 \"second\" { boundary; }\n```\n"));
    f.approve();
    let mut value = manifest();
    value["adrs"][0]["requirements"][0]["constraints"] = json!(["ADR-1:C2"]);
    f.manifest(&value);
    assert!(
        f.submit(&attest(f.draft(), Decision::NoSemanticChange))
            .is_err()
    );
    f.approve();
}

#[test]
fn unknown_requirement_partial_mapping_and_inactive_decision_are_not_reviewable() {
    let f = Fixture::new();
    assert!(reviews::prepare(&f.spec, &f.state, "UNKNOWN").is_err());
    f.approve();
    let mut value = manifest();
    value["adrs"][0]["requirements"][0]["mapping"] = json!("partial");
    value["adrs"][0]["requirements"][0]["reason"] = json!("Not complete");
    f.manifest(&value);
    assert_eq!(f.status().requirements[0].status, "INELIGIBLE");
    assert!(reviews::prepare(&f.spec, &f.state, "REQ-1").is_err());
    f.manifest(&manifest());
    f.adr(&ADR.replace("rule C1 \"boundary\" { boundary; }\n", ""));
    assert_eq!(f.status().requirements[0].status, "INELIGIBLE");
    assert!(reviews::prepare(&f.spec, &f.state, "REQ-1").is_err());
    f.adr(&ADR.replace("accepted", "proposed"));
    assert_eq!(f.status().requirements[0].status, "INACTIVE");
    assert_eq!(f.status().exit_code(), 3);
    assert!(reviews::prepare(&f.spec, &f.state, "REQ-1").is_err());
}

#[test]
fn removed_reviewed_requirement_and_empty_active_set_are_never_forgotten() {
    let f = Fixture::new();
    f.approve();
    let mut value = manifest();
    value["adrs"][0]["requirements"] = json!([]);
    f.manifest(&value);
    assert_eq!(f.status().requirements[0].status, "REMOVED");
    assert_eq!(f.status().exit_code(), 3);
    f.manifest(&manifest());
    fs::remove_file(f.spec.join("architecture.md")).unwrap();
    let report = f.status();
    assert_eq!(report.exit_code(), 3);
    assert_eq!(report.inventory_gaps[0].code, "missing_adr");
}

#[test]
fn stale_or_tampered_submission_is_rejected_before_any_store_write() {
    let f = Fixture::new();
    let record = attest(f.draft(), Decision::Approved);
    f.adr(&ADR.replace("must hold", "must always hold"));
    assert!(f.submit(&record).is_err());
    assert!(!f.state.exists());
    f.adr(ADR);
    let mut bad = record.clone();
    bad.binding.inputs[0].sha256 = "0".repeat(64);
    assert!(f.submit(&bad).is_err());
    bad = record.clone();
    bad.binding.inputs.pop();
    assert!(f.submit(&bad).is_err());
    bad = record;
    bad.binding.mapping_sha256 = "a".repeat(64);
    assert!(f.submit(&bad).is_err());
    assert!(!f.state.exists());
}

#[test]
fn schema_identity_provenance_and_nested_extensions_fail_closed() {
    let f = Fixture::new();
    let record = serde_json::to_value(attest(f.draft(), Decision::Approved)).unwrap();
    let path = f.root.join("bad.json");
    for (pointer, value) in [
        ("/schema_version", json!("unknown")),
        ("/reviewer/kind", json!("llm_derived")),
        ("/reviewer/identity", json!("")),
        ("/reviewer/approval_reference", json!("")),
        ("/rationale", json!(null)),
        ("/binding/mapping_sha256", json!("bad")),
        ("/binding/inputs", json!([])),
    ] {
        let mut bad = record.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(reviews::import(&f.spec, &f.state, &path).is_err());
    }
    for pointer in [
        "",
        "/binding",
        "/binding/inputs/0",
        "/binding/selection",
        "/binding/selection/provenance",
        "/reviewer",
    ] {
        let mut bad = record.clone();
        bad.pointer_mut(pointer).unwrap()["unknown"] = json!(true);
        fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
        assert!(reviews::import(&f.spec, &f.state, &path).is_err());
    }
    let mut bad = record;
    bad.as_object_mut().unwrap().remove("supersedes");
    fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
    assert!(reviews::import(&f.spec, &f.state, &path).is_err());
    assert!(!f.state.exists());
}

#[test]
fn corrupt_store_and_missing_predecessor_are_errors_not_missing_reviews() {
    let f = Fixture::new();
    let first = f.approve();
    f.adr(&ADR.replace("must hold", "must always hold"));
    let second = f.approve();
    fs::remove_file(f.record_path(&first)).unwrap();
    assert!(reviews::status(&f.spec, &f.state).is_err());
    fs::write(f.record_path(&second), b"broken JSON").unwrap();
    assert!(reviews::status(&f.spec, &f.state).is_err());
}

#[test]
fn content_hash_mismatch_and_branch_are_rejected() {
    let f = Fixture::new();
    let first = f.approve();
    let original = fs::read(f.record_path(&first)).unwrap();
    let mut record: Review = serde_json::from_slice(&original).unwrap();
    record.rationale = Some("Changed under retained filename".into());
    fs::write(f.record_path(&first), serde_json::to_vec(&record).unwrap()).unwrap();
    assert!(reviews::status(&f.spec, &f.state).is_err());
    fs::write(f.record_path(&first), original).unwrap();
    f.adr(&ADR.replace("must hold", "must always hold"));
    let left = attest(f.draft(), Decision::Approved);
    let mut right = left.clone();
    right.rationale = Some("Another concurrent review".into());
    f.submit(&left).unwrap();
    assert!(f.submit(&right).is_err());
    fs::write(
        f.record_path(&right.id()),
        serde_json::to_vec(&right).unwrap(),
    )
    .unwrap();
    assert!(reviews::status(&f.spec, &f.state).is_err());
}

#[test]
fn no_semantic_change_requires_a_predecessor_and_formalization_cannot_be_spoofed() {
    let f = Fixture::new();
    assert!(
        f.submit(&attest(f.draft(), Decision::NoSemanticChange))
            .is_err()
    );
    f.approve();
    let old = f.draft().binding.formalization_sha256;
    f.adr(&ADR.replace("{ boundary; }", "{ !boundary; }"));
    let mut bad = attest(f.draft(), Decision::NoSemanticChange);
    bad.binding.formalization_sha256 = old;
    assert!(f.submit(&bad).is_err());
    assert_eq!(f.status().requirements[0].status, "STALE");
}

#[test]
fn whole_inventory_and_cross_adr_declarations_are_conservative_inputs() {
    let f = Fixture::new();
    fs::write(f.spec.join("other.md"),"---\nid: ADR-2\nstatus: accepted\n---\nShared intent.\n```adrlogic\nbool shared;\nrule C2 \"shared\" { shared; }\n```\n").unwrap();
    let mut value = manifest();
    value["adrs"][0]["requirements"][0]["constraints"] = json!(["ADR-2:C2"]);
    value["adrs"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"ADR-2","source":"other.md","requirements":[]}));
    f.manifest(&value);
    f.approve();
    assert_eq!(f.status().requirements[0].status, "CURRENT");
    assert_eq!(f.status().exit_code(), 3); // Other inventory gap still blocks aggregate.
    let other = fs::read_to_string(f.spec.join("other.md")).unwrap();
    fs::write(
        f.spec.join("other.md"),
        other.replace("bool shared;", "bool shared;\nbool another;"),
    )
    .unwrap();
    let report = f.status();
    assert_eq!(report.requirements[0].status, "STALE");
    assert!(
        report.requirements[0]
            .changed_inputs
            .contains(&"spec:other.md".into())
    );
    assert!(
        f.submit(&attest(f.draft(), Decision::NoSemanticChange))
            .is_err()
    );
}

#[test]
fn relocation_and_repeated_reports_preserve_logical_binding_and_history() {
    let first = Fixture::new();
    let second = Fixture::new();
    let id = first.approve();
    assert_eq!(first.draft().binding, second.draft().binding);
    fs::create_dir_all(second.state.join("formalization-reviews")).unwrap();
    fs::copy(first.record_path(&id), second.record_path(&id)).unwrap();
    assert_eq!(
        serde_json::to_vec(&first.status()).unwrap(),
        serde_json::to_vec(&second.status()).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&first.status()).unwrap(),
        serde_json::to_vec(&first.status()).unwrap()
    );
}

#[test]
fn cli_is_strict_read_only_until_import_and_does_not_run_verifiers() {
    let f = Fixture::new();
    fs::write(
        f.spec.join("adrproof.json"),
        "invalid provider config, ignored",
    )
    .unwrap();
    let out = f.cli(&["prepare", "REQ-1", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["decision"],
        "draft"
    );
    assert!(!f.state.exists());
    let out = f.cli(&["status", "--json"]);
    assert_eq!(out.status.code(), Some(3));
    assert!(!f.state.exists());
    for args in [
        vec!["status", "extra"],
        vec!["approve", "REQ-1"],
        vec!["prepare"],
        vec!["import"],
        vec!["status", "--policy", "foo"],
        vec!["status", "--state-root", "other"],
    ] {
        let out = f.cli(&args);
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
    let record = attest(f.draft(), Decision::Approved);
    let path = f.root.join("approved.json");
    fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
    let out = f.cli(&["import", "--report", path.to_str().unwrap(), "--json"]);
    assert_eq!(out.status.code(), Some(0));
    let out = f.cli(&["status"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("not architectural PASS")
    );
}

#[test]
fn state_cannot_overlap_specification() {
    let f = Fixture::new();
    for state in [&f.spec, &f.spec.join("nested"), &f.root] {
        assert!(reviews::prepare(&f.spec, state, "REQ-1").is_err());
    }
    assert!(!f.spec.join("nested").exists());
}

#[cfg(unix)]
#[test]
fn physical_aliases_and_symlinked_stores_or_records_are_checked() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let alias = f.root.join("alias");
    symlink(&f.spec, &alias).unwrap();
    assert!(reviews::prepare(&f.spec, &alias, "REQ-1").is_err());
    assert_eq!(
        f.draft().binding,
        reviews::prepare(&alias, &f.state, "REQ-1").unwrap().binding
    );
    fs::create_dir_all(&f.state).unwrap();
    symlink(&f.spec, f.state.join("formalization-reviews")).unwrap();
    assert!(reviews::status(&f.spec, &f.state).is_err());
    fs::remove_file(f.state.join("formalization-reviews")).unwrap();
    let id = f.approve();
    let record = f.record_path(&id);
    let outside = f.root.join("saved.json");
    fs::rename(&record, &outside).unwrap();
    symlink(&outside, &record).unwrap();
    assert!(reviews::status(&f.spec, &f.state).is_err());
}
