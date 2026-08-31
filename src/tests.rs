use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static N: AtomicUsize = AtomicUsize::new(0);
fn dir() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!(
        "adrproof-test-{}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed),
        nonce
    ));
    fs::create_dir_all(&p).unwrap();
    p
}
fn adr(root: &Path, file: &str, id: &str, status: &str, relations: &str, body: &str) {
    fs::write(
        root.join(file),
        format!("---\nid: {id}\nstatus: {status}\n{relations}---\n\n```adrlogic\n{body}\n```\n"),
    )
    .unwrap()
}
fn spec(root: &Path) -> Result<EffectiveSpecification, Error> {
    effective(&load_adrs(root)?, false)
}
fn satisfiable(s: &EffectiveSpecification) -> bool {
    let names = s
        .declarations
        .iter()
        .filter_map(|d| {
            if let Decl::Bool(n) = d {
                Some(n.clone())
            } else {
                None
            }
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    (0..(1usize << names.len())).any(|mask| {
        let env = names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.clone(), mask & (1 << i) != 0))
            .collect();
        s.clauses.iter().all(|c| eval(&c.expression, &env))
    })
}
fn eval(e: &Expr, env: &BTreeMap<String, bool>) -> bool {
    match e {
        Expr::Bool(x) => *x,
        Expr::Name(n) => env[n],
        Expr::Not(x) => !eval(x, env),
        Expr::And(a, b) => eval(a, env) && eval(b, env),
        Expr::Or(a, b) => eval(a, env) || eval(b, env),
        Expr::Implies(a, b) => !eval(a, env) || eval(b, env),
        _ => panic!("test evaluator only supports Bool formulas"),
    }
}

fn workspace(direct_domain_db: bool) -> PathBuf {
    let root = dir();
    fs::create_dir_all(root.join("domain/src")).unwrap();
    fs::create_dir_all(root.join("repository/src")).unwrap();
    fs::create_dir_all(root.join("fake_sqlx/src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver='2'\nmembers=['domain','repository','fake_sqlx']\n",
    )
    .unwrap();
    let domain_dependency = if direct_domain_db {
        "fake_sqlx={path='../fake_sqlx'}"
    } else {
        "repository={path='../repository'}"
    };
    fs::write(
        root.join("domain/Cargo.toml"),
        format!("[package]\nname='domain'\nversion='0.1.0'\nedition='2024'\n[dependencies]\n{domain_dependency}\n"),
    )
    .unwrap();
    fs::write(root.join("domain/src/lib.rs"), "").unwrap();
    fs::write(root.join("repository/Cargo.toml"), "[package]\nname='repository'\nversion='0.1.0'\nedition='2024'\n[dependencies]\nfake_sqlx={path='../fake_sqlx'}\n").unwrap();
    fs::write(root.join("repository/src/lib.rs"), "").unwrap();
    fs::write(
        root.join("fake_sqlx/Cargo.toml"),
        "[package]\nname='fake_sqlx'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    fs::write(root.join("fake_sqlx/src/lib.rs"), "").unwrap();
    adr(
        &root,
        "architecture.md",
        "ADR-1",
        "accepted",
        "",
        "entity Package { domain, repository, fake_sqlx }; relation declares_direct_dependency(Package, Package); rule C1 \"no direct db\" { !declares_direct_dependency(domain, fake_sqlx); }",
    );
    root
}

fn ground_constraint_holds(model: &ProjectModel) -> bool {
    let facts = model
        .facts
        .values()
        .filter(|fact| fact.value && fact_is_solver_supported(fact))
        .map(|fact| (fact.relation.clone(), fact.arguments.clone()))
        .collect::<BTreeSet<_>>();
    fn formula(value: &RelationalFormula, facts: &BTreeSet<(String, Vec<String>)>) -> bool {
        match value {
            RelationalFormula::Bool(value) => *value,
            RelationalFormula::Relation(name, arguments) => {
                facts.contains(&(name.clone(), arguments.clone()))
            }
            RelationalFormula::Not(value) => !formula(value, facts),
            RelationalFormula::And(a, b) => formula(a, facts) && formula(b, facts),
            RelationalFormula::Or(a, b) => formula(a, facts) || formula(b, facts),
            RelationalFormula::Implies(a, b) => !formula(a, facts) || formula(b, facts),
            other => panic!("ground test evaluator cannot evaluate {other:?}"),
        }
    }
    model
        .constraints
        .values()
        .all(|constraint| formula(&constraint.formula, &facts))
}

#[test]
fn two_compatible_adrs_are_sat() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"x\" { x; }",
    );
    adr(
        &d,
        "b.md",
        "B",
        "accepted",
        "",
        "bool x; rule C1 \"also x\" { x; }",
    );
    assert!(satisfiable(&spec(&d).unwrap()))
}
#[test]
fn direct_conflict_is_unsat() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"x\" { x; }",
    );
    adr(
        &d,
        "b.md",
        "B",
        "accepted",
        "",
        "bool x; rule C1 \"not x\" { !x; }",
    );
    assert!(!satisfiable(&spec(&d).unwrap()))
}
#[test]
fn global_three_way_conflict_not_pairwise() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"x\" { x; }",
    );
    adr(
        &d,
        "b.md",
        "B",
        "accepted",
        "",
        "bool y; rule C1 \"y\" { y; }",
    );
    adr(
        &d,
        "c.md",
        "C",
        "accepted",
        "",
        "bool x; bool y; rule C1 \"not both\" { !(x && y); }",
    );
    let all = load_adrs(&d).unwrap();
    assert!(!satisfiable(&effective(&all, false).unwrap()));
    for omit in 0..3 {
        let pair = all
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != omit)
            .map(|(_, x)| x.clone())
            .collect::<Vec<_>>();
        assert!(
            satisfiable(&effective(&pair, false).unwrap()),
            "pair {omit} must be SAT"
        )
    }
}
#[test]
fn supersedes_removes_old_constraint() {
    let d = dir();
    adr(
        &d,
        "old.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"x\" { x; }",
    );
    adr(
        &d,
        "new.md",
        "B",
        "accepted",
        "supersedes: A\n",
        "bool x; rule C1 \"not x\" { !x; }",
    );
    let s = spec(&d).unwrap();
    assert_eq!(s.active_adrs, vec!["B"]);
    assert!(satisfiable(&s))
}
#[test]
fn bad_reference_is_rejected() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "supersedes: MISSING\n",
        "bool x; rule C1 \"x\" { x; }",
    );
    assert!(matches!(spec(&d), Err(Error::InvalidReference { .. })))
}
#[test]
fn type_error_precedes_solver() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "entity Crate { domain }; relation depends_on(Crate, Crate); rule C1 \"bad type\" { depends_on(domain, missing); }",
    );
    assert!(matches!(spec(&d), Err(Error::Diagnostic { .. })))
}
#[test]
fn smt_has_named_source_clauses_in_stable_order() {
    let d = dir();
    adr(
        &d,
        "z.md",
        "Z",
        "accepted",
        "",
        "bool x; rule C2 \"second\" { x; }",
    );
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"first\" { !x; }",
    );
    let s = spec(&d).unwrap();
    let a = to_smt(&s, None);
    let b = to_smt(&s, None);
    assert_eq!(a, b);
    assert!(a.find("|A:C1|").unwrap() < a.find("|Z:C2|").unwrap())
}

struct FixedBackend(Verdict);
impl ConstraintBackend for FixedBackend {
    fn check(&self, s: &RelationalProofObligation, p: &Path) -> Result<BackendResult, Error> {
        fs::write(p, obligation_to_smt(s)).unwrap();
        Ok(BackendResult {
            verdict: self.0.clone(),
            core: vec!["A:C1".into(), "B:C1".into()],
            solver_version: "Z3 4.13.4".into(),
            elapsed: Duration::from_millis(1),
            timeout_ms: 10_000,
        })
    }
}
#[test]
fn core_maps_to_clause_ids_and_spans() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"x\" { x; }",
    );
    adr(
        &d,
        "b.md",
        "B",
        "accepted",
        "",
        "bool x; rule C1 \"not x\" { !x; }",
    );
    let r = run_check(&d, &FixedBackend(Verdict::Unsat), &d.join("out")).unwrap();
    assert_eq!(r.conflicts.len(), 2);
    assert_eq!(r.conflicts[0].adr_id, "A");
    assert!(r.conflicts[0].span.line > 1)
}
#[test]
fn unknown_is_never_pass() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"x\" { x; }",
    );
    let r = run_check(&d, &FixedBackend(Verdict::Unknown), &d.join("out")).unwrap();
    assert_eq!(r.verdict, Verdict::Unknown);
    assert_ne!(r.verdict, Verdict::Sat)
}

#[test]
fn adrlogic_lowers_into_project_intent_ir() {
    let d = dir();
    adr(
        &d,
        "a.md",
        "A",
        "accepted",
        "",
        "bool x; rule C1 \"x\" { x; }",
    );
    let adrs = load_adrs(&d).unwrap();
    let model = lower_to_project_model(&adrs, &effective(&adrs, false).unwrap());
    assert!(model.decisions.contains_key(&DecisionId("A".into())));
    assert!(model.constraints.contains_key(&ConstraintId("A:C1".into())));
    assert!(matches!(
        model.constraints[&ConstraintId("A:C1".into())].formula,
        RelationalFormula::Name(_)
    ));
}

#[test]
fn cargo_provider_discovers_workspace_packages_and_direct_paths_offline() {
    let root = workspace(true);
    let provider = cargo_facts::CargoMetadataProvider::discover(&root).unwrap();
    let first = provider.extract().unwrap();
    let second = provider.extract().unwrap();
    let packages = first
        .facts
        .iter()
        .filter(|fact| fact.relation == "package")
        .map(|fact| fact.arguments[0].clone())
        .collect::<Vec<_>>();
    assert_eq!(packages, vec!["domain", "fake_sqlx", "repository"]);
    assert!(
        first
            .facts
            .iter()
            .any(|fact| fact.relation == "declares_direct_dependency"
                && fact.value
                && fact.arguments == ["domain", "fake_sqlx"])
    );
    assert_eq!(
        serde_json::to_string(&first.facts).unwrap(),
        serde_json::to_string(&second.facts).unwrap()
    );
}

#[test]
fn direct_dependency_violates_project_constraint() {
    let root = workspace(true);
    let (model, _) = load_project_model(&root).unwrap();
    assert!(!ground_constraint_holds(&model));
    let smt = obligation_to_smt(&relational_obligation(model));
    assert!(smt.contains("FACT:cargo:declared-direct:domain:fake_sqlx"));
}

#[test]
fn indirect_dependency_does_not_violate_direct_rule() {
    let root = workspace(false);
    let (model, _) = load_project_model(&root).unwrap();
    assert!(ground_constraint_holds(&model));
    assert!(
        !model
            .facts
            .values()
            .any(|fact| fact.relation == "declares_direct_dependency"
                && fact.value
                && fact.arguments == ["domain", "fake_sqlx"])
    );
}

#[test]
fn removing_prohibited_dependency_makes_constraint_pass() {
    let root = workspace(true);
    assert!(!ground_constraint_holds(
        &load_project_model(&root).unwrap().0
    ));
    fs::write(
        root.join("domain/Cargo.toml"),
        "[package]\nname='domain'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    assert!(ground_constraint_holds(
        &load_project_model(&root).unwrap().0
    ));
}

#[test]
fn evidence_fingerprints_become_stale_and_reverification_refreshes() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    let report = run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    assert_eq!(report.evidence_status, evidence::VerificationStatus::Pass);
    assert_eq!(
        current_evidence_status(&root, &out.join("evidence"), "Z3 4.13.4", 10_000).unwrap(),
        evidence::VerificationStatus::Pass
    );
    let manifest = root.join("domain/Cargo.toml");
    let mut changed = fs::read_to_string(&manifest).unwrap();
    changed.push_str("\n# relevant input changed\n");
    fs::write(&manifest, changed).unwrap();
    assert_eq!(
        current_evidence_status(&root, &out.join("evidence"), "Z3 4.13.4", 10_000).unwrap(),
        evidence::VerificationStatus::Stale
    );
    let refreshed = run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    assert_eq!(
        refreshed.evidence_status,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(
        current_evidence_status(&root, &out.join("evidence"), "Z3 4.13.4", 10_000).unwrap(),
        evidence::VerificationStatus::Pass
    );
}

