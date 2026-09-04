// Execution fixtures below require a POSIX shell and Unix executable permissions.
use super::*;

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
