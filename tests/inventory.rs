use adrproof::inventory::{INPUT_SCHEMA, InventoryReport, REPORT_SCHEMA, inspect};
use adrproof::project::{ConstraintId, DecisionId, GraphNode, LinkKind, RequirementId};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);
const ADR: &str = "---\nid: ADR-1\nstatus: accepted\n---\n\nA boundary is required.\nRecovery is required.\nThis reduces coupling.\n\n```adrlogic\nbool boundary;\nrule C1 \"boundary\" { boundary; }\nrule C2 \"second check\" { boundary; }\n```\n";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "adrproof-inventory-{}-{}-{time}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let fixture = Self(root);
        fixture.write("architecture.md", ADR);
        fixture.manifest(&declaration());
        fixture
    }
    fn write(&self, path: &str, text: &str) {
        fs::write(self.0.join(path), text).unwrap();
    }
    fn manifest(&self, value: &Value) {
        self.write(
            "requirements.json",
            &serde_json::to_string_pretty(value).unwrap(),
        );
    }
    fn report(&self) -> InventoryReport {
        inspect(&self.0).unwrap()
    }
    fn cli(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_adrproof"))
            .arg("inventory")
            .arg("--spec-root")
            .arg(&self.0)
            .args(arguments)
            .env("ADRPROOF_Z3", "nonexistent-solver")
            .env("PATH", "")
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn declaration() -> Value {
    json!({"schema_version": INPUT_SCHEMA, "adrs": [{"id":"ADR-1", "source":"architecture.md", "requirements": [
        {"id":"REQ-1","kind":"normative","start_line":6,"end_line":6,"mapping":"mapped","constraints":["ADR-1:C1","ADR-1:C2"]},
        {"id":"REQ-2","kind":"normative","start_line":7,"end_line":7,"mapping":"mapped","constraints":["ADR-1:C2"]},
        {"id":"WHY-1","kind":"rationale","start_line":8,"end_line":8,"mapping":"unmapped","constraints":[],"reason":"Motivation only"}
    ]}]})
}
fn codes(report: &InventoryReport) -> Vec<&str> {
    report.gaps.iter().map(|gap| gap.code.as_str()).collect()
}

#[test]
fn many_to_many_declared_links_reuse_the_intent_graph_without_proof_authority() {
    let f = Fixture::new();
    let report = f.report();
    assert_eq!(report.result, "RECORDED_UNREVIEWED");
    assert_eq!(report.exit_code(), 0);
    assert_eq!(report.review_status, "NOT_ASSESSED");
    assert_eq!(report.verification_status, "NOT_RUN");
    assert_eq!(report.model.requirements.len(), 3);
    let edges: Vec<_> = report
        .model
        .edges
        .iter()
        .filter(|edge| edge.kind == LinkKind::DeclaredFormalization)
        .collect();
    assert_eq!(edges.len(), 3);
    assert!(edges.iter().any(|edge| edge.from
        == GraphNode::Requirement(RequirementId("REQ-2".into()))
        && edge.to == GraphNode::Constraint(ConstraintId("ADR-1:C2".into()))));
    assert!(report.model.facts.is_empty());
    assert!(report.model.fact_coverage.is_empty());
    assert!(
        !report
            .model
            .edges
            .iter()
            .any(|edge| edge.kind == LinkKind::VerifiedBy)
    );
}

#[test]
fn prose_only_adr_keeps_provenance_and_reports_unmapped_requirements() {
    let f = Fixture::new();
    f.write("architecture.md", ADR.split("```adrlogic").next().unwrap());
    let mut value = declaration();
    for req in value["adrs"][0]["requirements"].as_array_mut().unwrap() {
        req["mapping"] = json!("unmapped");
        req["constraints"] = json!([]);
    }
    f.manifest(&value);
    let report = f.report();
    assert_eq!(
        codes(&report),
        ["unmapped_requirement", "unmapped_requirement"]
    );
    assert_eq!(
        report.model.decisions[&DecisionId("ADR-1".into())]
            .provenance
            .source,
        PathBuf::from("spec:architecture.md")
    );
    let adrs = adrproof::load_adrs(&f.0).unwrap();
    let model =
        adrproof::lower_to_project_model(&adrs, &adrproof::effective(&adrs, false).unwrap());
    assert_eq!(
        model.decisions[&DecisionId("ADR-1".into())]
            .provenance
            .source,
        f.0.join("architecture.md")
    );
}

#[test]
fn deleted_adr_is_not_silently_removed_from_the_required_set() {
    let f = Fixture::new();
    fs::remove_file(f.0.join("architecture.md")).unwrap();
    let report = f.report();
    assert_eq!(report.result, "INCOMPLETE");
    assert_eq!(codes(&report), ["missing_adr"]);
    assert_eq!(report.model.requirements.len(), 3);
    assert!(!report.decisions[0].present);
    assert_eq!(report.inputs.len(), 1);
}

#[test]
fn discovered_but_uninventoried_adr_is_visible_even_if_proposed() {
    let f = Fixture::new();
    f.write(
        "other.md",
        "---\nid: ADR-2\nstatus: proposed\n---\nA proposed requirement.\n",
    );
    let report = f.report();
    assert_eq!(codes(&report), ["uninventoried_adr"]);
    assert_eq!(
        report.decisions[1].inactive_reason.as_deref(),
        Some("proposed")
    );
}

#[test]
fn moved_or_misidentified_sources_and_deleted_constraints_are_gaps() {
    let f = Fixture::new();
    let mut value = declaration();
    value["adrs"][0]["source"] = json!("missing.md");
    f.manifest(&value);
    assert_eq!(
        codes(&f.report()),
        ["adr_source_mismatch", "uninventoried_adr"]
    );
    f.manifest(&declaration());
    f.write(
        "architecture.md",
        &ADR.replace("rule C2 \"second check\" { boundary; }\n", ""),
    );
    let report = f.report();
    assert_eq!(
        codes(&report),
        ["missing_active_constraint", "missing_active_constraint"]
    );
    assert!(
        report
            .gaps
            .iter()
            .all(|gap| gap.target.as_deref() == Some("ADR-1:C2"))
    );
}

#[test]
fn partial_and_empty_normative_inventories_cannot_look_complete() {
    let f = Fixture::new();
    let mut value = declaration();
    value["adrs"][0]["requirements"][0]["mapping"] = json!("partial");
    value["adrs"][0]["requirements"][0]["reason"] = json!("Recovery remains unformalized");
    f.manifest(&value);
    assert_eq!(codes(&f.report()), ["partial_mapping"]);
    value["adrs"][0]["requirements"] = json!([]);
    f.manifest(&value);
    assert_eq!(codes(&f.report()), ["no_normative_requirements"]);
    value["adrs"][0]["requirements"] = json!([declaration()["adrs"][0]["requirements"][2].clone()]);
    f.manifest(&value);
    assert_eq!(codes(&f.report()), ["no_normative_requirements"]);
}

#[test]
fn historical_lifecycle_is_visible_and_not_misrepresented_as_active_coverage() {
    let f = Fixture::new();
    let mut value = declaration();
    for (id, status) in [
        ("OLD", "accepted"),
        ("PROPOSED", "proposed"),
        ("DEPRECATED", "deprecated"),
        ("RETIRED", "superseded"),
    ] {
        let file = format!("{id}.md");
        f.write(
            &file,
            &format!("---\nid: {id}\nstatus: {status}\n---\nHistorical requirement.\n"),
        );
        value["adrs"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id":id,"source":file,"requirements":[]}));
    }
    f.write(
        "architecture.md",
        &ADR.replace(
            "status: accepted\n",
            "status: accepted\nsupersedes: [OLD]\n",
        ),
    );
    for req in value["adrs"][0]["requirements"].as_array_mut().unwrap() {
        req["start_line"] = json!(req["start_line"].as_u64().unwrap() + 1);
        req["end_line"] = json!(req["end_line"].as_u64().unwrap() + 1);
    }
    f.manifest(&value);
    let report = f.report();
    assert!(report.gaps.is_empty());
    assert_eq!(report.decisions.iter().filter(|d| d.active).count(), 1);
    assert_eq!(
        report
            .decisions
            .iter()
            .find(|d| d.id == "OLD")
            .unwrap()
            .inactive_reason
            .as_deref(),
        Some("superseded_by_accepted_adr")
    );
}

#[test]
fn cross_adr_mapping_requires_an_active_target() {
    let f = Fixture::new();
    f.write("other.md", "---\nid: ADR-2\nstatus: accepted\n---\nA shared check.\n```adrlogic\nbool other;\nrule C3 \"shared\" { other; }\n```\n");
    let mut value = declaration();
    value["adrs"][0]["requirements"][1]["constraints"] = json!(["ADR-2:C3"]);
    value["adrs"].as_array_mut().unwrap().push(json!({"id":"ADR-2","source":"other.md","requirements":[{"id":"REQ-3","kind":"normative","start_line":5,"end_line":5,"mapping":"mapped","constraints":["ADR-1:C1"]}]}));
    f.manifest(&value);
    assert_eq!(f.report().exit_code(), 0);
    let other = fs::read_to_string(f.0.join("other.md"))
        .unwrap()
        .replace("accepted", "deprecated");
    f.write("other.md", &other);
    assert_eq!(codes(&f.report()), ["missing_active_constraint"]);
}

#[test]
fn malformed_declarations_fail_closed() {
    let f = Fixture::new();
    for (pointer, replacement) in [
        ("/schema_version", json!("unknown")),
        ("/adrs", json!([])),
        ("/adrs/0/id", json!(" ")),
        ("/adrs/0/requirements/0/id", json!("REQ-2")),
        ("/adrs/0/requirements/0/constraints", json!([])),
        (
            "/adrs/0/requirements/0/constraints",
            json!(["ADR-1:C1", "ADR-1:C1"]),
        ),
        ("/adrs/0/requirements/0/mapping", json!("unmapped")),
        ("/adrs/0/requirements/0/mapping", json!("partial")),
        ("/adrs/0/requirements/0/kind", json!("rationale")),
        ("/adrs/0/requirements/2/reason", json!("")),
        ("/adrs/0/requirements/0/start_line", json!(0)),
        ("/adrs/0/requirements/0/start_line", json!(100)),
        ("/adrs/0/requirements/0/end_line", json!(100)),
        ("/adrs/0/requirements/0/end_line", json!(2)),
    ] {
        let mut value = declaration();
        *value.pointer_mut(pointer).unwrap() = replacement;
        f.manifest(&value);
        assert!(inspect(&f.0).is_err(), "accepted {value}");
    }
    for pointer in ["", "/adrs/0", "/adrs/0/requirements/0"] {
        let mut value = declaration();
        value.pointer_mut(pointer).unwrap()["unexpected"] = json!(true);
        f.manifest(&value);
        assert!(inspect(&f.0).is_err());
    }
    f.write(
        "requirements.json",
        "{\"schema_version\":\"x\",\"schema_version\":\"y\",\"adrs\":[]}",
    );
    assert!(inspect(&f.0).is_err());
}

#[test]
fn duplicate_sources_decisions_and_discovered_clause_ids_are_rejected() {
    let f = Fixture::new();
    let mut value = declaration();
    value["adrs"]
        .as_array_mut()
        .unwrap()
        .push(declaration()["adrs"][0].clone());
    f.manifest(&value);
    assert!(inspect(&f.0).is_err());
    value["adrs"][1]["id"] = json!("ADR-2");
    f.manifest(&value);
    assert!(inspect(&f.0).is_err());
    f.manifest(&declaration());
    f.write("duplicate.md", ADR);
    assert!(inspect(&f.0).is_err());
    fs::remove_file(f.0.join("duplicate.md")).unwrap();
    f.write("architecture.md", &ADR.replace("rule C2", "rule C1"));
    assert!(inspect(&f.0).is_err());
}

#[test]
fn unsafe_paths_and_empty_text_selections_are_rejected() {
    let f = Fixture::new();
    for source in [
        "../outside.md",
        "/absolute.md",
        "C:/outside.md",
        "dir\\file.md",
        "./file.md",
        "dir//file.md",
        "CON.md",
    ] {
        let mut value = declaration();
        value["adrs"][0]["source"] = json!(source);
        f.manifest(&value);
        assert!(inspect(&f.0).is_err(), "accepted {source}");
    }
    let mut value = declaration();
    value["adrs"][0]["requirements"][0]["start_line"] = json!(5);
    value["adrs"][0]["requirements"][0]["end_line"] = json!(5);
    f.manifest(&value);
    assert!(inspect(&f.0).is_err());
}

#[test]
fn snapshots_are_deterministic_relocatable_and_hash_content_not_mtime() {
    let first = Fixture::new();
    let second = Fixture::new();
    let original = first.report();
    assert_eq!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&first.report()).unwrap()
    );
    assert_eq!(
        serde_json::to_vec(&original).unwrap(),
        serde_json::to_vec(&second.report()).unwrap()
    );
    for input in &original.inputs {
        let bytes = fs::read(first.0.join(input.source.strip_prefix("spec:").unwrap())).unwrap();
        assert_eq!(
            *input,
            adrproof::evidence::fingerprint_bytes(&input.source, &bytes)
        );
    }
    let path = first.0.join("architecture.md");
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    first.write(
        "architecture.md",
        &ADR.replace(
            "A boundary is required.",
            "A stronger boundary is required.",
        ),
    );
    fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    let edited = first.report();
    assert_ne!(original.inputs, edited.inputs);
    assert_eq!(original.model.constraints, edited.model.constraints);
    assert_eq!(edited.review_status, "NOT_ASSESSED");
    first.write(
        "architecture.md",
        &ADR.replace("{ boundary; }", "{ !boundary; }"),
    );
    assert_ne!(first.report().inputs, original.inputs);
    first.write("architecture.md", ADR);
    let mut value = declaration();
    value["adrs"][0]["requirements"][0]["constraints"] = json!(["ADR-1:C1"]);
    first.manifest(&value);
    assert_ne!(first.report().inputs, original.inputs);
}