#[test]
fn missing_evidence_and_errors_are_not_pass() {
    let root = workspace(false);
    assert_eq!(
        current_evidence_status(&root, &root.join("missing"), "Z3 4.13.4", 10_000).unwrap(),
        evidence::VerificationStatus::Unverified
    );
    assert!(!evidence::VerificationStatus::Unverified.is_ci_pass());
    assert!(!evidence::VerificationStatus::Unknown.is_ci_pass());
    assert!(!evidence::VerificationStatus::Error.is_ci_pass());
    assert!(!evidence::VerificationStatus::Stale.is_ci_pass());
}

#[test]
fn stable_json_and_ledger_except_documented_runtime_fields() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let mut first: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("proof-ledger.json")).unwrap()).unwrap();
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let mut second: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("proof-ledger.json")).unwrap()).unwrap();
    for value in [&mut first, &mut second] {
        value.as_object_mut().unwrap().remove("elapsed_ms");
        value["evidence"]
            .as_object_mut()
            .unwrap()
            .remove("recorded_at_unix_nanos");
        value["evidence"].as_object_mut().unwrap().remove("id");
    }
    assert_eq!(first, second);
}

#[test]
fn proof_graph_connects_constraint_obligation_and_evidence() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let model: ProjectModel =
        serde_json::from_slice(&fs::read(out.join("project-model.json")).unwrap()).unwrap();
    assert!(
        model
            .edges
            .iter()
            .any(|edge| matches!(edge.kind, LinkKind::ParticipatesIn))
    );
    assert!(
        model
            .edges
            .iter()
            .any(|edge| matches!(edge.kind, LinkKind::EvidenceFor))
    );
}

#[test]
fn evidence_records_every_relevant_manifest_fingerprint() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let evidence = evidence::latest(&out.join("evidence")).unwrap().unwrap();
    assert!(
        evidence
            .inputs
            .iter()
            .any(|item| item.source.ends_with("domain/Cargo.toml"))
    );
    assert!(evidence.inputs.iter().all(|item| item.sha256.len() == 64));
}

struct BrokenBackend;
impl ConstraintBackend for BrokenBackend {
    fn check(&self, _: &RelationalProofObligation, _: &Path) -> Result<BackendResult, Error> {
        Err(Error::SolverFailure("verifier crashed".into()))
    }
}

#[test]
fn backend_execution_failure_is_not_pass() {
    let root = workspace(false);
    assert!(matches!(
        run_check(&root, &BrokenBackend, &root.join(".adrproof")),
        Err(Error::SolverFailure(_))
    ));
}

#[test]
fn registry_dependency_and_package_rename_are_observable_offline() {
    let root = dir();
    fs::create_dir_all(root.join("domain/src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nresolver='2'\nmembers=['domain']\n",
    )
    .unwrap();
    fs::write(root.join("domain/src/lib.rs"), "").unwrap();
    fs::write(root.join("domain/Cargo.toml"), "[package]\nname='domain'\nversion='0.1.0'\nedition='2024'\n[dependencies]\ndatabase={package='sqlx',version='1'}\n").unwrap();
    let extracted = cargo_facts::CargoMetadataProvider::discover(&root)
        .unwrap()
        .extract()
        .unwrap();
    let dependency = extracted
        .facts
        .iter()
        .find(|fact| fact.relation == "declares_direct_dependency")
        .unwrap();
    assert_eq!(dependency.arguments, ["domain", "sqlx"]);
    assert_eq!(dependency.attributes["declared_name"], "database");
    assert_eq!(dependency.attributes["actual_package"], "sqlx");
    assert_eq!(dependency.attributes["source_kind"], "registry");
    assert!(
        extracted
            .coverage
            .iter()
            .any(|coverage| coverage.world == project::WorldAssumption::Closed)
    );
}

#[test]
fn absent_provider_coverage_cannot_produce_pass() {
    let root = dir();
    adr(
        &root,
        "a.md",
        "A",
        "accepted",
        "",
        "entity Package { domain, sqlx }; relation declares_direct_dependency(Package, Package); rule C1 \"no sqlx\" { !declares_direct_dependency(domain, sqlx); }",
    );
    let report = run_check(&root, &FixedBackend(Verdict::Sat), &root.join("out")).unwrap();
    assert_eq!(report.verdict, Verdict::Unverified);
    assert_eq!(
        report.evidence_status,
        evidence::VerificationStatus::Unverified
    );
}

#[test]
fn backend_version_and_configuration_changes_stale_evidence() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    assert_eq!(
        current_evidence_status(&root, &out.join("evidence"), "4.14.0", 10_000).unwrap(),
        evidence::VerificationStatus::Stale
    );
    assert_eq!(
        current_evidence_status(&root, &out.join("evidence"), "4.13.4", 20_000).unwrap(),
        evidence::VerificationStatus::Stale
    );
}

#[test]
fn immutable_evidence_history_retains_pass_then_fail() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    run_check(&root, &FixedBackend(Verdict::Unsat), &out).unwrap();
    let history = evidence::load_all(&out.join("evidence")).unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(
        history[0].result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(
        history[1].result_at_execution,
        evidence::VerificationStatus::Fail
    );
}

#[test]
fn impact_and_explain_follow_typed_dependency_paths() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let impact = query::impact(
        &root,
        &out,
        &root.join("domain/Cargo.toml"),
        "4.13.4",
        10_000,
    )
    .unwrap();
    let joined = serde_json::to_string(&impact).unwrap();
    assert!(joined.contains("Produces"));
    assert!(joined.contains("RelevantTo"));
    assert!(joined.contains("ParticipatesIn"));
    let explanation = query::explain(&root, &out, "ADR-1:C1", "4.13.4", 10_000).unwrap();
    assert!(explanation.subject.contains("constraint:ADR-1:C1"));
    assert!(!explanation.provenance.is_empty());
}

#[test]
fn semantic_fingerprints_are_checkout_path_independent() {
    let a = dir();
    let b = dir();
    fs::write(a.join("input"), "same").unwrap();
    fs::write(b.join("input"), "same").unwrap();
    assert_eq!(
        evidence::fingerprint_files(&a, &[a.join("input")]).unwrap(),
        evidence::fingerprint_files(&b, &[b.join("input")]).unwrap()
    );
}

#[test]
fn status_exposes_intent_without_machine_constraint() {
    let root = workspace(false);
    adr(&root, "unverified.md", "ADR-2", "accepted", "", "");
    let report = query::status(&root, &root.join(".adrproof"), "4.13.4", 10_000).unwrap();
    assert_eq!(report.unverified_intent, vec!["ADR-2"]);
}

#[test]
fn irrelevant_manifest_change_does_not_stale_global_evidence() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let path = root.join("fake_sqlx/Cargo.toml");
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("\n# no fact relevant to the constraint changed\n");
    fs::write(path, text).unwrap();
    assert_eq!(
        current_evidence_status(&root, &out.join("evidence"), "4.13.4", 10_000).unwrap(),
        evidence::VerificationStatus::Pass
    );
}

#[test]
fn impact_and_explain_json_are_deterministic() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let first = query::impact(
        &root,
        &out,
        &root.join("domain/Cargo.toml"),
        "4.13.4",
        10_000,
    )
    .unwrap();
    let second = query::impact(
        &root,
        &out,
        &root.join("domain/Cargo.toml"),
        "4.13.4",
        10_000,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&first).unwrap(),
        serde_json::to_string(&second).unwrap()
    );
}

fn external_roots() -> (PathBuf, PathBuf, PathBuf) {
    let project = workspace(false);
    let spec_root = dir();
    let state_root = dir();
    fs::rename(
        project.join("architecture.md"),
        spec_root.join("architecture.md"),
    )
    .unwrap();
    (project, spec_root, state_root)
}

fn file_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for entry in entries {
            if entry.is_dir() {
                visit(root, &entry, output);
            } else {
                output.push((
                    entry.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(entry).unwrap(),
                ));
            }
        }
    }
    let mut output = Vec::new();
    visit(root, root, &mut output);
    output
}

#[test]
fn explicit_roots_keep_project_read_only_and_state_external() {
    let (project, spec_root, state_root) = external_roots();
    let before = file_snapshot(&project);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&project, fs::Permissions::from_mode(0o555)).unwrap();
    }
    let roots = roots::VerificationRoots::explicit(&project, &spec_root, &state_root);
    let report = run_check_with_roots(&roots, &FixedBackend(Verdict::Sat)).unwrap();
    assert_eq!(report.verdict, Verdict::Sat);
    assert_eq!(before, file_snapshot(&project));
    assert!(state_root.join("evidence").is_dir());
    assert!(state_root.join("proof-ledger.json").is_file());
    assert!(!project.join(".adrproof").exists());
    assert!(
        cargo_facts::CargoMetadataProvider::discover(&project)
            .unwrap()
            .extract()
            .is_ok()
    );
    assert_eq!(before, file_snapshot(&project));
}

#[test]
fn external_roots_provenance_impact_and_explain_are_namespaced() {
    let (project, spec_root, state_root) = external_roots();
    let roots = roots::VerificationRoots::explicit(&project, &spec_root, &state_root);
    run_check_with_roots(&roots, &FixedBackend(Verdict::Sat)).unwrap();
    let (model, _) = load_project_model_with_roots(&roots).unwrap();
    assert!(model.constraints.values().all(|constraint| {
        constraint
            .provenance
            .source
            .to_string_lossy()
            .starts_with("spec:")
    }));
    assert!(model.facts.values().all(|fact| {
        fact.provenance
            .source
            .to_string_lossy()
            .starts_with("project:")
    }));
    let impact =
        query::impact_with_roots(&roots, Path::new("domain/Cargo.toml"), "4.13.4", 10_000).unwrap();
    assert!(impact.subject.contains("project:domain/Cargo.toml"));
    assert!(
        serde_json::to_string(&impact)
            .unwrap()
            .contains("ParticipatesIn")
    );
    let explanation = query::explain_with_roots(&roots, "ADR-1:C1", "4.13.4", 10_000).unwrap();
    assert!(explanation.provenance[0].starts_with("spec:"));
    let explanation_json = serde_json::to_string(&explanation).unwrap();
    assert!(explanation_json.contains("Produces"));
    assert!(explanation_json.contains("RelevantTo"));
}

#[test]
fn root_relocation_does_not_stale_semantically_identical_evidence() {
    let (project_a, spec_a, state_a) = external_roots();
    let roots_a = roots::VerificationRoots::explicit(&project_a, &spec_a, &state_a);
    run_check_with_roots(&roots_a, &FixedBackend(Verdict::Sat)).unwrap();

    let (project_b, spec_b, state_b) = external_roots();
    let roots_b = roots::VerificationRoots::explicit(&project_b, &spec_b, &state_b);
    assert_eq!(
        current_evidence_status_with_roots(
            &roots_b,
            &state_a.join("evidence"),
            "Z3 4.13.4",
            10_000,
        )
        .unwrap(),
        evidence::VerificationStatus::Pass
    );
    fs::create_dir_all(state_b.join("evidence")).unwrap();
    for item in fs::read_dir(state_a.join("evidence")).unwrap() {
        let item = item.unwrap();
        fs::copy(item.path(), state_b.join("evidence").join(item.file_name())).unwrap();
    }
    assert_eq!(
        current_evidence_status_with_roots(
            &roots_b,
            &state_b.join("evidence"),
            "Z3 4.13.4",
            10_000,
        )
        .unwrap(),
        evidence::VerificationStatus::Pass
    );
}

#[test]
fn external_spec_and_project_changes_stale_but_state_location_does_not() {
    let (project, spec_root, state_root) = external_roots();
    let roots = roots::VerificationRoots::explicit(&project, &spec_root, &state_root);
    run_check_with_roots(&roots, &FixedBackend(Verdict::Sat)).unwrap();
    let spec_path = spec_root.join("architecture.md");
    let original_spec = fs::read_to_string(&spec_path).unwrap();
    fs::write(&spec_path, format!("{original_spec}\n<!-- changed -->\n")).unwrap();
    assert_eq!(
        current_evidence_status_with_roots(
            &roots,
            &state_root.join("evidence"),
            "Z3 4.13.4",
            10_000,
        )
        .unwrap(),
        evidence::VerificationStatus::Stale
    );
    fs::write(&spec_path, original_spec).unwrap();
    let manifest = project.join("domain/Cargo.toml");
    let original_manifest = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, format!("{original_manifest}\n# changed\n")).unwrap();
    assert_eq!(
        current_evidence_status_with_roots(
            &roots,
            &state_root.join("evidence"),
            "Z3 4.13.4",
            10_000,
        )
        .unwrap(),
        evidence::VerificationStatus::Stale
    );
}

