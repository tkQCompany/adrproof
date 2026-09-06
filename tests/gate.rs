use adrproof::evidence::VerificationStatus;
use adrproof::roots::VerificationRoots;
use adrproof::{evidence, gate, native_test, reviews};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const VERSION: &str = "synthetic-gate-evidence-v1";
const ADR: &str = "---\nid: ADR-1\nstatus: accepted\n---\n\nThe boundary must hold.\n\n```adrlogic\nbool boundary;\nrule C1 \"boundary\" { boundary; }\n```\n";

struct Fixture {
    root: PathBuf,
    roots: VerificationRoots,
}
impl Fixture {
    fn new() -> Self {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "adrproof-gate-{}-{}-{time}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let roots = VerificationRoots::explicit(
            &root.join("project"),
            &root.join("spec"),
            &root.join("state"),
        );
        fs::create_dir_all(&roots.project_root).unwrap();
        fs::create_dir_all(&roots.specification_root).unwrap();
        let f = Self { root, roots };
        f.adr(ADR);
        f.manifest(&manifest());
        f
    }
    fn adr(&self, value: &str) {
        fs::write(self.roots.specification_root.join("architecture.md"), value).unwrap();
    }
    fn manifest(&self, value: &Value) {
        write(
            &self.roots.specification_root.join("requirements.json"),
            value,
        );
    }
    fn approve(&self) -> String {
        let mut review = reviews::prepare(
            &self.roots.specification_root,
            &self.roots.state_root,
            "REQ-1",
        )
        .unwrap();
        review.decision = reviews::Decision::Approved;
        review.reviewer = Some(reviewer());
        review.rationale = Some("Synthetic fixture only".into());
        let path = self.root.join("review.json");
        write(&path, &review);
        reviews::import(
            &self.roots.specification_root,
            &self.roots.state_root,
            &path,
        )
        .unwrap()
    }
    fn draft(&self, native: &[String]) -> gate::RequiredSet {
        gate::prepare(&self.roots, VERSION, 1000, native).unwrap()
    }
    fn baseline(&self, native: &[String]) -> (PathBuf, String) {
        let mut set = self.draft(native);
        set.decision = "approved".into();
        set.reviewer = Some(reviewer());
        set.rationale = Some("Synthetic set approval".into());
        self.save(&set)
    }
    fn save(&self, value: &impl serde::Serialize) -> (PathBuf, String) {
        let path = self.root.join("baseline.json");
        write(&path, value);
        let pin = evidence::fingerprint_bytes("baseline", &fs::read(&path).unwrap()).sha256;
        (path, pin)
    }
    fn verify(&self, verdict: adrproof::Verdict) {
        adrproof::run_check_with_roots(&self.roots, &SyntheticBackend(verdict)).unwrap();
    }
    fn native(&self) -> native_test::NativeTestDefinition {
        let path = self
            .roots
            .specification_root
            .join("native-tests/checks/unit.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(self.roots.project_root.join("logic.txt"), "original").unwrap();
        write(
            &path,
            &json!({"id":"unit", "version":"1", "claim":"unit behavior", "authority":"test-result",
            "does_not_prove":["All behavior"], "command":"unit-command", "working_directory":".", "minimum_passed":1,
            "maximum_skipped":0,"required_tests":["boundary"],"inputs":[{"root":"project","path":"logic.txt"}],"excluded_inputs":[]}),
        );
        native_test::discover(&self.roots.specification_root)
            .unwrap()
            .remove(0)
    }
    fn import_native(&self, definition: &native_test::NativeTestDefinition, result: &str) {
        let path = self.root.join("native-report.json");
        write(
            &path,
            &json!({"schema_version":"nextest-summary-v1","runner":"synthetic","runner_version":"1","command":"unit-command",
            "working_directory":".","result":result,"passed":1,"failed":0,"skipped":0,"duration_seconds":0.0,
            "tests":[{"name":"boundary","status":"PASS"}],"diagnostics":[]}),
        );
        let e = native_test::import(&self.roots, definition, &path).unwrap();
        native_test::store(&self.roots.state_root.join("native-test-evidence"), e).unwrap();
    }
    fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_adrproof"))
            .arg("gate")
            .args(args)
            .arg("--project-root")
            .arg(&self.roots.project_root)
            .arg("--spec-root")
            .arg(&self.roots.specification_root)
            .arg("--state-root")
            .arg(&self.roots.state_root)
            .env("PATH", "")
            .env("ADRPROOF_Z3", "missing")
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn write(path: &Path, value: &impl serde::Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
fn reviewer() -> reviews::Reviewer {
    reviews::Reviewer {
        kind: "human_attestation".into(),
        identity: "synthetic-reviewer".into(),
        approval_reference: "test-only:approval".into(),
    }
}
fn manifest() -> Value {
    json!({"schema_version":"adrproof-requirements-v1alpha1","adrs":[{"id":"ADR-1","source":"architecture.md","requirements":[{
    "id":"REQ-1","kind":"normative","start_line":6,"end_line":6,"mapping":"mapped","constraints":["ADR-1:C1"]}]}]})
}

// Deliberately injected records exercise gate composition, not solver correctness.
// No PASS generated here qualifies Z3 or a real project's architecture.
struct SyntheticBackend(adrproof::Verdict);
impl adrproof::ConstraintBackend for SyntheticBackend {
    fn check(
        &self,
        obligation: &adrproof::project::RelationalProofObligation,
        artifact: &Path,
    ) -> Result<adrproof::BackendResult, adrproof::Error> {
        fs::write(artifact, adrproof::obligation_to_smt(obligation)).unwrap();
        Ok(adrproof::BackendResult {
            verdict: self.0.clone(),
            core: vec![],
            solver_version: VERSION.into(),
            elapsed: std::time::Duration::ZERO,
            timeout_ms: 1000,
        })
    }
}
fn ready() -> (Fixture, PathBuf, String) {
    let f = Fixture::new();
    f.approve();
    f.verify(adrproof::Verdict::Sat);
    let (path, pin) = f.baseline(&[]);
    (f, path, pin)
}
fn tree(path: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = vec![];
    if !path.exists() {
        return result;
    }
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            result.extend(tree(&path));
        } else {
            result.push((path.clone(), fs::read(path).unwrap()));
        }
    }
    result.sort();
    result
}