#[test]
fn cli_outcomes_are_not_proof_pass_and_inspection_does_not_run_tools_or_write_state() {
    let f = Fixture::new();
    f.write("Cargo.toml", "invalid: must not run cargo");
    f.write(
        "adrproof.json",
        "invalid: must not load provider configuration",
    );
    let before = f.report().inputs;
    let output = f.cli(&["--json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["schema_version"], REPORT_SCHEMA);
    assert_eq!(value["result"], "RECORDED_UNREVIEWED");
    let output = f.cli(&[]);
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("This is not architectural PASS")
    );
    assert!(!f.0.join(".adrproof").exists());
    assert_eq!(f.report().inputs, before);
    let mut value = declaration();
    value["adrs"][0]["requirements"][0]["mapping"] = json!("unmapped");
    value["adrs"][0]["requirements"][0]["constraints"] = json!([]);
    f.manifest(&value);
    let output = f.cli(&["--json"]);
    assert_eq!(output.status.code(), Some(3));
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"],
        "INCOMPLETE"
    );
    f.write("requirements.json", "not JSON");
    let output = f.cli(&["--json"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert_eq!(f.cli(&["--policy", "ignored.json"]).status.code(), Some(2));
    assert_eq!(f.cli(&["first", "second"]).status.code(), Some(2));
    assert_eq!(f.cli(&["extra-root"]).status.code(), Some(2));
    assert_eq!(f.cli(&["--spec-root"]).status.code(), Some(2));
    assert_eq!(f.cli(&["--spec-root", "other"]).status.code(), Some(2));
}

#[test]
fn neutral_example_keeps_the_unformalized_requirement_visible() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/requirement-inventory");
    let report = inspect(&root).unwrap();
    assert_eq!(report.exit_code(), 3);
    assert_eq!(codes(&report), ["unmapped_requirement"]);
    assert_eq!(report.gaps[0].requirement.as_deref(), Some("REQ-recovery"));
}

#[cfg(unix)]
#[test]
fn symlinked_inputs_are_rejected_but_a_root_alias_preserves_identity() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let holder = Fixture::new();
    let alias = holder.0.join("alias");
    symlink(&f.0, &alias).unwrap();
    assert_eq!(
        serde_json::to_vec(&f.report()).unwrap(),
        serde_json::to_vec(&inspect(&alias).unwrap()).unwrap()
    );
    symlink(f.0.join("architecture.md"), f.0.join("link.md")).unwrap();
    assert!(inspect(&f.0).is_err());
    fs::remove_file(f.0.join("link.md")).unwrap();
    symlink(&holder.0, f.0.join("cycle")).unwrap();
    assert!(inspect(&f.0).is_err());
}