#[test]
fn corrupted_external_evidence_fails_safely() {
    let (project, spec_root, state_root) = external_roots();
    let roots = roots::VerificationRoots::explicit(&project, &spec_root, &state_root);
    fs::create_dir_all(state_root.join("evidence")).unwrap();
    fs::write(state_root.join("evidence/broken.json"), "not json").unwrap();
    assert!(query::status_with_roots(&roots, "4.13.4", 10_000).is_err());
}

#[test]
fn cargo_fact_summary_and_coverage_are_deterministic() {
    let root = workspace(false);
    let provider = cargo_facts::CargoMetadataProvider::discover(&root).unwrap();
    let first = provider.extract().unwrap();
    let second = provider.extract().unwrap();
    assert_eq!(first.fact_counts(), second.fact_counts());
    assert_eq!(
        serde_json::to_string(&(first.fact_counts(), first.coverage)).unwrap(),
        serde_json::to_string(&(second.fact_counts(), second.coverage)).unwrap()
    );
}

#[test]
fn adrlogic_accepts_real_cargo_names_with_hyphens() {
    let root = dir();
    adr(
        &root,
        "cargo-name.md",
        "BIZ-ARCH-TEST",
        "accepted",
        "",
        "entity Package { storefront-domain, search-sdk }; relation declares_direct_dependency(Package, Package); rule C1 \"Cargo names\" { !declares_direct_dependency(storefront-domain, search-sdk); }",
    );
    assert!(spec(&root).is_ok());
}

#[test]
fn closed_world_absence_uses_source_package_manifest_provenance() {
    let root = workspace(false);
    let roots = roots::VerificationRoots::explicit(&root, &root, &root.join("state"));
    let (model, _) = load_project_model_with_roots(&roots).unwrap();
    let absence = &model.facts[&project::FactId("cargo:absence:domain:fake_sqlx".into())];
    assert_eq!(
        absence.provenance.source,
        PathBuf::from("project:domain/Cargo.toml")
    );
}

fn sql_project(files: &[(&str, &str)]) -> PathBuf {
    let root = dir();
    fs::create_dir_all(root.join("migrations")).unwrap();
    for (name, sql) in files {
        fs::write(root.join("migrations").join(name), sql).unwrap();
    }
    root
}

fn sql_facts(root: &Path) -> sql_migrations::SqlMigrationFacts {
    sql_migrations::PostgresMigrationFactProvider::discover(root)
        .unwrap()
        .extract()
        .unwrap()
}

fn has_sql_fact(facts: &sql_migrations::SqlMigrationFacts, relation: &str, args: &[&str]) -> bool {
    facts.facts.iter().any(|fact| {
        fact.relation == relation
            && fact.arguments
                == args
                    .iter()
                    .map(|value| value.to_string())
                    .collect::<Vec<_>>()
    })
}

#[test]
fn postgres_migrations_are_ordered_numerically_and_duplicate_versions_fail() {
    let root = sql_project(&[
        (
            "0010_require_name.sql",
            "ALTER TABLE company ALTER COLUMN name SET NOT NULL;",
        ),
        (
            "0002_create_company.sql",
            "CREATE TABLE company (name TEXT);",
        ),
    ]);
    assert!(has_sql_fact(
        &sql_facts(&root),
        "column_not_null",
        &["public.company", "name"]
    ));
    fs::write(root.join("migrations/0002_duplicate.sql"), "SELECT 1;").unwrap();
    assert!(
        sql_migrations::PostgresMigrationFactProvider::discover(&root)
            .unwrap()
            .extract()
            .is_err()
    );
}

#[test]
fn create_table_emits_columns_keys_uniques_foreign_keys_checks_and_types() {
    let root = sql_project(&[(
        "0001_schema.sql",
        r#"
CREATE SCHEMA audit;
CREATE TABLE "Foo" (
    id UUID PRIMARY KEY,
    email TEXT UNIQUE,
    name TEXT NOT NULL,
    parent_id UUID REFERENCES "Foo"(id),
    CONSTRAINT foo_name_check CHECK (name <> '')
);
CREATE TABLE audit.fact (
    source_id UUID,
    kind TEXT,
    CONSTRAINT fact_pair_key UNIQUE (source_id, kind),
    CONSTRAINT fact_source_fk FOREIGN KEY (source_id) REFERENCES "Foo"(id),
    CONSTRAINT fact_kind_check CHECK (kind IN ('registry', 'llm'))
);
"#,
    )]);
    let facts = sql_facts(&root);
    assert!(has_sql_fact(&facts, "schema", &["audit"]));
    assert!(has_sql_fact(&facts, "table", &["public.Foo"]));
    assert!(has_sql_fact(&facts, "column", &["public.Foo", "id"]));
    assert!(has_sql_fact(
        &facts,
        "column_type",
        &["public.Foo", "id", "uuid"]
    ));
    assert!(has_sql_fact(&facts, "primary_key", &["public.Foo", "id"]));
    assert!(has_sql_fact(
        &facts,
        "column_not_null",
        &["public.Foo", "id"]
    ));
    assert!(has_sql_fact(
        &facts,
        "column_not_null",
        &["public.Foo", "name"]
    ));
    assert!(has_sql_fact(
        &facts,
        "unique_constraint",
        &["public.Foo", "email"]
    ));
    assert!(has_sql_fact(
        &facts,
        "unique_constraint",
        &["audit.fact", "source_id,kind"]
    ));
    assert!(has_sql_fact(
        &facts,
        "foreign_key",
        &["public.Foo", "parent_id", "public.Foo", "id"]
    ));
    assert!(has_sql_fact(
        &facts,
        "foreign_key",
        &["audit.fact", "source_id", "public.Foo", "id"]
    ));
    let checks = facts
        .facts
        .iter()
        .filter(|fact| fact.relation == "check_constraint")
        .collect::<Vec<_>>();
    assert_eq!(checks.len(), 2);
    assert!(checks.iter().all(|fact| {
        fact.attributes
            .get("expression_ast_json")
            .is_some_and(|expression| !expression.is_empty())
    }));
    assert!(
        facts
            .coverage
            .iter()
            .all(|coverage| { coverage.world == project::WorldAssumption::Closed })
    );
}

#[test]
fn alter_and_drop_operations_produce_only_effective_schema_state() {
    let root = sql_project(&[
        (
            "0001_create.sql",
            "CREATE TABLE item (x TEXT, y TEXT, CONSTRAINT item_pkey PRIMARY KEY (x)); CREATE TABLE obsolete(id INT);",
        ),
        (
            "0002_alter.sql",
            "ALTER TABLE item ADD COLUMN z TEXT; ALTER TABLE item ALTER COLUMN x SET NOT NULL; ALTER TABLE item ADD CONSTRAINT item_z_key UNIQUE(z);",
        ),
        (
            "0003_relax.sql",
            "ALTER TABLE item ALTER COLUMN x DROP NOT NULL; ALTER TABLE item DROP CONSTRAINT item_pkey; ALTER TABLE item DROP CONSTRAINT item_z_key; ALTER TABLE item DROP COLUMN y; DROP TABLE obsolete;",
        ),
    ]);
    let facts = sql_facts(&root);
    assert!(has_sql_fact(&facts, "column", &["public.item", "x"]));
    assert!(has_sql_fact(&facts, "column", &["public.item", "z"]));
    assert!(!has_sql_fact(&facts, "column", &["public.item", "y"]));
    assert!(!has_sql_fact(
        &facts,
        "column_not_null",
        &["public.item", "x"]
    ));
    assert!(
        !facts
            .facts
            .iter()
            .any(|fact| fact.relation == "primary_key")
    );
    assert!(
        !facts
            .facts
            .iter()
            .any(|fact| fact.relation == "unique_constraint")
    );
    assert!(!has_sql_fact(&facts, "table", &["public.obsolete"]));
}

#[test]
fn composite_primary_key_and_constraint_provenance_are_preserved() {
    let root = sql_project(&[(
        "0001_composite.sql",
        "\n\nCREATE TABLE membership (tenant UUID, member UUID, CONSTRAINT membership_pkey PRIMARY KEY (tenant, member));",
    )]);
    let facts = sql_facts(&root);
    let primary = facts
        .facts
        .iter()
        .find(|fact| fact.relation == "primary_key")
        .unwrap();
    assert_eq!(primary.arguments, ["public.membership", "tenant,member"]);
    assert_eq!(
        primary.provenance.source,
        PathBuf::from("migrations/0001_composite.sql")
    );
    assert_eq!(primary.provenance.span.as_ref().unwrap().line, 3);
}

#[test]
fn quoted_and_unquoted_identifiers_remain_distinct() {
    let root = sql_project(&[(
        "0001_names.sql",
        "CREATE TABLE Foo(id INT); CREATE TABLE \"Foo\"(id INT); CREATE TABLE custom.foo(id INT);",
    )]);
    let facts = sql_facts(&root);
    assert!(has_sql_fact(&facts, "table", &["public.foo"]));
    assert!(has_sql_fact(&facts, "table", &["public.Foo"]));
    assert!(has_sql_fact(&facts, "table", &["custom.foo"]));
}

#[test]
fn unsupported_mutating_sql_degrades_every_selected_relation_to_partial() {
    let root = sql_project(&[(
        "0001_dynamic.sql",
        "CREATE TABLE known(id INT); DO $$ BEGIN EXECUTE 'ALTER TABLE known ADD COLUMN hidden TEXT'; END $$;",
    )]);
    let facts = sql_facts(&root);
    assert_eq!(facts.unsupported.len(), 1);
    assert_eq!(
        facts.unsupported[0].effect,
        sql_migrations::SchemaEffect::UnknownSchemaEffect
    );
    let model = project::ProjectModel {
        fact_coverage: facts.coverage.clone(),
        ..Default::default()
    };
    assert_eq!(
        model.coverage_for(
            "column",
            &project::CoverageScope::Table("public.known".into())
        ),
        Some(project::WorldAssumption::Partial)
    );
    assert!(
        facts
            .facts
            .iter()
            .any(|fact| fact.relation == "unsupported_sql")
    );
}

struct GroundBackend;
impl ConstraintBackend for GroundBackend {
    fn check(
        &self,
        obligation: &RelationalProofObligation,
        path: &Path,
    ) -> Result<BackendResult, Error> {
        fs::write(path, obligation_to_smt(obligation)).unwrap();
        Ok(BackendResult {
            verdict: if ground_constraint_holds(&obligation.model) {
                Verdict::Sat
            } else {
                Verdict::Unsat
            },
            core: obligation
                .model
                .constraints
                .keys()
                .map(|id| id.0.clone())
                .collect(),
            solver_version: "Z3 4.13.4".into(),
            elapsed: Duration::from_millis(1),
            timeout_ms: 10_000,
        })
    }
}

#[test]
fn sql_facts_drive_pass_stale_fail_restore_pass_and_relocate() {
    let project = sql_project(&[("0001_users.sql", "CREATE TABLE users(id UUID PRIMARY KEY);")]);
    let spec_root = dir();
    let state_root = dir();
    adr(
        &spec_root,
        "schema.md",
        "DB-1",
        "accepted",
        "",
        "entity Table { public.users }; entity Column { id }; relation primary_key(Table, Column); rule C1 \"users pk\" { primary_key(public.users, id); }",
    );
    let roots = roots::VerificationRoots::explicit(&project, &spec_root, &state_root);
    assert_eq!(
        run_check_with_roots(&roots, &GroundBackend)
            .unwrap()
            .verdict,
        Verdict::Sat
    );
    let migration = project.join("migrations/0002_drop_pk.sql");
    fs::write(&migration, "ALTER TABLE users DROP CONSTRAINT users_pkey;").unwrap();
    assert_eq!(
        current_evidence_status_with_roots(
            &roots,
            &state_root.join("evidence"),
            "Z3 4.13.4",
            10_000,
        )
        .unwrap(),
        evidence::VerificationStatus::Stale
    );
    assert_eq!(
        run_check_with_roots(&roots, &GroundBackend)
            .unwrap()
            .verdict,
        Verdict::Unsat
    );
    fs::remove_file(&migration).unwrap();
    assert_eq!(
        run_check_with_roots(&roots, &GroundBackend)
            .unwrap()
            .verdict,
        Verdict::Sat
    );

    let relocated = sql_project(&[("0001_users.sql", "CREATE TABLE users(id UUID PRIMARY KEY);")]);
    let relocated_roots = roots::VerificationRoots::explicit(&relocated, &spec_root, &dir());
    assert_eq!(
        current_evidence_status_with_roots(
            &relocated_roots,
            &state_root.join("evidence"),
            "Z3 4.13.4",
            10_000,
        )
        .unwrap(),
        evidence::VerificationStatus::Pass
    );
}