#[test]
fn current_reviews_and_proof_pass_without_processes_or_writes() {
    let (f, path, pin) = ready();
    let before = tree(&f.root);
    let out = f.cli(&[
        "evaluate",
        "--baseline",
        path.to_str().unwrap(),
        "--baseline-sha256",
        &pin,
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["result"], "PASS");
    assert_eq!(report["checks"][0]["freshness"], "CURRENT");
    assert_eq!(tree(&f.root), before);
}

#[test]
fn preparation_is_draft_and_never_approves() {
    let f = Fixture::new();
    assert!(gate::prepare(&f.roots, VERSION, 1000, &[]).is_err());
    assert!(!f.roots.state_root.exists());
    f.approve();
    let before = tree(&f.roots.state_root);
    let (path, pin) = f.save(&f.draft(&[]));
    assert!(gate::evaluate(&f.roots, &path, &pin).is_err());
    assert_eq!(tree(&f.roots.state_root), before);
}

#[test]
fn pin_rejects_rewritten_or_weakened_set() {
    let (f, path, pin) = ready();
    let mut set: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    set["scope"]["constraints"] = json!([]);
    write(&path, &set);
    assert!(
        gate::evaluate(&f.roots, &path, &pin)
            .unwrap_err()
            .to_string()
            .contains("PIN_MISMATCH")
    );
    let (_, new_pin) = f.save(&set);
    assert!(gate::evaluate(&f.roots, &path, &new_pin).is_err());
}

#[test]
fn scope_removal_lifecycle_and_mapping_narrowing_cannot_pass() {
    for mutation in ["remove", "rationale", "partial", "target", "lifecycle"] {
        let (f, path, pin) = ready();
        let mut m = manifest();
        match mutation {
            "remove" => m["adrs"][0]["requirements"] = json!([]),
            "rationale" => {
                m["adrs"][0]["requirements"][0]["kind"] = json!("rationale");
            }
            "partial" => {
                m["adrs"][0]["requirements"][0]["mapping"] = json!("partial");
                m["adrs"][0]["requirements"][0]["reason"] = json!("narrowed");
            }
            "target" => m["adrs"][0]["requirements"][0]["constraints"] = json!([]),
            _ => f.adr(&ADR.replace("accepted", "proposed")),
        }
        f.manifest(&m);
        if let Ok(r) = gate::evaluate(&f.roots, &path, &pin) {
            assert_ne!(r.result, "PASS", "{mutation}");
        }
    }
}

#[test]
fn fresh_proof_does_not_rescue_changed_prose_or_unapproved_new_baseline() {
    let (f, path, pin) = ready();
    f.adr(&ADR.replace("must hold", "must hold under a stronger requirement"));
    f.verify(adrproof::Verdict::Sat);
    let r = gate::evaluate(&f.roots, &path, &pin).unwrap();
    assert_eq!(r.result, "INCOMPLETE");
    assert_eq!(r.checks[0].status, VerificationStatus::Pass);
    f.approve();
    let r = gate::evaluate(&f.roots, &path, &pin).unwrap();
    assert_eq!(r.reviews.result, "REVIEWS_CURRENT");
    assert_eq!(r.result, "INCOMPLETE");
    assert!(!r.scope_matches);
}

#[test]
fn protected_head_prevents_tail_rollback() {
    let f = Fixture::new();
    let old = f.approve();
    let new = f.approve();
    assert_ne!(old, new);
    f.verify(adrproof::Verdict::Sat);
    let (path, pin) = f.baseline(&[]);
    fs::remove_file(
        f.roots
            .state_root
            .join("formalization-reviews")
            .join(format!("{new}.json")),
    )
    .unwrap();
    let r = gate::evaluate(&f.roots, &path, &pin).unwrap();
    assert_eq!(r.reviews.result, "REVIEWS_CURRENT");
    assert_eq!(r.result, "INCOMPLETE");
    assert!(!r.scope_matches);
}

#[test]
fn missing_stale_unknown_and_latest_fail_are_not_old_pass() {
    let f = Fixture::new();
    f.approve();
    let (path, pin) = f.baseline(&[]);
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().checks[0].status,
        VerificationStatus::Unverified
    );
    f.verify(adrproof::Verdict::Sat);
    f.verify(adrproof::Verdict::Unknown);
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().result,
        "INCOMPLETE"
    );
    f.verify(adrproof::Verdict::Unsat);
    let r = gate::evaluate(&f.roots, &path, &pin).unwrap();
    assert_eq!(r.result, "FAIL");
    assert_eq!(r.exit_code(), 1);
    let mut set: gate::RequiredSet = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    set.backend_version = "synthetic".into();
    let (path, pin) = f.save(&set);
    let r = gate::evaluate(&f.roots, &path, &pin).unwrap();
    assert_eq!(r.result, "INCOMPLETE");
    assert_eq!(r.checks[0].status, VerificationStatus::Stale);
}

