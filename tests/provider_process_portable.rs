use adrproof::Error;
use adrproof::external_provider::{
    DIAGNOSTIC_OUTPUT_LIMIT, DIAGNOSTIC_RESPONSE, DIAGNOSTIC_TIMEOUT, run_selected,
};
use adrproof::roots::VerificationRoots;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn native_provider_process_contract_is_portable() {
    let root = unique_root();
    let project = root.join("project");
    let specification = root.join("spec");
    let state = root.join("state");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&specification).unwrap();
    fs::write(project.join("input.txt"), "component=api\n").unwrap();

    let executable =
        specification.join(format!("portable-provider{}", std::env::consts::EXE_SUFFIX));
    let source =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/portable_provider.rs");
    let compilation = Command::new("rustc")
        .arg(&source)
        .arg("--edition=2024")
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compilation.status.success(),
        "portable provider compilation failed: {}",
        String::from_utf8_lossy(&compilation.stderr)
    );

    let roots = VerificationRoots::explicit(&project, &specification, &state);

    configure(&specification, &executable, "valid", 2_000);
    let runs = run_selected(&roots, Some("portable-fixture")).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].facts.len(), 1);
    assert_eq!(runs[0].facts[0].relation, "component");
    assert_eq!(runs[0].facts[0].arguments, ["api"]);
    assert_eq!(
        runs[0]
            .inputs
            .iter()
            .map(|input| input.identity.as_str())
            .collect::<Vec<_>>(),
        vec![
            "project:input.txt",
            "spec:adrproof.json",
            &format!("spec:portable-provider{}", std::env::consts::EXE_SUFFIX)
        ]
    );

    configure(&specification, &executable, "malformed", 2_000);
    assert_code(
        run_selected(&roots, Some("portable-fixture")).unwrap_err(),
        DIAGNOSTIC_RESPONSE,
    );

    configure(&specification, &executable, "oversized", 2_000);
    assert_code(
        run_selected(&roots, Some("portable-fixture")).unwrap_err(),
        DIAGNOSTIC_OUTPUT_LIMIT,
    );

    configure(&specification, &executable, "sleep", 100);
    assert_code(
        run_selected(&roots, Some("portable-fixture")).unwrap_err(),
        DIAGNOSTIC_TIMEOUT,
    );
}

fn configure(specification: &Path, executable: &Path, mode: &str, timeout_ms: u64) {
    let executable = executable.file_name().unwrap().to_string_lossy();
    fs::write(
        specification.join("adrproof.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "z3_version": "4.13.4",
            "external_providers": [{
                "id": "portable-fixture",
                "protocol": "adrproof-external-provider-v1",
                "version": "1.0.0",
                "executable": executable,
                "args": [mode],
                "timeout_ms": timeout_ms
            }]
        }))
        .unwrap(),
    )
    .unwrap();
}

fn assert_code(error: Error, expected: &'static str) {
    let Error::ExternalProviderFailure { code, .. } = error else {
        panic!("expected external-provider failure, got {error:?}");
    };
    assert_eq!(code, expected);
}

fn unique_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "adrproof-portable-provider-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