#[test]
fn sql_comment_change_is_conservatively_stale() {
    let project = sql_project(&[("0001_users.sql", "CREATE TABLE users(id UUID PRIMARY KEY);")]);
    let spec_root = dir();
    let state_root = dir();
    adr(
        &spec_root,
        "schema.md",
        "DB-1",
        "accepted",
        "",
        "entity Table { public.users }; entity Column { id }; relation primary_key(Table, Column); rule C1 \"users pk\" { primary_key(public.users, id); }",
    );
    let roots = roots::VerificationRoots::explicit(&project, &spec_root, &state_root);
    run_check_with_roots(&roots, &GroundBackend).unwrap();
    let migration = project.join("migrations/0001_users.sql");
    let sql = fs::read_to_string(&migration).unwrap();
    fs::write(migration, format!("{sql}\n-- comment only\n")).unwrap();
    assert_eq!(
        current_evidence_status_with_roots(
            &roots,
            &state_root.join("evidence"),
            "Z3 4.13.4",
            10_000,
        )
        .unwrap(),
        evidence::VerificationStatus::Stale
    );
}

#[test]
fn partial_sql_coverage_cannot_turn_absence_into_pass() {
    let project = sql_project(&[(
        "0001_dynamic.sql",
        "CREATE TABLE users(id UUID); DO $$ BEGIN EXECUTE 'ALTER TABLE users ADD PRIMARY KEY(id)'; END $$;",
    )]);
    let spec_root = dir();
    adr(
        &spec_root,
        "schema.md",
        "DB-1",
        "accepted",
        "",
        "entity Table { public.users }; entity Column { id }; relation primary_key(Table, Column); rule C1 \"no pk\" { !primary_key(public.users, id); }",
    );
    let roots = roots::VerificationRoots::explicit(&project, &spec_root, &dir());
    let report = run_check_with_roots(&roots, &GroundBackend).unwrap();
    assert_eq!(report.verdict, Verdict::Unverified);
    assert_eq!(
        report.evidence_status,
        evidence::VerificationStatus::Unverified
    );
}

#[test]
fn cargo_and_sql_providers_coexist_in_one_project_model() {
    let project = workspace(false);
    fs::create_dir_all(project.join("migrations")).unwrap();
    fs::write(
        project.join("migrations/0001_table.sql"),
        "CREATE TABLE users(id UUID PRIMARY KEY);",
    )
    .unwrap();
    let roots = roots::VerificationRoots::explicit(&project, &project, &dir());
    let (model, _) = load_project_model_with_roots(&roots).unwrap();
    assert!(model.facts.values().any(|fact| fact.relation == "package"));
    assert!(model.facts.values().any(|fact| fact.relation == "table"));
    assert!(
        model
            .fact_coverage
            .iter()
            .any(|coverage| coverage.provider == "cargo_metadata")
    );
    assert!(
        model
            .fact_coverage
            .iter()
            .any(|coverage| coverage.provider == "postgres_migrations")
    );

    let first = serde_json::to_string(&model.facts).unwrap();
    let first_coverage = serde_json::to_string(&model.fact_coverage).unwrap();
    let (second_model, _) = load_project_model_with_roots(&roots).unwrap();
    let second = serde_json::to_string(&second_model.facts).unwrap();
    let second_coverage = serde_json::to_string(&second_model.fact_coverage).unwrap();
    assert_eq!(
        first, second,
        "combined provider facts must be deterministic"
    );
    assert_eq!(first_coverage, second_coverage);
}

fn coverage(
    relation: &str,
    world: project::WorldAssumption,
    scope: project::CoverageScope,
) -> project::FactCoverage {
    project::FactCoverage {
        relation: relation.into(),
        provider: "test".into(),
        world,
        scope,
        qualifiers: BTreeMap::new(),
        statement: "test completeness claim".into(),
        diagnostics: Vec::new(),
    }
}

#[test]
fn typed_scoped_coverage_resolves_uncertainty_conservatively() {
    let model = project::ProjectModel {
        fact_coverage: vec![
            coverage(
                "foreign_key",
                project::WorldAssumption::Closed,
                project::CoverageScope::Global,
            ),
            coverage(
                "foreign_key",
                project::WorldAssumption::Partial,
                project::CoverageScope::Schema("analytics".into()),
            ),
            coverage(
                "primary_key",
                project::WorldAssumption::Closed,
                project::CoverageScope::Table("public.entity".into()),
            ),
            coverage(
                "primary_key",
                project::WorldAssumption::Partial,
                project::CoverageScope::Global,
            ),
        ],
        ..Default::default()
    };
    assert_eq!(
        model.coverage_for(
            "foreign_key",
            &project::CoverageScope::Table("public.entity".into())
        ),
        Some(project::WorldAssumption::Closed)
    );
    assert_eq!(
        model.coverage_for(
            "foreign_key",
            &project::CoverageScope::Table("analytics.tmp".into())
        ),
        Some(project::WorldAssumption::Partial)
    );
    assert_eq!(
        model.coverage_for(
            "primary_key",
            &project::CoverageScope::Table("public.entity".into())
        ),
        Some(project::WorldAssumption::Partial),
        "global uncertainty must override a local completeness claim"
    );
}

#[test]
fn ctas_and_materialized_views_have_precise_object_scoped_coverage() {
    let root = sql_project(&[(
        "0001_objects.sql",
        "CREATE TABLE stable(id UUID PRIMARY KEY); \
         CREATE TABLE analytics.tmp AS SELECT id FROM stable; \
         CREATE MATERIALIZED VIEW analytics.rollup AS SELECT id FROM stable;",
    )]);
    let facts = sql_facts(&root);
    assert!(has_sql_fact(&facts, "table", &["public.stable"]));
    assert!(has_sql_fact(&facts, "table", &["analytics.tmp"]));
    assert!(has_sql_fact(
        &facts,
        "materialized_view",
        &["analytics.rollup"]
    ));
    assert!(!has_sql_fact(&facts, "table", &["analytics.rollup"]));
    let model = project::ProjectModel {
        fact_coverage: facts.coverage,
        ..Default::default()
    };
    assert_eq!(
        model.coverage_for(
            "column",
            &project::CoverageScope::Table("public.stable".into())
        ),
        Some(project::WorldAssumption::Closed)
    );
    assert_eq!(
        model.coverage_for(
            "column",
            &project::CoverageScope::Table("analytics.tmp".into())
        ),
        Some(project::WorldAssumption::Partial)
    );
    assert_eq!(
        model.coverage_for(
            "column_type",
            &project::CoverageScope::MaterializedView("analytics.rollup".into())
        ),
        Some(project::WorldAssumption::Partial)
    );
    assert_eq!(
        model.coverage_for(
            "foreign_key",
            &project::CoverageScope::Table("public.stable".into())
        ),
        Some(project::WorldAssumption::Closed)
    );
}

#[test]
fn partition_and_inheritance_emit_lineage_and_only_sound_inherited_properties() {
    let root = sql_project(&[(
        "0001_lineage.sql",
        "CREATE TABLE partition_parent(id UUID PRIMARY KEY, kind TEXT NOT NULL, CHECK(kind <> '')); \
         CREATE TABLE partition_child PARTITION OF partition_parent DEFAULT; \
         CREATE TABLE inheritance_child(extra TEXT) INHERITS (partition_parent);",
    )]);
    let facts = sql_facts(&root);
    for child in ["public.partition_child", "public.inheritance_child"] {
        assert!(has_sql_fact(&facts, "column", &[child, "id"]));
        assert!(has_sql_fact(&facts, "column_not_null", &[child, "id"]));
        assert!(has_sql_fact(&facts, "column", &[child, "kind"]));
        assert!(has_sql_fact(&facts, "column_not_null", &[child, "kind"]));
        assert!(facts.facts.iter().any(|fact| {
            fact.relation == "check_constraint" && fact.arguments.first() == Some(&child.into())
        }));
    }
    assert!(has_sql_fact(
        &facts,
        "partition_of",
        &["public.partition_child", "public.partition_parent"]
    ));
    assert!(has_sql_fact(
        &facts,
        "inherits",
        &["public.inheritance_child", "public.partition_parent"]
    ));
    assert!(!has_sql_fact(
        &facts,
        "primary_key",
        &["public.inheritance_child", "id"]
    ));
    let model = project::ProjectModel {
        fact_coverage: facts.coverage,
        ..Default::default()
    };
    assert_eq!(
        model.coverage_for(
            "primary_key",
            &project::CoverageScope::Table("public.partition_child".into())
        ),
        Some(project::WorldAssumption::Partial)
    );
    assert_eq!(
        model.coverage_for(
            "primary_key",
            &project::CoverageScope::Table("public.inheritance_child".into())
        ),
        Some(project::WorldAssumption::Closed)
    );
}

#[test]
fn deterministic_do_effect_classification_limits_coverage_damage() {
    let safe = sql_project(&[(
        "0001_safe.sql",
        "CREATE TABLE item(id INT); DO $$ BEGIN IF EXISTS (SELECT 1 FROM item) THEN RAISE EXCEPTION 'not empty'; END IF; END $$;",
    )]);
    let safe_facts = sql_facts(&safe);
    assert_eq!(
        safe_facts.unsupported[0].effect,
        sql_migrations::SchemaEffect::KnownIrrelevantToSchema
    );
    assert!(safe_facts.coverage.iter().all(|item| {
        item.world == project::WorldAssumption::Closed
            || item.scope != project::CoverageScope::Global
    }));

    let unsafe_root = sql_project(&[(
        "0001_unsafe.sql",
        "CREATE TABLE item(id INT); DO $$ BEGIN EXECUTE 'ALTER TABLE item ADD COLUMN hidden TEXT'; END $$;",
    )]);
    let unsafe_facts = sql_facts(&unsafe_root);
    assert_eq!(
        unsafe_facts.unsupported[0].effect,
        sql_migrations::SchemaEffect::UnknownSchemaEffect
    );
    let model = project::ProjectModel {
        fact_coverage: unsafe_facts.coverage,
        ..Default::default()
    };
    assert_eq!(
        model.coverage_for(
            "column",
            &project::CoverageScope::Table("public.anything".into())
        ),
        Some(project::WorldAssumption::Partial)
    );
}

#[test]
fn positive_observed_fact_survives_partial_but_absence_does_not() {
    let project = sql_project(&[(
        "0001_schema.sql",
        "CREATE TABLE users(id UUID PRIMARY KEY); DO $$ BEGIN EXECUTE 'SELECT 1'; END $$;",
    )]);
    let positive_spec = dir();
    adr(
        &positive_spec,
        "schema.md",
        "DB-POS",
        "accepted",
        "",
        "entity Table { public.users }; entity Column { id }; relation primary_key(Table, Column); rule C1 \"pk exists\" { primary_key(public.users, id); }",
    );
    let positive_roots = roots::VerificationRoots::explicit(&project, &positive_spec, &dir());
    assert_eq!(
        run_check_with_roots(&positive_roots, &GroundBackend)
            .unwrap()
            .verdict,
        Verdict::Sat
    );

    let negative_spec = dir();
    adr(
        &negative_spec,
        "schema.md",
        "DB-NEG",
        "accepted",
        "",
        "entity Table { public.users }; entity Column { missing }; relation primary_key(Table, Column); rule C1 \"missing pk\" { !primary_key(public.users, missing); }",
    );
    let negative_roots = roots::VerificationRoots::explicit(&project, &negative_spec, &dir());
    assert_eq!(
        run_check_with_roots(&negative_roots, &GroundBackend)
            .unwrap()
            .verdict,
        Verdict::Unverified
    );
}