#[test]
fn wrong_obligation_cannot_satisfy_global_check() {
    let (f, path, pin) = ready();
    let dir = f.roots.state_root.join("evidence");
    let mut item = evidence::latest(&dir).unwrap().unwrap();
    item.obligation.0 = "PO:other".into();
    let entry = fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
    write(&entry, &item);
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().checks[0].status,
        VerificationStatus::Unverified
    );
}

#[test]
fn selected_native_check_requires_current_nonvacuous_pass_and_unchanged_definition() {
    let f = Fixture::new();
    let definition = f.native();
    f.approve();
    f.verify(adrproof::Verdict::Sat);
    let (path, pin) = f.baseline(&["unit".into()]);
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().result,
        "INCOMPLETE"
    );
    f.import_native(&definition, "PASS");
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().result,
        "PASS"
    );
    f.import_native(&definition, "FAIL");
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().result,
        "FAIL"
    );
    fs::write(f.roots.project_root.join("logic.txt"), "changed").unwrap();
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().checks[1].status,
        VerificationStatus::Stale
    );
    let mut changed = definition;
    changed.required_tests.clear();
    write(&changed.source, &changed);
    f.import_native(&changed, "PASS");
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().result,
        "INCOMPLETE"
    );
}

#[test]
fn executable_providers_are_rejected_without_execution_or_writes() {
    for cargo in [true, false] {
        let (f, path, pin) = ready();
        if cargo {
            fs::write(f.roots.project_root.join("Cargo.toml"), "[workspace]\n").unwrap();
        } else {
            write(
                &f.roots.specification_root.join("adrproof.json"),
                &json!({"external_providers":[{
            "id":"must-not-run","version":"1","protocol":"adrproof-external-provider-v1","executable":"missing"}]}),
            );
        }
        let before = tree(&f.root);
        let out = f.cli(&[
            "evaluate",
            "--baseline",
            path.to_str().unwrap(),
            "--baseline-sha256",
            &pin,
            "--json",
        ]);
        assert_eq!(out.status.code(), Some(2));
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("UNSUPPORTED_EXECUTION"),
            "{}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert_eq!(tree(&f.root), before);
    }
}

