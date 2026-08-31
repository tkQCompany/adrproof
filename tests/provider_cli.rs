#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_state(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "adrproof-provider-cli-{label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn adrproof() -> Command {
    Command::new(env!("CARGO_BIN_EXE_adrproof"))
}

#[test]
fn provider_check_json_success_uses_versioned_report() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example = repository.join("examples/external-provider");
    let state = unique_state("success");
    let output = adrproof()
        .args(["provider", "check", "component-manifest", "--project-root"])
        .arg(example.join("project"))
        .arg("--spec-root")
        .arg(example.join("spec"))
        .arg("--state-root")
        .arg(&state)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["schema_version"],
        "adrproof-provider-check-report-v1"
    );
    assert_eq!(report["result"], "PASS");
    assert_eq!(
        report["providers"][0]["provider"]["id"],
        "component-manifest"
    );
    assert!(
        report["providers"][0]["semantic_inputs"]
            .as_array()
            .unwrap()
            .len()
            >= 3
    );
}

#[test]
fn provider_check_json_failure_uses_code_and_exit_six() {
    let root = unique_state("failure");
    let project = root.join("project");
    let specification = root.join("spec");
    let state = root.join("state");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&specification).unwrap();
    let provider = specification.join("malformed.sh");
    fs::write(&provider, "#!/bin/sh\ncat >/dev/null\nprintf '{'\n").unwrap();
    executable(&provider);
    fs::write(
        specification.join("adrproof.json"),
        r#"{
  "z3_version": "4.13.4",
  "external_providers": [{
    "id": "malformed",
    "protocol": "adrproof-external-provider-v1",
    "version": "1.0.0",
    "executable": "malformed.sh"
  }]
}"#,
    )
    .unwrap();

    let output = adrproof()
        .args(["provider", "check", "malformed", "--project-root"])
        .arg(&project)
        .arg("--spec-root")
        .arg(&specification)
        .arg("--state-root")
        .arg(&state)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(6));
    assert!(output.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(
        report["schema_version"],
        "adrproof-provider-check-report-v1"
    );
    assert_eq!(report["result"], "ERROR");
    assert_eq!(report["exit_code"], 6);
    assert_eq!(report["diagnostics"][0]["code"], "ADRP-EXTP-300");
}

#[test]
fn provider_check_without_configuration_is_structured() {
    let root = unique_state("no-config");
    let project = root.join("project");
    let specification = root.join("spec");
    let state = root.join("state");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&specification).unwrap();

    let output = adrproof()
        .args(["provider", "check", "--project-root"])
        .arg(&project)
        .arg("--spec-root")
        .arg(&specification)
        .arg("--state-root")
        .arg(&state)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(6));
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(report["result"], "ERROR");
    assert_eq!(report["diagnostics"][0]["code"], "ADRP-EXTP-100");
}

#[test]
fn provider_check_text_summary_lists_semantic_inputs() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let example = repository.join("examples/external-provider");
    let state = unique_state("summary");
    let output = adrproof()
        .args(["provider", "check", "component-manifest", "--project-root"])
        .arg(example.join("project"))
        .arg("--spec-root")
        .arg(example.join("spec"))
        .arg("--state-root")
        .arg(&state)
        .arg("--summary")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("component-manifest: PASS"));
    assert!(stdout.contains("input: project:component.json"));
    assert!(stdout.contains("input: spec:adrproof.json"));
}

fn executable(path: &Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}