#[test]
fn unrelated_ctas_partial_does_not_block_authoritative_table_result() {
    let project = sql_project(&[(
        "0001_schema.sql",
        "CREATE TABLE entity(id UUID PRIMARY KEY); \
         CREATE TABLE entity_source_record(entity_id UUID REFERENCES entity(id)); \
         CREATE TABLE analytics.tmp AS SELECT id FROM entity;",
    )]);
    let spec = dir();
    adr(
        &spec,
        "schema.md",
        "DB-FK",
        "accepted",
        "",
        "entity Table { public.entity_source_record, public.entity }; entity Column { entity_id, id }; relation foreign_key(Table, Column, Table, Column); rule C1 \"source FK\" { foreign_key(public.entity_source_record, entity_id, public.entity, id); }",
    );
    let roots = roots::VerificationRoots::explicit(&project, &spec, &dir());
    assert_eq!(
        run_check_with_roots(&roots, &GroundBackend)
            .unwrap()
            .verdict,
        Verdict::Sat
    );
}

#[test]
fn sql_closed_world_absence_uses_nearest_effective_object_provenance() {
    let project = sql_project(&[("0001_users.sql", "CREATE TABLE users(id UUID, email TEXT);")]);
    let spec = dir();
    adr(
        &spec,
        "schema.md",
        "DB-ABSENCE",
        "accepted",
        "",
        "entity Table { public.users }; entity Column { id }; relation primary_key(Table, Column); rule C1 \"no id pk\" { !primary_key(public.users, id); }",
    );
    let roots = roots::VerificationRoots::explicit(&project, &spec, &dir());
    let (model, _) = load_project_model_with_roots(&roots).unwrap();
    let absence = model
        .facts
        .values()
        .find(|fact| {
            fact.relation == "primary_key"
                && fact.arguments == ["public.users", "id"]
                && !fact.value
        })
        .unwrap();
    assert_eq!(
        absence.provenance.source,
        PathBuf::from("project:migrations/0001_users.sql")
    );
    assert_eq!(absence.provenance.span.as_ref().unwrap().line, 1);
}

fn scenario_fixture(output: &str) -> (roots::VerificationRoots, scenario::ScenarioDefinition) {
    let project = dir();
    let specification = dir();
    let state = dir();
    fs::write(project.join("implementation.rs"), "fn pipeline() {}\n").unwrap();
    fs::write(specification.join("fixture.json"), "{\"entity\":1}\n").unwrap();
    let source = specification.join("scenarios/test.json");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "{}\n").unwrap();
    let definition = scenario::ScenarioDefinition {
        id: "TEST-001".into(),
        version: "1".into(),
        description: "deterministic scenario".into(),
        claim: "the exact scenario satisfies its postcondition".into(),
        authority: "this exact deterministic execution only".into(),
        does_not_prove: vec!["all executions".into()],
        coverage: scenario::ScenarioCoverage {
            fault_class: "transport".into(),
            fault_point: scenario::FaultPoint::MeiliRequestTimeout,
            state_space_scope: "one event".into(),
            concurrency_scope: "one worker".into(),
            covered: vec!["timeout before UID".into()],
            not_covered: vec!["arbitrary crashes".into()],
        },
        runner: scenario::ScenarioCommand {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), format!("printf '%s' '{output}'")],
            timeout_ms: 1_000,
            runner_version: "test-runner-1".into(),
        },
        expected_postconditions: BTreeMap::from([(
            "work_recoverable".into(),
            serde_json::Value::Bool(true),
        )]),
        inputs: vec![
            scenario::ScenarioInput {
                root: scenario::InputRoot::Project,
                path: "implementation.rs".into(),
            },
            scenario::ScenarioInput {
                root: scenario::InputRoot::Specification,
                path: "fixture.json".into(),
            },
        ],
        source,
    };
    (
        roots::VerificationRoots::explicit(&project, &specification, &state),
        definition,
    )
}

fn native_test_fixture() -> (
    roots::VerificationRoots,
    crate::native_test::NativeTestDefinition,
    PathBuf,
) {
    use crate::native_test::{NativeTestCase, NativeTestCaseStatus, NativeTestReport};

    let project = dir();
    let specification = dir();
    let state = dir();
    fs::write(project.join("worker.rs"), "fn worker() {}\n").unwrap();
    let source = specification.join("native-tests/checks/integration.json");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "{}\n").unwrap();
    let definition = crate::native_test::NativeTestDefinition {
        id: "INTEGRATION".into(),
        version: "1".into(),
        claim: "selected native suite passes".into(),
        authority: "the imported native test execution only".into(),
        does_not_prove: vec!["all environments".into()],
        command: "cargo nextest run --workspace --run-ignored all".into(),
        working_directory: "backend".into(),
        minimum_passed: 2,
        maximum_skipped: 0,
        required_tests: vec!["worker::heartbeat".into()],
        inputs: vec![scenario::ScenarioInput {
            root: scenario::InputRoot::Project,
            path: "worker.rs".into(),
        }],
        excluded_inputs: Vec::new(),
        source,
    };
    fs::write(
        &definition.source,
        serde_json::to_vec_pretty(&definition).unwrap(),
    )
    .unwrap();
    let report = state.join("report.json");
    fs::write(
        &report,
        serde_json::to_vec_pretty(&NativeTestReport {
            schema_version: crate::native_test::REPORT_SCHEMA.into(),
            runner: "cargo-nextest".into(),
            runner_version: "0.9".into(),
            command: definition.command.clone(),
            working_directory: definition.working_directory.clone(),
            result: evidence::VerificationStatus::Pass,
            passed: 2,
            failed: 0,
            skipped: 0,
            duration_seconds: 1.25,
            tests: vec![NativeTestCase {
                name: "worker::heartbeat".into(),
                status: NativeTestCaseStatus::Pass,
            }],
            diagnostics: Vec::new(),
        })
        .unwrap(),
    )
    .unwrap();
    (
        roots::VerificationRoots::explicit(&project, &specification, &state),
        definition,
        report,
    )
}

#[test]
fn native_test_import_is_non_vacuous_immutable_and_precisely_stale() {
    use crate::native_test;

    let (roots, definition, report) = native_test_fixture();
    let evidence = native_test::import(&roots, &definition, &report).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert!(evidence.non_vacuity.non_empty_execution);
    assert!(evidence.non_vacuity.all_required_observed_pass);
    let stored =
        native_test::store(&roots.state_root.join("native-test-evidence"), evidence).unwrap();
    assert_eq!(
        native_test::assess(&roots, &definition, &stored).unwrap(),
        evidence::EvidenceValidity::Current
    );

    fs::write(roots.project_root.join("unrelated.txt"), "irrelevant\n").unwrap();
    assert_eq!(
        native_test::assess(&roots, &definition, &stored).unwrap(),
        evidence::EvidenceValidity::Current
    );
    fs::write(roots.project_root.join("worker.rs"), "fn changed() {}\n").unwrap();
    assert_eq!(
        native_test::assess(&roots, &definition, &stored).unwrap(),
        evidence::EvidenceValidity::Stale
    );
}

#[test]
fn native_test_import_rejects_missing_required_test_and_empty_execution() {
    use crate::native_test::{self, NativeTestReport};

    let (roots, definition, report) = native_test_fixture();
    let mut value: NativeTestReport = serde_json::from_slice(&fs::read(&report).unwrap()).unwrap();
    value.passed = 0;
    value.tests.clear();
    fs::write(&report, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let evidence = native_test::import(&roots, &definition, &report).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Fail
    );
    assert!(!evidence.non_vacuity.non_empty_execution);
    assert!(!evidence.non_vacuity.all_required_observed_pass);
    assert!(
        evidence
            .diagnostics
            .iter()
            .any(|line| line.contains("required test"))
    );
}

#[test]
fn evidence_bundle_verifies_offline_and_detects_tampering() {
    let project = dir();
    let specification = dir();
    let state = dir();
    fs::create_dir_all(state.join("scenario-evidence")).unwrap();
    fs::write(
        state.join("scenario-evidence/EVIDENCE.json"),
        "{\"status\":\"PASS\"}\n",
    )
    .unwrap();
    fs::write(state.join("proof-ledger.json"), "{\"entries\":[]}\n").unwrap();
    let roots = roots::VerificationRoots::explicit(&project, &specification, &state);
    let output = dir().join("bundle");
    let manifest = crate::bundle::create(&roots, &output).unwrap();
    assert_eq!(manifest.files.len(), 2);
    let verified = crate::bundle::verify(&output).unwrap();
    assert!(verified.valid);
    assert_eq!(verified.verified_files, 2);

    fs::write(
        output.join("data/scenario-evidence/EVIDENCE.json"),
        "tampered\n",
    )
    .unwrap();
    let tampered = crate::bundle::verify(&output).unwrap();
    assert!(!tampered.valid);
    assert!(
        tampered
            .diagnostics
            .iter()
            .any(|line| line.contains("SHA-256 mismatch"))
    );
}

#[test]
fn signed_bundle_requires_a_valid_trusted_ed25519_key() {
    let project = dir();
    let specification = dir();
    let state = dir();
    fs::write(state.join("proof-ledger.json"), "{\"entries\":[]}\n").unwrap();
    let roots = roots::VerificationRoots::explicit(&project, &specification, &state);
    let output = dir().join("signed-bundle");
    let secret = [7u8; 32];
    let public = ed25519_dalek::SigningKey::from_bytes(&secret)
        .verifying_key()
        .to_bytes();
    crate::bundle::create_signed(&roots, &output, &secret).unwrap();

    let verified = crate::bundle::verify_with_key(&output, Some(&public), true).unwrap();
    assert!(verified.valid);
    assert!(verified.signature.present);
    assert!(verified.signature.cryptographically_valid);
    assert_eq!(verified.signature.trusted_key_match, Some(true));

    let wrong = [9u8; 32];
    let untrusted = crate::bundle::verify_with_key(&output, Some(&wrong), true).unwrap();
    assert!(!untrusted.valid);
    assert_eq!(untrusted.signature.trusted_key_match, Some(false));

    let manifest_path = output.join("bundle.json");
    let changed = fs::read_to_string(&manifest_path)
        .unwrap()
        .replace("Offline integrity", "Tampered integrity");
    fs::write(&manifest_path, changed).unwrap();
    let tampered = crate::bundle::verify_with_key(&output, Some(&public), true).unwrap();
    assert!(!tampered.valid);
    assert!(!tampered.signature.cryptographically_valid);
}

#[test]
fn diagnostic_waivers_expire_and_sarif_preserves_the_underlying_finding() {
    let finding = serde_json::json!({
        "kind": "scenario",
        "id": "SCENARIO:S1",
        "source": "spec:scenarios/S1.json",
        "status": "FAIL"
    });
    let active = crate::policy::DiagnosticPolicy {
        schema_version: crate::policy::POLICY_SCHEMA.into(),
        waivers: vec![crate::policy::Waiver {
            id: "W-1".into(),
            finding_id: "SCENARIO:S1".into(),
            owner: "verification-team".into(),
            reason: "bounded migration window".into(),
            expires_unix_seconds: 200,
        }],
    };
    crate::policy::validate(&active).unwrap();
    let waived = crate::policy::apply(vec![finding.clone()], &active, 100);
    assert_eq!(waived.unwaived_finding_count, 0);
    assert_eq!(waived.applied_waivers.len(), 1);
    assert_eq!(waived.findings[0]["status"], "FAIL");
    assert!(waived.findings[0].get("waiver").is_some());
    let sarif = crate::policy::sarif(&waived.findings);
    assert_eq!(sarif["runs"][0]["results"][0]["level"], "error");
    assert_eq!(
        sarif["runs"][0]["results"][0]["suppressions"][0]["status"],
        "accepted"
    );

    let expired = crate::policy::apply(vec![finding], &active, 200);
    assert_eq!(expired.unwaived_finding_count, 1);
    assert!(expired.applied_waivers.is_empty());
    assert!(expired.diagnostics[0].contains("expired"));
}