#[test]
fn malformed_store_and_baseline_fail_closed() {
    let (f, path, _) = ready();
    let mut set: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    set["reviewer"]["unexpected"] = json!(true);
    let (_, pin) = f.save(&set);
    assert!(gate::evaluate(&f.roots, &path, &pin).is_err());
    let (path, pin) = f.baseline(&[]);
    fs::write(
        f.roots.state_root.join("evidence/partial.tmp"),
        "interrupted write",
    )
    .unwrap();
    assert!(gate::evaluate(&f.roots, &path, &pin).is_err());
}

#[test]
fn strict_cli_errors_are_json_exit_two() {
    let f = Fixture::new();
    for args in [
        vec!["evaluate", "--json"],
        vec!["prepare", "--json", "--baseline", "x"],
        vec!["evaluate", "--json", "--state-root", "x"],
        vec!["evaluate", "--json", "--json"],
    ] {
        let out = f.cli(&args);
        assert_eq!(out.status.code(), Some(2));
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["result"], "ERROR");
    }
}

#[test]
fn relocated_roots_retain_identical_gate_report() {
    let (mut f, path, pin) = ready();
    let before = serde_json::to_value(gate::evaluate(&f.roots, &path, &pin).unwrap()).unwrap();
    let relocated = f.root.with_extension("relocated");
    fs::rename(&f.root, &relocated).unwrap();
    f.root = relocated;
    f.roots = VerificationRoots::explicit(
        &f.root.join("project"),
        &f.root.join("spec"),
        &f.root.join("state"),
    );
    let after = serde_json::to_value(
        gate::evaluate(&f.roots, &f.root.join("baseline.json"), &pin).unwrap(),
    )
    .unwrap();
    assert_eq!(after, before);
}

#[test]
fn overlapping_state_is_rejected() {
    let f = Fixture::new();
    let roots = VerificationRoots::explicit(
        &f.roots.project_root,
        &f.roots.specification_root,
        &f.roots.project_root.join("state"),
    );
    assert!(gate::prepare(&roots, VERSION, 1000, &[]).is_err());
}

#[test]
fn sql_freshness_and_scoped_coverage_are_both_required() {
    let f = Fixture::new();
    f.adr(&ADR.replace("bool boundary;", "entity Table { public.users }; entity Column { id }; relation primary_key(Table, Column);")
        .replace("{ boundary; }", "{ primary_key(public.users, id); }"));
    let migration = f.roots.project_root.join("migrations/0001_users.sql");
    fs::create_dir_all(migration.parent().unwrap()).unwrap();
    fs::write(&migration, "CREATE TABLE users(id UUID PRIMARY KEY);").unwrap();
    f.approve();
    f.verify(adrproof::Verdict::Sat);
    let (path, pin) = f.baseline(&[]);
    let r = gate::evaluate(&f.roots, &path, &pin).unwrap();
    assert_eq!(r.result, "PASS");
    assert!(!r.coverage.is_empty());
    fs::write(
        &migration,
        "-- changed comment\nCREATE TABLE users(id UUID PRIMARY KEY);",
    )
    .unwrap();
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().checks[0].status,
        VerificationStatus::Stale
    );
    f.verify(adrproof::Verdict::Sat);
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().result,
        "PASS"
    );
    fs::write(&migration,"CREATE TABLE users(id UUID PRIMARY KEY); DO $$ BEGIN EXECUTE 'ALTER TABLE users ADD COLUMN hidden TEXT'; END $$;").unwrap();
    f.verify(adrproof::Verdict::Sat);
    let r = gate::evaluate(&f.roots, &path, &pin).unwrap();
    assert_eq!(r.result, "INCOMPLETE");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.starts_with("COVERAGE_INCOMPLETE"))
    );
}

