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

#[test]
fn evidence_storage_uses_portable_filenames_without_changing_logical_ids() {
    let root = workspace(false);
    let out = root.join(".adrproof");
    run_check(&root, &FixedBackend(Verdict::Sat), &out).unwrap();
    let directory = out.join("evidence");
    let stored = evidence::latest(&directory).unwrap().unwrap();
    assert!(stored.id.0.starts_with("EVIDENCE:"));
    let files = fs::read_dir(&directory)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(files.len(), 1);
    let name = files[0].file_name().into_string().unwrap();
    assert!(name.ends_with(".json"));
    assert!(
        !name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c)),
        "non-portable filename: {name}"
    );
    #[cfg(unix)]
    {
        // Old Unix evidence files remain readable without a migration or rewrite.
        fs::rename(
            files[0].path(),
            directory.join(format!("{}.json", stored.id.0)),
        )
        .unwrap();
        assert_eq!(evidence::latest(&directory).unwrap().unwrap(), stored);
    }
}

#[cfg(unix)]
#[test]
fn canonical_cargo_paths_under_an_aliased_root_remain_relevant_inputs() {
    let physical = workspace(false);
    let alias_parent = dir();
    let alias = alias_parent.join("project");
    std::os::unix::fs::symlink(&physical, &alias).unwrap();
    let state = dir();
    let roots = roots::VerificationRoots::explicit(&alias, &alias, &state);
    let manifest = fs::canonicalize(physical.join("domain/Cargo.toml")).unwrap();
    assert_eq!(
        roots.project_identity(&manifest),
        "project:domain/Cargo.toml"
    );
    assert_eq!(roots.spec_identity(&manifest), "spec:domain/Cargo.toml");
    run_check_with_roots(&roots, &FixedBackend(Verdict::Sat)).unwrap();
    let stored = evidence::latest(&state.join("evidence")).unwrap().unwrap();
    assert!(
        stored
            .inputs
            .iter()
            .any(|i| i.source == "project:domain/Cargo.toml")
    );
    let mut changed = fs::read_to_string(&manifest).unwrap();
    changed.push_str("\n# relevant aliased input changed\n");
    fs::write(&manifest, changed).unwrap();
    assert_eq!(
        current_evidence_status_with_roots(&roots, &state.join("evidence"), "Z3 4.13.4", 10_000)
            .unwrap(),
        evidence::VerificationStatus::Stale
    );
    run_check_with_roots(&roots, &FixedBackend(Verdict::Sat)).unwrap();
    assert_eq!(
        current_evidence_status_with_roots(&roots, &state.join("evidence"), "Z3 4.13.4", 10_000)
            .unwrap(),
        evidence::VerificationStatus::Pass
    );
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

// Core regressions above run everywhere; these execution fixtures require Unix.
#[cfg(unix)]
mod posix_execution;