#[test]
fn heterogeneous_impact_reaches_native_test_evidence_and_parent() {
    let (roots, definition, report) = native_test_fixture();
    let stored = crate::native_test::store(
        &roots.state_root.join("native-test-evidence"),
        crate::native_test::import(&roots, &definition, &report).unwrap(),
    )
    .unwrap();
    fs::create_dir_all(roots.specification_root.join("scenarios")).unwrap();
    fs::write(
        roots.specification_root.join("scenarios/parents.json"),
        serde_json::to_vec_pretty(&vec![scenario::ParentObligation {
            id: "PARENT-NATIVE".into(),
            claim: "native integration suite passes".into(),
            authority: "the native test child only".into(),
            required_children: vec![scenario::RequiredChild {
                obligation_id: "NATIVE-TEST:INTEGRATION".into(),
                evidence_kind: scenario::ChildEvidenceKind::NativeTest,
            }],
            source: PathBuf::new(),
        }])
        .unwrap(),
    )
    .unwrap();

    let report = query::heterogeneous_impact_with_roots(
        &roots,
        &roots.project_root.join("worker.rs"),
        "4.13.4",
        10_000,
    )
    .unwrap();
    assert!(
        report
            .affected_obligations
            .contains(&"NATIVE-TEST:INTEGRATION".into())
    );
    assert!(report.affected_parents.contains(&"PARENT-NATIVE".into()));
    assert!(report.affected_evidence.contains(&stored.id.0));
    assert!(
        report
            .paths
            .iter()
            .flatten()
            .any(|edge| edge.contains("RequiredBy"))
    );
}

#[test]
fn scenario_postconditions_deterministically_decide_pass_and_fail() {
    let pass_output =
        r#"{"postconditions":{"work_recoverable":true},"trace":["ignored by verdict"]}"#;
    let (roots, definition) = scenario_fixture(pass_output);
    let first = scenario::run(&roots, &definition).unwrap();
    let second = scenario::run(&roots, &definition).unwrap();
    assert_eq!(
        first.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(first.postconditions, second.postconditions);
    assert_eq!(
        first.implementation_fingerprint,
        second.implementation_fingerprint
    );
    assert_eq!(first.fixture_fingerprint, second.fixture_fingerprint);

    let fail_output = r#"{"postconditions":{"work_recoverable":false},"trace":["looks good"]}"#;
    let (_, failing) = scenario_fixture(fail_output);
    let result = scenario::run(
        &roots,
        &scenario::ScenarioDefinition {
            source: definition.source.clone(),
            inputs: definition.inputs.clone(),
            ..failing
        },
    )
    .unwrap();
    assert_eq!(
        result.result_at_execution,
        evidence::VerificationStatus::Fail
    );
    assert!(!result.postconditions[0].passed);
}

#[test]
fn scenario_runner_path_can_be_spec_relative_and_relocatable() {
    use std::os::unix::fs::PermissionsExt;

    let output = r#"{"postconditions":{"work_recoverable":true}}"#;
    let (roots, mut definition) = scenario_fixture(output);
    let runner = roots.specification_root.join("harness/run.sh");
    fs::create_dir_all(runner.parent().unwrap()).unwrap();
    fs::write(
        &runner,
        "#!/bin/sh\nprintf '%s' '{\"postconditions\":{\"work_recoverable\":true}}'\n",
    )
    .unwrap();
    fs::set_permissions(&runner, fs::Permissions::from_mode(0o755)).unwrap();
    definition.runner.program = "harness/run.sh".into();
    definition.runner.args.clear();

    let evidence = scenario::run(&roots, &definition).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(definition.runner.program, PathBuf::from("harness/run.sh"));
}

#[test]
fn scenario_infrastructure_failure_is_error_not_invariant_fail() {
    let output = r#"{"infrastructure_error":"postgres did not start"}"#;
    let (roots, definition) = scenario_fixture(output);
    let evidence = scenario::run(&roots, &definition).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Error
    );
}

#[test]
fn scenario_inputs_definition_fixture_runner_and_implementation_control_staleness() {
    let output = r#"{"postconditions":{"work_recoverable":true}}"#;
    let (roots, mut definition) = scenario_fixture(output);
    let evidence = scenario::run(&roots, &definition).unwrap();
    assert_eq!(
        scenario::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Current
    );
    fs::write(
        roots.project_root.join("implementation.rs"),
        "fn changed() {}\n",
    )
    .unwrap();
    assert_eq!(
        scenario::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    fs::write(
        roots.project_root.join("implementation.rs"),
        "fn pipeline() {}\n",
    )
    .unwrap();
    fs::write(
        roots.specification_root.join("fixture.json"),
        "{\"entity\":2}\n",
    )
    .unwrap();
    assert_eq!(
        scenario::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    fs::write(
        roots.specification_root.join("fixture.json"),
        "{\"entity\":1}\n",
    )
    .unwrap();
    definition.runner.runner_version = "test-runner-2".into();
    assert_eq!(
        scenario::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Stale
    );
}

#[test]
fn scenario_history_is_immutable_and_relocation_is_semantic() {
    let output = r#"{"postconditions":{"work_recoverable":true}}"#;
    let (roots, definition) = scenario_fixture(output);
    let first = scenario::store(
        &roots.state_root.join("scenario-evidence"),
        scenario::run(&roots, &definition).unwrap(),
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let second = scenario::store(
        &roots.state_root.join("scenario-evidence"),
        scenario::run(&roots, &definition).unwrap(),
    )
    .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(
        scenario::load_all(&roots.state_root.join("scenario-evidence"))
            .unwrap()
            .len(),
        2
    );
    let relocated_state = dir();
    fs::rename(
        roots.state_root.join("scenario-evidence"),
        relocated_state.join("scenario-evidence"),
    )
    .unwrap();
    let relocated = roots::VerificationRoots::explicit(
        &roots.project_root,
        &roots.specification_root,
        &relocated_state,
    );
    assert_eq!(
        scenario::latest_assessment(&relocated, &definition)
            .unwrap()
            .unwrap()
            .current_validity,
        evidence::EvidenceValidity::Current
    );
}

fn parent(children: &[(&str, scenario::ChildEvidenceKind)]) -> scenario::ParentObligation {
    scenario::ParentObligation {
        id: "PARENT".into(),
        claim: "all required scoped claims hold".into(),
        authority: "intersection of child scopes".into(),
        required_children: children
            .iter()
            .map(|(id, kind)| scenario::RequiredChild {
                obligation_id: (*id).into(),
                evidence_kind: kind.clone(),
            })
            .collect(),
        source: PathBuf::new(),
    }
}

fn child(
    id: &str,
    status: evidence::VerificationStatus,
    validity: Option<evidence::EvidenceValidity>,
) -> scenario::ChildStatus {
    scenario::ChildStatus {
        obligation_id: id.into(),
        status,
        validity,
        evidence_id: Some(project::EvidenceId(format!("EVIDENCE:{id}"))),
    }
}

#[test]
fn aggregate_requires_all_children_current_pass() {
    use evidence::{EvidenceValidity::*, VerificationStatus::*};
    let parent = parent(&[
        ("DB", scenario::ChildEvidenceKind::Relational),
        ("S1", scenario::ChildEvidenceKind::Scenario),
    ]);
    assert_eq!(
        scenario::aggregate(
            &parent,
            vec![
                child("DB", Pass, Some(Current)),
                child("S1", Pass, Some(Current))
            ]
        )
        .status,
        Pass
    );
    assert_eq!(
        scenario::aggregate(
            &parent,
            vec![
                child("DB", Pass, Some(Current)),
                child("S1", Fail, Some(Current))
            ]
        )
        .status,
        Fail
    );
    assert_eq!(
        scenario::aggregate(
            &parent,
            vec![
                child("DB", Pass, Some(Current)),
                child("S1", Pass, Some(evidence::EvidenceValidity::Stale)),
            ]
        )
        .status,
        evidence::VerificationStatus::Stale
    );
    assert_eq!(
        scenario::aggregate(&parent, vec![child("DB", Pass, Some(Current))]).status,
        Unverified
    );
    assert_eq!(
        scenario::aggregate(
            &parent,
            vec![
                child("DB", Pass, Some(Current)),
                child("S1", Error, Some(Current))
            ]
        )
        .status,
        Error
    );
}

#[test]
fn model_and_validation_children_obey_parent_stale_gate() {
    use evidence::{EvidenceValidity::*, VerificationStatus::*};
    let parent = parent(&[
        ("MODEL:SAFETY", scenario::ChildEvidenceKind::Model),
        (
            "MODEL-VALIDATION:SCENARIOS",
            scenario::ChildEvidenceKind::ModelValidation,
        ),
    ]);
    assert_eq!(
        scenario::aggregate(
            &parent,
            vec![
                child("MODEL:SAFETY", Pass, Some(Current)),
                child(
                    "MODEL-VALIDATION:SCENARIOS",
                    Pass,
                    Some(evidence::EvidenceValidity::Stale),
                ),
            ],
        )
        .status,
        evidence::VerificationStatus::Stale
    );
}

#[test]
fn formal_model_pass_cannot_substitute_for_implementation_conformance_child() {
    use evidence::{EvidenceValidity::Current, VerificationStatus::*};
    let parent = parent(&[
        ("MODEL:SAFETY", scenario::ChildEvidenceKind::Model),
        (
            "SCENARIO:IMPLEMENTATION-CONFORMANCE",
            scenario::ChildEvidenceKind::Scenario,
        ),
    ]);
    assert_eq!(
        scenario::aggregate(&parent, vec![child("MODEL:SAFETY", Pass, Some(Current))],).status,
        Unverified
    );
}

#[test]
fn scenario_graph_connects_inputs_obligations_evidence_and_required_children() {
    let (roots, definition) = scenario_fixture(r#"{"postconditions":{"work_recoverable":true}}"#);
    let evidence = scenario::store(
        &roots.state_root.join("scenario-evidence"),
        scenario::run(&roots, &definition).unwrap(),
    )
    .unwrap();
    let parent = scenario::ParentObligation {
        id: "PARENT".into(),
        claim: "bounded aggregate".into(),
        authority: "all required children".into(),
        required_children: vec![scenario::RequiredChild {
            obligation_id: "SCENARIO:SYNTHETIC".into(),
            evidence_kind: scenario::ChildEvidenceKind::Scenario,
        }],
        source: roots.specification_root.join("parents.json"),
    };
    let edges = scenario::graph_edges(&roots, &[definition], &[parent]).unwrap();
    assert!(
        edges
            .iter()
            .any(|edge| edge.kind == project::LinkKind::RelevantTo)
    );
    assert!(edges.iter().any(|edge| {
        edge.kind == project::LinkKind::EvidenceFor
            && edge.to == project::GraphNode::Evidence(evidence.id.clone())
    }));
    assert!(
        edges
            .iter()
            .any(|edge| edge.kind == project::LinkKind::Requires)
    );
}

#[test]
fn scenario_graph_keeps_missing_historical_artifact_as_an_identity() {
    let (roots, definition) = scenario_fixture(r#"{"postconditions":{"work_recoverable":true}}"#);
    let mut historical = definition.clone();
    historical.id = "HISTORICAL".into();
    historical.inputs = vec![scenario::ScenarioInput {
        root: scenario::InputRoot::Project,
        path: "removed-historical-runner.rs".into(),
    }];

    let edges = scenario::graph_edges(&roots, &[historical], &[]).unwrap();
    assert!(edges.iter().any(|edge| {
        edge.kind == project::LinkKind::RelevantTo
            && edge.from
                == project::GraphNode::Artifact(project::ArtifactId(
                    "project:removed-historical-runner.rs".into(),
                ))
    }));
}

fn quint_fixture(
    checker_output: &str,
    backend: quint::ModelCheckerBackend,
) -> (
    roots::VerificationRoots,
    quint::ModelCheckDefinition,
    PathBuf,
) {
    use std::os::unix::fs::PermissionsExt;

    let project = dir();
    let specification = dir();
    let state = dir();
    fs::create_dir_all(specification.join("models/checks")).unwrap();
    fs::write(
        specification.join("models/pipeline.qnt"),
        "module pipeline { var x: int action init = x' = 0 action step = x' = x val safe = x == 0 }\n",
    )
    .unwrap();
    let source = specification.join("models/checks/safe.json");
    fs::write(&source, "{}\n").unwrap();
    let script = state.join("fake-quint");
    fs::write(
        &script,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf '%s\\n' '0.32.0'\nelse\n  printf '%s\\n' '{}'\nfi\n",
            checker_output.replace("'", "'\\''")
        ),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let (backend_version, max_steps) = match backend {
        quint::ModelCheckerBackend::Tlc => ("2.19", None),
        quint::ModelCheckerBackend::Apalache => ("0.56.1", Some(20)),
    };
    let definition = quint::ModelCheckDefinition {
        id: "SAFE-001".into(),
        model_id: "PIPELINE".into(),
        model: "models/pipeline.qnt".into(),
        property_id: "WatermarkNeverMovesBackward".into(),
        property_name: "safe".into(),
        property_kind: quint::ModelPropertyKind::Invariant,
        expected_outcome: quint::ExpectedModelCheckerOutcome::NoCounterexample,
        backend,
        quint_version: "0.32.0".into(),
        backend_version: backend_version.into(),
        main: None,
        init: "init".into(),
        step: "step".into(),
        constants: BTreeMap::from([("EVENTS".into(), serde_json::json!([1]))]),
        bounds: BTreeMap::from([("workers".into(), serde_json::json!(2))]),
        model_bindings: BTreeMap::new(),
        fairness: Vec::new(),
        max_steps,
        timeout_ms: 1_000,
        semantic_flags: Vec::new(),
        authority: quint::FormalModelAuthority {
            claim: "property holds in the checked formal model".into(),
            scope: "configured finite state space".into(),
            does_not_prove: vec!["implementation conformance".into()],
        },
        source,
    };
    (
        roots::VerificationRoots::explicit(&project, &specification, &state),
        definition,
        script,
    )
}

#[test]
fn quint_tlc_pass_is_exhaustive_formal_model_only_and_records_stats() {
    let output = "# APALACHE version: 0.56.1 | build: test\nTLC2 Version 2.19 of 08 August 2024\n3 states generated, 3 distinct states found, 0 states left on queue.\nThe depth of the complete state graph search is 3.\n[ok] No violation found (10ms).";
    let (roots, definition, executable) = quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(
        evidence.exploration,
        quint::ExplorationSemantics::ExhaustiveFinite
    );
    assert_eq!(evidence.explored_state_stats.distinct_states, Some(3));
    assert_eq!(evidence.explored_state_stats.depth, Some(3));
    assert!(
        evidence
            .authority
            .does_not_prove
            .contains(&"implementation conformance".into())
    );
}

#[test]
fn quint_invariant_violation_is_fail_with_immutable_counterexample() {
    let output = "TLC2 Version 2.19 of 08 August 2024\nState 1: <Initial predicate>\nx = 0\nState 2: <step>\nx = 1\n[violation] Found an issue (10ms).";
    let (roots, definition, executable) = quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Fail
    );
    assert!(
        evidence
            .counterexample
            .iter()
            .any(|line| line.contains("State 2"))
    );
    let stored = quint::store(&roots.state_root.join("model-evidence"), evidence).unwrap();
    assert_eq!(
        quint::load_all(&roots.state_root.join("model-evidence"))
            .unwrap()
            .first()
            .unwrap()
            .counterexample,
        stored.counterexample
    );
}

#[test]
fn quint_behavior_admission_requires_a_counterexample_witness() {
    let output = "TLC2 Version 2.19 of 08 August 2024\nState 1: Fetch(A)\nState 2: Fetch(B)\nState 3: Submit(A)\nState 4: Submit(B)\n[violation] Found an issue (10ms).";
    let (roots, mut definition, executable) =
        quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    definition.expected_outcome = quint::ExpectedModelCheckerOutcome::CounterexampleRequired;
    definition.property_id = "S8TraceAdmitted".into();
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert!(evidence.diagnostics[0].contains("admits"));

    let no_witness = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found (10ms).";
    let (roots, mut definition, executable) =
        quint_fixture(no_witness, quint::ModelCheckerBackend::Tlc);
    definition.expected_outcome = quint::ExpectedModelCheckerOutcome::CounterexampleRequired;
    assert_eq!(
        quint::run_with_executable(&roots, &definition, &executable)
            .unwrap()
            .result_at_execution,
        evidence::VerificationStatus::Fail
    );
}

#[test]
fn quint_apalache_pass_is_bounded_and_never_universal() {
    let output = "# APALACHE version: 0.56.1 | build: test\n[ok] No violation found (10ms).";
    let (roots, definition, executable) =
        quint_fixture(output, quint::ModelCheckerBackend::Apalache);
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(
        evidence.exploration,
        quint::ExplorationSemantics::Bounded { max_steps: 20 }
    );
    assert!(evidence.diagnostics[0].contains("within 20 steps"));
}

#[test]
fn quint_temporal_evidence_records_fairness_and_rejects_apalache_authority() {
    let output = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found (10ms).";
    let (roots, mut definition, executable) =
        quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    definition.property_kind = quint::ModelPropertyKind::Temporal;
    definition.fairness = vec!["weak fairness of sweeper".into()];
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(evidence.fairness, vec!["weak fairness of sweeper"]);

    definition.backend = quint::ModelCheckerBackend::Apalache;
    definition.backend_version = "0.56.1".into();
    definition.max_steps = Some(10);
    let unsupported = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        unsupported.result_at_execution,
        evidence::VerificationStatus::Unverified
    );
    assert_eq!(
        unsupported.completion,
        quint::CompletionSemantics::Unsupported
    );
}

#[test]
fn quint_tool_version_mismatch_is_error() {
    let output = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found.";
    let (roots, mut definition, executable) =
        quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    definition.quint_version = "9.9.9".into();
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Error
    );
    assert_eq!(
        evidence.completion,
        quint::CompletionSemantics::InfrastructureError
    );
}

#[test]
fn quint_timeout_and_false_zero_exit_are_never_pass() {
    use std::os::unix::fs::PermissionsExt;

    let (roots, mut definition, executable) =
        quint_fixture("server crashed", quint::ModelCheckerBackend::Tlc);
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Error
    );
    assert_eq!(
        evidence.completion,
        quint::CompletionSemantics::InfrastructureError
    );

    fs::write(&executable, "#!/bin/sh\nsleep 1\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    definition.timeout_ms = 10;
    let timeout = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        timeout.result_at_execution,
        evidence::VerificationStatus::Error
    );
    assert_eq!(
        timeout.completion,
        quint::CompletionSemantics::IncompleteTimeout
    );
}

#[test]
fn quint_success_marker_without_backend_identity_is_not_pass() {
    let (roots, definition, executable) =
        quint_fixture("[ok] No violation found.", quint::ModelCheckerBackend::Tlc);
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Error
    );
    assert_eq!(
        evidence.completion,
        quint::CompletionSemantics::InfrastructureError
    );
    assert_eq!(evidence.backend_version, "undetected");
}