#[test]
fn required_missing_scope_cannot_be_satisfied_by_another_scope() {
    let (f, path, _) = ready();
    let mut set: gate::RequiredSet = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    set.required_coverage.push(gate::RequiredCoverage {
        relation: "table".into(),
        scope: adrproof::project::CoverageScope::Table("public.missing".into()),
    });
    let (path, pin) = f.save(&set);
    assert_eq!(
        gate::evaluate(&f.roots, &path, &pin).unwrap().result,
        "INCOMPLETE"
    );
}

#[test]
fn unknown_or_duplicate_native_selection_is_rejected() {
    let f = Fixture::new();
    f.approve();
    assert!(gate::prepare(&f.roots, VERSION, 1000, &["missing".into()]).is_err());
    assert!(gate::prepare(&f.roots, VERSION, 1000, &["x".into(), "x".into()]).is_err());
}

#[cfg(unix)]
#[test]
fn aliased_state_overlap_and_native_input_cycles_are_rejected() {
    let f = Fixture::new();
    f.approve();
    let alias = f.root.join("project-alias");
    std::os::unix::fs::symlink(&f.roots.project_root, &alias).unwrap();
    let roots = VerificationRoots::explicit(
        &f.roots.project_root,
        &f.roots.specification_root,
        &alias.join("state"),
    );
    assert!(gate::prepare(&roots, VERSION, 1000, &[]).is_err());
    let definition = f.native();
    let dir = f.roots.project_root.join("loop");
    fs::create_dir(&dir).unwrap();
    std::os::unix::fs::symlink(&dir, dir.join("again")).unwrap();
    let mut changed = definition;
    changed.inputs[0].path = "loop".into();
    write(&changed.source, &changed);
    assert!(gate::prepare(&f.roots, VERSION, 1000, &["unit".into()]).is_err());
}