#[test]
fn quint_model_bindings_machine_check_declared_constants_and_bounds() {
    let output = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found.";
    let (roots, mut definition, executable) =
        quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    fs::write(
        roots.specification_root.join("models/pipeline.qnt"),
        "module pipeline {\n  pure val ADRPROOF_EVENT_IDS = Set(1)\n  pure val ADRPROOF_WORKERS = 2\n  var x: int\n  action init = x' = 0\n  action step = x' = x\n  val safe = x == 0\n}\n",
    )
    .unwrap();
    definition.model_bindings = BTreeMap::from([
        ("constants.EVENTS".into(), "ADRPROOF_EVENT_IDS".into()),
        ("bounds.workers".into(), "ADRPROOF_WORKERS".into()),
    ]);

    let pass = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(pass.result_at_execution, evidence::VerificationStatus::Pass);
    assert_eq!(pass.model_bindings, definition.model_bindings);

    definition
        .bounds
        .insert("workers".into(), serde_json::json!(3));
    let mismatch = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        mismatch.result_at_execution,
        evidence::VerificationStatus::Error
    );
    assert_eq!(
        mismatch.completion,
        quint::CompletionSemantics::InfrastructureError
    );
    assert!(
        mismatch
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.contains("model binding mismatch"))
    );
}

#[test]
fn quint_semantic_inputs_versions_and_bounds_control_staleness_not_locations() {
    let output = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found (10ms).";
    let (roots, mut definition, executable) =
        quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    let evidence = quint::run_with_executable(&roots, &definition, &executable).unwrap();
    assert_eq!(
        quint::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Current
    );

    fs::write(
        roots.specification_root.join("models/pipeline.qnt"),
        "module pipeline { var x: int action init = x' = 1 action step = x' = x val safe = x == 0 }\n",
    )
    .unwrap();
    assert_eq!(
        quint::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    fs::write(
        roots.specification_root.join("models/pipeline.qnt"),
        "module pipeline { var x: int action init = x' = 0 action step = x' = x val safe = x == 0 }\n",
    )
    .unwrap();
    definition.property_name = "changed".into();
    assert_eq!(
        quint::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    definition.property_name = "safe".into();
    definition
        .bounds
        .insert("workers".into(), serde_json::json!(3));
    assert_eq!(
        quint::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    definition
        .bounds
        .insert("workers".into(), serde_json::json!(2));
    definition.backend_version = "2.20".into();
    assert_eq!(
        quint::assess(&roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    definition.backend_version = "2.19".into();

    let relocated_spec = dir();
    fs::create_dir_all(relocated_spec.join("models/checks")).unwrap();
    fs::copy(
        roots.specification_root.join("models/pipeline.qnt"),
        relocated_spec.join("models/pipeline.qnt"),
    )
    .unwrap();
    fs::copy(
        &definition.source,
        relocated_spec.join("models/checks/safe.json"),
    )
    .unwrap();
    definition.source = relocated_spec.join("models/checks/safe.json");
    let relocated_roots =
        roots::VerificationRoots::explicit(&roots.project_root, &relocated_spec, &roots.state_root);
    assert_eq!(
        quint::assess(&relocated_roots, &definition, &evidence).unwrap(),
        evidence::EvidenceValidity::Current
    );
}

#[test]
fn quint_evidence_history_is_immutable() {
    let output = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found.";
    let (roots, definition, executable) = quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    let directory = roots.state_root.join("model-evidence");
    let first = quint::store(
        &directory,
        quint::run_with_executable(&roots, &definition, &executable).unwrap(),
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let second = quint::store(
        &directory,
        quint::run_with_executable(&roots, &definition, &executable).unwrap(),
    )
    .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(quint::load_all(&directory).unwrap().len(), 2);
}

#[test]
fn quint_graph_connects_model_artifact_obligation_and_evidence() {
    let output = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found.";
    let (roots, definition, executable) = quint_fixture(output, quint::ModelCheckerBackend::Tlc);
    let evidence = quint::store(
        &roots.state_root.join("model-evidence"),
        quint::run_with_executable(&roots, &definition, &executable).unwrap(),
    )
    .unwrap();
    let edges = quint::graph_edges(&roots, &[definition], &[]).unwrap();
    assert!(
        edges
            .iter()
            .any(|edge| edge.kind == project::LinkKind::Defines)
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge.kind == project::LinkKind::RelevantTo)
    );
    assert!(edges.iter().any(|edge| {
        edge.kind == project::LinkKind::EvidenceFor
            && edge.to == project::GraphNode::Evidence(evidence.id.clone())
    }));
}

#[test]
fn quint_model_falsification_pass_stale_fail_restore_pass() {
    use std::os::unix::fs::PermissionsExt;

    let (roots, definition, executable) = quint_fixture("", quint::ModelCheckerBackend::Tlc);
    fs::write(
        &executable,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf '0.32.0\\n'\nelif grep -q BROKEN \"$2\"; then\n  printf 'TLC2 Version 2.19 of 08 August 2024\\n[violation] Found an issue.\\n'\nelse\n  printf 'TLC2 Version 2.19 of 08 August 2024\\n[ok] No violation found.\\n'\nfi\n",
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let directory = roots.state_root.join("model-evidence");
    let model = roots.specification_root.join("models/pipeline.qnt");
    let original = fs::read_to_string(&model).unwrap();
    let pass = quint::store(
        &directory,
        quint::run_with_executable(&roots, &definition, &executable).unwrap(),
    )
    .unwrap();
    assert_eq!(pass.result_at_execution, evidence::VerificationStatus::Pass);

    fs::write(&model, format!("{original}\n// BROKEN\n")).unwrap();
    assert_eq!(
        quint::assess(&roots, &definition, &pass).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    let fail = quint::store(
        &directory,
        quint::run_with_executable(&roots, &definition, &executable).unwrap(),
    )
    .unwrap();
    assert_eq!(fail.result_at_execution, evidence::VerificationStatus::Fail);

    fs::write(&model, original).unwrap();
    assert_eq!(
        quint::assess(&roots, &definition, &fail).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    let restored = quint::store(
        &directory,
        quint::run_with_executable(&roots, &definition, &executable).unwrap(),
    )
    .unwrap();
    assert_eq!(
        restored.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(
        quint::assess(&roots, &definition, &restored).unwrap(),
        evidence::EvidenceValidity::Current
    );
    assert_eq!(quint::load_all(&directory).unwrap().len(), 3);
}

fn correspondence_fixture() -> (
    roots::VerificationRoots,
    correspondence::CorrespondenceDefinition,
) {
    let project = dir();
    let specification = dir();
    let state = dir();
    fs::create_dir_all(project.join("src")).unwrap();
    fs::create_dir_all(specification.join("correspondence/checks")).unwrap();
    fs::create_dir_all(specification.join("models")).unwrap();
    fs::write(
        project.join("src/pipeline.rs"),
        "fn pipeline() { fetch_pending(); submit(); begin(); record_task(); mark_processed(); commit(); let _ = \"UPDATE ledger\"; }\n",
    )
    .unwrap();
    fs::write(
        specification.join("models/pipeline.qnt"),
        "module pipeline {\n  action fetch = true\n  action submit = true\n  action persist = true\n}\n",
    )
    .unwrap();
    let source = specification.join("correspondence/checks/pipeline.json");
    let definition = correspondence::CorrespondenceDefinition {
        id: "PIPELINE-001".into(),
        claim: "selected syntax corresponds to selected model actions".into(),
        authority: "AST syntax only".into(),
        does_not_prove: vec!["semantic refinement".into()],
        model: "models/pipeline.qnt".into(),
        transitions: vec![correspondence::TransitionCorrespondence {
            id: "PERSIST".into(),
            rust: correspondence::RustFunctionSelector {
                file: "src/pipeline.rs".into(),
                function: "pipeline".into(),
            },
            model_actions: vec!["fetch".into(), "submit".into(), "persist".into()],
            required_calls: vec!["mark_processed".into(), "commit".into()],
            ordered_calls: vec![
                "fetch_pending".into(),
                "submit".into(),
                "begin".into(),
                "record_task".into(),
                "mark_processed".into(),
                "commit".into(),
            ],
            required_string_fragments: vec!["UPDATE ledger".into()],
            required_syntax_fragments: vec!["mark_processed()".into()],
            authority: "named calls in one parsed function".into(),
            does_not_prove: vec!["resolved call targets".into()],
        }],
        source: source.clone(),
    };
    fs::write(&source, serde_json::to_vec_pretty(&definition).unwrap()).unwrap();
    (
        roots::VerificationRoots::explicit(&project, &specification, &state),
        definition,
    )
}

#[test]
fn rust_quint_correspondence_pass_stale_fail_restore_pass_is_immutable() {
    let (roots, definition) = correspondence_fixture();
    let directory = roots.state_root.join("correspondence-evidence");
    let first = correspondence::store(
        &directory,
        correspondence::run(&roots, &definition).unwrap(),
    )
    .unwrap();
    assert_eq!(
        first.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(
        correspondence::assess(&roots, &definition, &first).unwrap(),
        evidence::EvidenceValidity::Current
    );

    fs::write(
        roots.project_root.join("src/pipeline.rs"),
        "fn pipeline() { fetch_pending(); submit(); begin(); record_task(); commit(); let _ = \"UPDATE ledger\"; }\n",
    )
    .unwrap();
    assert_eq!(
        correspondence::assess(&roots, &definition, &first).unwrap(),
        evidence::EvidenceValidity::Stale
    );
    let failing = correspondence::store(
        &directory,
        correspondence::run(&roots, &definition).unwrap(),
    )
    .unwrap();
    assert_eq!(
        failing.result_at_execution,
        evidence::VerificationStatus::Fail
    );

    fs::write(
        roots.project_root.join("src/pipeline.rs"),
        "fn pipeline() { fetch_pending(); submit(); begin(); record_task(); mark_processed(); commit(); let _ = \"UPDATE ledger\"; }\n",
    )
    .unwrap();
    let restored = correspondence::store(
        &directory,
        correspondence::run(&roots, &definition).unwrap(),
    )
    .unwrap();
    assert_eq!(
        restored.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_ne!(first.id, failing.id);
    assert_ne!(failing.id, restored.id);
    assert_eq!(correspondence::load_all(&directory).unwrap().len(), 3);
}

#[test]
fn rust_quint_correspondence_parse_error_is_error_and_graph_is_typed() {
    let (roots, definition) = correspondence_fixture();
    fs::write(roots.project_root.join("src/pipeline.rs"), "fn broken(").unwrap();
    let evidence = correspondence::store(
        &roots.state_root.join("correspondence-evidence"),
        correspondence::run(&roots, &definition).unwrap(),
    )
    .unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Error
    );
    let edges = correspondence::graph_edges(&roots, &[definition]).unwrap();
    assert!(
        edges
            .iter()
            .any(|edge| edge.kind == project::LinkKind::Defines)
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge.kind == project::LinkKind::RelevantTo)
    );
    assert!(
        edges
            .iter()
            .any(|edge| edge.kind == project::LinkKind::EvidenceFor)
    );
}

#[test]
fn stale_correspondence_child_blocks_parent_pass() {
    let parent = scenario::ParentObligation {
        id: "PARENT".into(),
        claim: "implementation and model correspond".into(),
        authority: "all required evidence".into(),
        required_children: Vec::new(),
        source: PathBuf::new(),
    };
    let assessment = scenario::aggregate(
        &parent,
        vec![scenario::ChildStatus {
            obligation_id: "CORRESPONDENCE:PIPELINE-001".into(),
            status: evidence::VerificationStatus::Pass,
            validity: Some(evidence::EvidenceValidity::Stale),
            evidence_id: None,
        }],
    );
    assert_eq!(assessment.status, evidence::VerificationStatus::Stale);
}

fn cross_validation_fixture(
    model_output: &str,
) -> (
    roots::VerificationRoots,
    quint::ModelValidationDefinition,
    PathBuf,
) {
    use std::os::unix::fs::PermissionsExt;

    let scenario_output =
        r#"{"postconditions":{"work_recoverable":true},"trace":["Fetch(A)","Timeout"]}"#;
    let (roots, mut scenario_definition) = scenario_fixture(scenario_output);
    let placeholder_source = scenario_definition.source.clone();
    scenario_definition.id = "S1".into();
    scenario_definition.source = roots.specification_root.join("scenarios/S1.json");
    fs::remove_file(placeholder_source).unwrap();
    fs::create_dir_all(scenario_definition.source.parent().unwrap()).unwrap();
    fs::write(
        &scenario_definition.source,
        serde_json::to_vec_pretty(&scenario_definition).unwrap(),
    )
    .unwrap();
    scenario::store(
        &roots.state_root.join("scenario-evidence"),
        scenario::run(&roots, &scenario_definition).unwrap(),
    )
    .unwrap();

    fs::create_dir_all(roots.specification_root.join("models/checks")).unwrap();
    fs::write(
        roots.specification_root.join("models/pipeline.qnt"),
        "module pipeline { var x: int action init = x' = 0 action step = x' = 1 val witness = x == 0 }\n",
    )
    .unwrap();
    let model_source = roots
        .specification_root
        .join("models/checks/S1-witness.json");
    let model_definition = quint::ModelCheckDefinition {
        id: "ADMITS-S1".into(),
        model_id: "PIPELINE".into(),
        model: "models/pipeline.qnt".into(),
        property_id: "S1Admitted".into(),
        property_name: "witness".into(),
        property_kind: quint::ModelPropertyKind::Invariant,
        expected_outcome: quint::ExpectedModelCheckerOutcome::CounterexampleRequired,
        backend: quint::ModelCheckerBackend::Tlc,
        quint_version: "0.32.0".into(),
        backend_version: "2.19".into(),
        main: None,
        init: "init".into(),
        step: "step".into(),
        constants: BTreeMap::new(),
        bounds: BTreeMap::from([("workers".into(), serde_json::json!(2))]),
        model_bindings: BTreeMap::new(),
        fairness: Vec::new(),
        max_steps: None,
        timeout_ms: 1_000,
        semantic_flags: Vec::new(),
        authority: quint::FormalModelAuthority {
            claim: "S1 trace is reachable".into(),
            scope: "finite model".into(),
            does_not_prove: vec!["implementation refinement".into()],
        },
        source: model_source.clone(),
    };
    fs::write(
        &model_source,
        serde_json::to_vec_pretty(&model_definition).unwrap(),
    )
    .unwrap();
    let executable = roots.state_root.join("fake-quint-model");
    fs::write(
        &executable,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf '0.32.0\\n'; else printf '%s\\n' '{}'; fi\n",
            model_output.replace("'", "'\\''")
        ),
    )
    .unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    quint::store(
        &roots.state_root.join("model-evidence"),
        quint::run_with_executable(&roots, &model_definition, &executable).unwrap(),
    )
    .unwrap();

    let validation_source = roots
        .specification_root
        .join("models/scenario-validation.json");
    let validation = quint::ModelValidationDefinition {
        id: "VALIDATE-S1".into(),
        claim: "observed S1 is admitted".into(),
        authority: "selected trace cross-validation".into(),
        does_not_prove: vec!["refinement".into()],
        mappings: vec![quint::ScenarioModelMapping {
            scenario_id: "S1".into(),
            expected_scenario_result: evidence::VerificationStatus::Pass,
            model_check_id: "ADMITS-S1".into(),
            trace_pattern: vec!["Fetch(A)".into(), "Timeout".into()],
        }],
        source: validation_source.clone(),
    };
    fs::write(
        validation_source,
        serde_json::to_vec_pretty(&vec![validation.clone()]).unwrap(),
    )
    .unwrap();
    (roots, validation, executable)
}

#[test]
fn scenario_model_cross_validation_is_separate_immutable_evidence() {
    let output = "TLC2 Version 2.19 of 08 August 2024\nState 1: Fetch(A)\nState 2: Timeout\n[violation] Found an issue.";
    let (roots, validation, _) = cross_validation_fixture(output);
    let evidence = quint::run_validation(&roots, &validation).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Pass
    );
    assert_eq!(
        evidence.mappings[0].status,
        evidence::VerificationStatus::Pass
    );
    assert!(evidence.does_not_prove.contains(&"refinement".into()));
    let stored = quint::store_validation(
        &roots.state_root.join("model-validation-evidence"),
        evidence,
    )
    .unwrap();
    assert!(stored.id.0.starts_with("MODEL-VALIDATION-EVIDENCE:"));

    fs::write(
        roots.project_root.join("implementation.rs"),
        "fn implementation_changed() {}\n",
    )
    .unwrap();
    assert_eq!(
        quint::assess_validation(&roots, &validation, &stored).unwrap(),
        evidence::EvidenceValidity::Stale
    );
}

#[test]
fn scenario_model_mismatch_is_validation_fail_not_model_property_pass() {
    let output = "TLC2 Version 2.19 of 08 August 2024\n[ok] No violation found.";
    let (roots, validation, _) = cross_validation_fixture(output);
    let evidence = quint::run_validation(&roots, &validation).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Fail
    );
    assert_eq!(
        evidence.mappings[0].model_result,
        Some(evidence::VerificationStatus::Fail)
    );
}

#[test]
fn scenario_model_validation_rejects_a_missing_observed_trace_event() {
    let output = "TLC2 Version 2.19 of 08 August 2024\nState 1: Fetch(A)\nState 2: Timeout\n[violation] Found an issue.";
    let (roots, validation, _) = cross_validation_fixture(output);
    let mut latest = scenario::load_all(&roots.state_root.join("scenario-evidence"))
        .unwrap()
        .pop()
        .unwrap();
    latest.id = project::EvidenceId("pending".into());
    latest.trace = vec!["Fetch(A)".into()];
    latest.recorded_at_unix_nanos += 1;
    scenario::store(&roots.state_root.join("scenario-evidence"), latest).unwrap();

    let evidence = quint::run_validation(&roots, &validation).unwrap();
    assert_eq!(
        evidence.result_at_execution,
        evidence::VerificationStatus::Fail
    );
    assert!(!evidence.mappings[0].scenario_trace_matches);
    assert_eq!(evidence.mappings[0].missing_trace_events, vec!["Timeout"]);
}