const CONTEXT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
impl Fixture {
    fn capture(&self) -> (PathBuf, String) {
        let snapshot = adrproof::fact_snapshot::capture(&self.roots, CONTEXT).unwrap();
        let path = self.root.join("facts.json");
        write(&path, &snapshot);
        let pin = evidence::fingerprint_bytes("snapshot", &fs::read(&path).unwrap()).sha256;
        (path, pin)
    }
    fn snapshot_baseline(&self, path: &Path, pin: &str) -> (PathBuf, String) {
        let mut set = gate::prepare_snapshot(&self.roots, VERSION, 1000, &[], path, pin).unwrap();
        assert_eq!(set.schema_version, gate::SNAPSHOT_SET_SCHEMA);
        set.decision = "approved".into();
        set.reviewer = Some(reviewer());
        set.rationale = Some("Synthetic snapshot baseline".into());
        self.save(&set)
    }
    fn cargo(&self) {
        fs::write(
            self.roots.project_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        self.package("domain");
        let output = Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(&self.roots.project_root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        self.adr(
            &ADR.replace(
                "bool boundary;",
                "entity Package { domain }; relation package(Package);",
            )
            .replace("{ boundary; }", "{ package(domain); }"),
        );
    }
    fn package(&self, name: &str) {
        let path = self.roots.project_root.join("crates").join(name);
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(
            path.join("Cargo.toml"),
            format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
        )
        .unwrap();
        fs::write(path.join("src/lib.rs"), "pub fn boundary() {}\n").unwrap();
    }
    fn provider(&self, mode: &str) {
        let exe = self
            .roots
            .specification_root
            .join(format!("fixture{}", std::env::consts::EXE_SUFFIX));
        let out = Command::new("rustc")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portable_provider.rs"))
            .args(["--edition=2024", "-o"])
            .arg(&exe)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        self.configure_provider(mode);
        fs::write(self.roots.project_root.join("input.txt"), "component=api\n").unwrap();
        self.adr(
            &ADR.replace(
                "bool boundary;",
                "entity Component { api }; relation component(Component);",
            )
            .replace("{ boundary; }", "{ component(api); }"),
        );
    }
    fn configure_provider(&self, mode: &str) {
        let mut args = vec![mode.to_string()];
        if mode == "mutate" {
            args.push(
                self.roots
                    .project_root
                    .join("unexpected.txt")
                    .to_string_lossy()
                    .into(),
            );
        }
        write(
            &self.roots.specification_root.join("adrproof.json"),
            &json!({"external_providers":[{
            "id":"portable-fixture","version":"1.0.0","protocol":"adrproof-external-provider-v1",
            "executable":format!("fixture{}",std::env::consts::EXE_SUFFIX),"args":args,"timeout_ms":1000}]}),
        );
    }
}

#[test]
fn cargo_snapshot_gate_uses_actual_metadata_without_executing_during_admission() {
    let f = Fixture::new();
    f.cargo();
    f.approve();
    f.verify(adrproof::Verdict::Sat);
    let (snapshot, sp) = f.capture();
    let (baseline, bp) = f.snapshot_baseline(&snapshot, &sp);
    let raw: Value = serde_json::from_slice(&fs::read(&snapshot).unwrap()).unwrap();
    assert!(
        raw["model"]["facts"]
            .as_object()
            .unwrap()
            .values()
            .any(|f| f["relation"] == "package")
    );
    let before = tree(&f.root);
    let out = f.cli(&[
        "evaluate-snapshot",
        "--snapshot",
        snapshot.to_str().unwrap(),
        "--snapshot-sha256",
        &sp,
        "--baseline",
        baseline.to_str().unwrap(),
        "--baseline-sha256",
        &bp,
        "--json",
    ]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["schema_version"], gate::SNAPSHOT_REPORT_SCHEMA);
    assert_eq!(r["fact_snapshot"]["snapshot_sha256"], sp);
    assert_eq!(tree(&f.root), before);
    assert!(gate::evaluate(&f.roots, &baseline, &bp).is_err());
    f.package("new_member");
    assert!(
        gate::evaluate_snapshot(&f.roots, &baseline, &bp, &snapshot, &sp)
            .unwrap_err()
            .to_string()
            .contains("SNAPSHOT_STALE")
    );
}

#[test]
fn snapshot_detects_added_deleted_modified_and_permission_inputs() {
    for mutation in [
        "add",
        "delete",
        "modify",
        "empty-directory",
        "config",
        "lock",
    ] {
        let f = Fixture::new();
        f.cargo();
        let (path, pin) = f.capture();
        match mutation {
            "add" => {
                fs::write(f.roots.project_root.join("new.txt"), "new").unwrap();
            }
            "delete" => {
                fs::remove_file(f.roots.project_root.join("crates/domain/src/lib.rs")).unwrap()
            }
            "modify" => fs::write(
                f.roots.project_root.join("crates/domain/Cargo.toml"),
                "changed",
            )
            .unwrap(),
            "empty-directory" => fs::create_dir(f.roots.project_root.join("empty")).unwrap(),
            "config" => {
                fs::create_dir(f.roots.project_root.join(".cargo")).unwrap();
                fs::write(
                    f.roots.project_root.join(".cargo/config.toml"),
                    "[net]\noffline=true\n",
                )
                .unwrap();
            }
            _ => fs::write(f.roots.project_root.join("Cargo.lock"), "changed").unwrap(),
        }
        assert!(
            adrproof::fact_snapshot::validate(&f.roots, &path, &pin).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn external_snapshot_rejects_stale_executable_configuration_and_preserves_partial_coverage() {
    let f = Fixture::new();
    f.provider("closed");
    f.approve();
    f.verify(adrproof::Verdict::Sat);
    let (path, pin) = f.capture();
    let (baseline, bp) = f.snapshot_baseline(&path, &pin);
    assert_eq!(
        gate::evaluate_snapshot(&f.roots, &baseline, &bp, &path, &pin)
            .unwrap()
            .result,
        "PASS"
    );
    let executable = f
        .roots
        .specification_root
        .join(format!("fixture{}", std::env::consts::EXE_SUFFIX));
    let original = fs::read(&executable).unwrap();
    fs::write(&executable, b"changed").unwrap();
    assert!(adrproof::fact_snapshot::validate(&f.roots, &path, &pin).is_err());
    fs::write(executable, original).unwrap();
    f.configure_provider("valid");
    assert!(adrproof::fact_snapshot::validate(&f.roots, &path, &pin).is_err());
    f.verify(adrproof::Verdict::Sat);
    let (path, pin) = f.capture();
    assert!(
        gate::evaluate_snapshot(&f.roots, &baseline, &bp, &path, &pin)
            .unwrap_err()
            .to_string()
            .contains("PROVIDER_POLICY_DRIFT")
    );
    let (baseline, bp) = f.snapshot_baseline(&path, &pin);
    let r = gate::evaluate_snapshot(&f.roots, &baseline, &bp, &path, &pin).unwrap();
    assert_eq!(r.result, "INCOMPLETE");
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.starts_with("COVERAGE_INCOMPLETE"))
    );
}

#[test]
fn failed_or_mutating_provider_never_emits_snapshot() {
    let f = Fixture::new();
    f.provider("malformed");
    assert!(adrproof::fact_snapshot::capture(&f.roots, CONTEXT).is_err());
    f.configure_provider("mutate");
    assert!(
        adrproof::fact_snapshot::capture(&f.roots, CONTEXT)
            .unwrap_err()
            .to_string()
            .contains("CAPTURE_INPUT_DRIFT")
    );
    assert!(f.roots.project_root.join("unexpected.txt").exists());
}

#[test]
fn snapshot_tampering_profile_mismatch_and_legacy_downgrade_fail_closed() {
    let f = Fixture::new();
    f.approve();
    f.verify(adrproof::Verdict::Sat);
    let (path, pin) = f.capture();
    let (baseline, bp) = f.snapshot_baseline(&path, &pin);
    let mut raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    raw["producer_context_sha256"] =
        json!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");
    write(&path, &raw);
    assert!(
        adrproof::fact_snapshot::validate(&f.roots, &path, &pin)
            .unwrap_err()
            .to_string()
            .contains("PIN_MISMATCH")
    );
    let newpin = evidence::fingerprint_bytes("snapshot", &fs::read(&path).unwrap()).sha256;
    assert!(
        gate::evaluate_snapshot(&f.roots, &baseline, &bp, &path, &newpin)
            .unwrap_err()
            .to_string()
            .contains("CONTEXT_MISMATCH")
    );
    let (legacy, lp) = f.baseline(&[]);
    assert!(gate::evaluate_snapshot(&f.roots, &legacy, &lp, &path, &newpin).is_err());
    raw["semantic_inputs"] = json!([]);
    write(&path, &raw);
    let newpin = evidence::fingerprint_bytes("snapshot", &fs::read(&path).unwrap()).sha256;
    assert!(adrproof::fact_snapshot::validate(&f.roots, &path, &newpin).is_err());
}

#[test]
fn snapshot_facts_cannot_substitute_for_missing_or_latest_failing_proof() {
    let f = Fixture::new();
    f.cargo();
    f.approve();
    let (path, pin) = f.capture();
    let (baseline, bp) = f.snapshot_baseline(&path, &pin);
    assert_eq!(
        gate::evaluate_snapshot(&f.roots, &baseline, &bp, &path, &pin)
            .unwrap()
            .result,
        "INCOMPLETE"
    );
    f.verify(adrproof::Verdict::Sat);
    assert_eq!(
        gate::evaluate_snapshot(&f.roots, &baseline, &bp, &path, &pin)
            .unwrap()
            .result,
        "PASS"
    );
    f.verify(adrproof::Verdict::Unsat);
    assert_eq!(
        gate::evaluate_snapshot(&f.roots, &baseline, &bp, &path, &pin)
            .unwrap()
            .result,
        "FAIL"
    );
}

#[test]
fn source_export_boundary_rejects_excluded_trees_and_escaping_semantic_inputs() {
    let f = Fixture::new();
    fs::create_dir(f.roots.project_root.join("target")).unwrap();
    assert!(adrproof::fact_snapshot::capture(&f.roots, CONTEXT).is_err());
    let f = Fixture::new();
    let (path, _) = f.capture();
    let mut raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    raw["semantic_inputs"][0]["source"] = json!("project:../outside.txt");
    write(&path, &raw);
    let pin = evidence::fingerprint_bytes("snapshot", &fs::read(&path).unwrap()).sha256;
    assert!(adrproof::fact_snapshot::validate(&f.roots, &path, &pin).is_err());
}

#[test]
fn snapshot_relocation_and_state_changes_do_not_stale_facts() {
    let mut f = Fixture::new();
    f.cargo();
    let (path, pin) = f.capture();
    fs::create_dir_all(&f.roots.state_root).unwrap();
    fs::write(f.roots.state_root.join("runner-log.txt"), "not semantic").unwrap();
    assert!(adrproof::fact_snapshot::validate(&f.roots, &path, &pin).is_ok());
    let relocated = f.root.with_extension("snapshot-relocated");
    fs::rename(&f.root, &relocated).unwrap();
    f.root = relocated;
    f.roots = VerificationRoots::explicit(
        &f.root.join("project"),
        &f.root.join("spec"),
        &f.root.join("state"),
    );
    assert!(adrproof::fact_snapshot::validate(&f.roots, &f.root.join("facts.json"), &pin).is_ok());
    let fresh = adrproof::fact_snapshot::capture(&f.roots, CONTEXT).unwrap();
    assert_eq!(
        serde_json::to_value(fresh).unwrap(),
        serde_json::from_slice::<Value>(&fs::read(f.root.join("facts.json")).unwrap()).unwrap()
    );
}

#[cfg(unix)]
#[test]
fn snapshot_detects_permission_changes_and_rejects_child_aliases() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let (path, pin) = f.capture();
    let file = f.roots.specification_root.join("architecture.md");
    let mut mode = fs::metadata(&file).unwrap().permissions();
    mode.set_mode(mode.mode() ^ 0o100);
    fs::set_permissions(file, mode).unwrap();
    assert!(adrproof::fact_snapshot::validate(&f.roots, &path, &pin).is_err());
    std::os::unix::fs::symlink(
        &f.roots.specification_root,
        f.roots.project_root.join("alias"),
    )
    .unwrap();
    assert!(adrproof::fact_snapshot::capture(&f.roots, CONTEXT).is_err());
}

#[test]
fn snapshot_cannot_remove_obligations_even_with_a_new_transport_pin() {
    let f = Fixture::new();
    let (path, _) = f.capture();
    let mut raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    raw["model"]["constraints"] = json!({});
    write(&path, &raw);
    let pin = evidence::fingerprint_bytes("snapshot", &fs::read(&path).unwrap()).sha256;
    assert!(
        adrproof::fact_snapshot::validate(&f.roots, &path, &pin)
            .unwrap_err()
            .to_string()
            .contains("SPEC_MISMATCH")
    );
}

#[test]
fn snapshot_commands_require_complete_explicit_options() {
    let f = Fixture::new();
    for args in [
        vec![
            "prepare-snapshot",
            "--backend-version",
            VERSION,
            "--timeout-ms",
            "1000",
            "--json",
        ],
        vec!["evaluate-snapshot", "--json"],
        vec!["capture", "--producer-context-sha256", CONTEXT, "--json"],
    ] {
        let out = f.cli(&args);
        assert_eq!(out.status.code(), Some(2));
    }
    let out = Command::new(env!("CARGO_BIN_EXE_adrproof"))
        .args(["snapshot", "capture", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let output: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(output["schema_version"], adrproof::fact_snapshot::SCHEMA);
}
