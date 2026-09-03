use adrproof::roots::VerificationRoots;
use adrproof::{Error, Verdict, Z3Backend, cargo_facts::CargoMetadataProvider};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const COMMANDS: &[&str] = &[
    "check",
    "facts",
    "explain",
    "impact",
    "status",
    "diagnose",
    "scenario",
    "native-test",
    "provider",
    "bundle",
    "model",
    "correspondence",
];

fn main() {
    let code = if print_requested_help() {
        0
    } else {
        match real_main() {
            Ok(c) => c,
            Err(e) => {
                let arguments = std::env::args().collect::<Vec<_>>();
                let provider_json_requested = arguments.get(1).map(String::as_str)
                    == Some("provider")
                    && arguments.iter().any(|argument| argument == "--json");
                if let Error::ExternalProviderFailure { code, message } = &e
                    && provider_json_requested
                {
                    eprintln!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "schema_version": adrproof::external_provider::CHECK_REPORT_SCHEMA_VERSION,
                        "protocol": adrproof::external_provider::PROTOCOL_VERSION,
                        "result": "ERROR",
                        "exit_code": 6,
                        "diagnostics": [{"code": code, "message": message}],
                    }))
                    .expect("external provider error report serialization")
                );
                } else {
                    eprintln!("ERROR — {e}");
                }
                match e {
                    Error::Timeout(_) => 4,
                    Error::SolverMissing(_)
                    | Error::SolverVersion { .. }
                    | Error::SolverFailure(_) => 5,
                    Error::ProviderFailure(_) | Error::ExternalProviderFailure { .. } => 6,
                    Error::Io { .. }
                    | Error::Diagnostic { .. }
                    | Error::InvalidReference { .. } => 2,
                }
            }
        }
    };
    std::process::exit(code);
}

fn print_requested_help() -> bool {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [flag, ..] if flag == "--help" || flag == "-h" => {
            print_help(None);
            true
        }
        [command, rest @ ..]
            if COMMANDS.contains(&command.as_str())
                && rest
                    .iter()
                    .any(|argument| argument == "--help" || argument == "-h") =>
        {
            print_help(Some(command));
            true
        }
        _ => false,
    }
}

fn print_help(command: Option<&str>) {
    let usage = match command {
        None => {
            println!(
                "Usage: adrproof COMMAND [OPTIONS]\n\nCommands:\n  {}\n\nRun 'adrproof COMMAND --help' for command-specific syntax.",
                COMMANDS.join("\n  ")
            );
            return;
        }
        Some("check") => {
            "adrproof check [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--policy PATH] [--sarif PATH] [--json]"
        }
        Some("facts") => {
            "adrproof facts [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--json] [--summary]"
        }
        Some("explain") => {
            "adrproof explain ID [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--json]"
        }
        Some("impact") => {
            "adrproof impact --path PATH [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--json]"
        }
        Some("status") => {
            "adrproof status [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--json]"
        }
        Some("diagnose") => {
            "adrproof diagnose [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--json]"
        }
        Some("scenario") => {
            "adrproof scenario <list|run ID|status [ID]> [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH]"
        }
        Some("native-test") => {
            "adrproof native-test <list|import ID --report PATH|status [ID]> [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH]"
        }
        Some("provider") => {
            "adrproof provider check [PROVIDER-ID] [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--json] [--summary]"
        }
        Some("bundle") => {
            "adrproof bundle <create --output PATH|verify PATH> [ROOT] [--signing-key PATH] [--public-key PATH] [--require-signature] [--json]"
        }
        Some("model") => {
            "adrproof model <list|check ID|validate [ID]|status [ID]> [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH]"
        }
        Some("correspondence") => {
            "adrproof correspondence <list|check ID|status [ID]> [ROOT] [--project-root PATH] [--spec-root PATH] [--state-root PATH]"
        }
        Some(_) => unreachable!("command list and help must stay synchronized"),
    };
    println!("Usage: {usage}");
}

#[derive(Default)]
struct Cli {
    command: String,
    project_root: Option<PathBuf>,
    spec_root: Option<PathBuf>,
    state_root: Option<PathBuf>,
    legacy_root: Option<PathBuf>,
    query_id: Option<String>,
    scenario_action: Option<String>,
    scenario_id: Option<String>,
    model_action: Option<String>,
    model_id: Option<String>,
    correspondence_action: Option<String>,
    correspondence_id: Option<String>,
    native_test_action: Option<String>,
    native_test_id: Option<String>,
    provider_action: Option<String>,
    provider_id: Option<String>,
    report_path: Option<PathBuf>,
    bundle_action: Option<String>,
    bundle_path: Option<PathBuf>,
    signing_key_path: Option<PathBuf>,
    public_key_path: Option<PathBuf>,
    require_signature: bool,
    policy_path: Option<PathBuf>,
    sarif_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
    impact_path: Option<PathBuf>,
    json: bool,
    summary: bool,
}

fn parse_cli() -> Result<Cli, Error> {
    let mut values = std::env::args().skip(1).collect::<Vec<_>>();
    let mut cli = Cli {
        command: values.first().cloned().unwrap_or_default(),
        ..Cli::default()
    };
    if !COMMANDS.contains(&cli.command.as_str()) {
        return Err(Error::ProviderFailure(
            "usage: adrproof <check|facts|explain|impact|status|diagnose|scenario|native-test|provider|bundle|model|correspondence> [list|check|run|import|create|verify|status] [ID|PATH] [--report PATH] [--output PATH] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--signing-key PATH] [--public-key PATH] [--require-signature] [--policy PATH] [--sarif PATH] [--json] [--summary]".into(),
        ));
    }
    values.remove(0);
    let mut positional = Vec::new();
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--json" => cli.json = true,
            "--summary" => cli.summary = true,
            "--require-signature" => cli.require_signature = true,
            "--project-root" | "--spec-root" | "--state-root" | "--path" | "--report"
            | "--output" | "--signing-key" | "--public-key" | "--policy" | "--sarif" => {
                let flag = values[index].clone();
                index += 1;
                let value = values
                    .get(index)
                    .ok_or_else(|| Error::ProviderFailure(format!("{flag} requires a path")))?;
                match flag.as_str() {
                    "--project-root" => cli.project_root = Some(value.into()),
                    "--spec-root" => cli.spec_root = Some(value.into()),
                    "--state-root" => cli.state_root = Some(value.into()),
                    "--report" => cli.report_path = Some(value.into()),
                    "--output" => cli.output_path = Some(value.into()),
                    "--signing-key" => cli.signing_key_path = Some(value.into()),
                    "--public-key" => cli.public_key_path = Some(value.into()),
                    "--policy" => cli.policy_path = Some(value.into()),
                    "--sarif" => cli.sarif_path = Some(value.into()),
                    _ => cli.impact_path = Some(value.into()),
                }
            }
            value if value.starts_with('-') => {
                return Err(Error::ProviderFailure(format!("unknown option `{value}`")));
            }
            value => positional.push(value.to_string()),
        }
        index += 1;
    }
    if cli.command == "scenario" {
        cli.scenario_action = positional.first().cloned();
        cli.scenario_id = positional.get(1).cloned();
        cli.legacy_root = positional.get(2).map(PathBuf::from);
    } else if cli.command == "native-test" {
        cli.native_test_action = positional.first().cloned();
        cli.native_test_id = positional.get(1).cloned();
        cli.legacy_root = positional.get(2).map(PathBuf::from);
    } else if cli.command == "provider" {
        cli.provider_action = positional.first().cloned();
        cli.provider_id = positional.get(1).cloned();
        cli.legacy_root = positional.get(2).map(PathBuf::from);
    } else if cli.command == "bundle" {
        cli.bundle_action = positional.first().cloned();
        cli.bundle_path = positional.get(1).map(PathBuf::from);
        cli.legacy_root = positional.get(2).map(PathBuf::from);
    } else if cli.command == "model" {
        cli.model_action = positional.first().cloned();
        cli.model_id = positional.get(1).cloned();
        cli.legacy_root = positional.get(2).map(PathBuf::from);
    } else if cli.command == "correspondence" {
        cli.correspondence_action = positional.first().cloned();
        cli.correspondence_id = positional.get(1).cloned();
        cli.legacy_root = positional.get(2).map(PathBuf::from);
    } else if cli.command == "explain" {
        cli.query_id = positional.first().cloned();
        cli.legacy_root = positional.get(1).map(PathBuf::from);
    } else {
        cli.legacy_root = positional.first().map(PathBuf::from);
    }
    Ok(cli)
}

fn real_main() -> Result<i32, Error> {
    let cli = parse_cli()?;
    let legacy = cli
        .legacy_root
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let project = cli.project_root.as_deref().unwrap_or(&legacy);
    let spec = cli.spec_root.as_deref().unwrap_or(&legacy);
    let default_state = legacy.join(".adrproof");
    let state = cli.state_root.as_deref().unwrap_or(&default_state);
    let roots = VerificationRoots::explicit(project, spec, state);
    if cli.state_root.is_some()
        && (roots.state_root == roots.project_root || roots.state_root == roots.specification_root)
    {
        return Err(Error::ProviderFailure(
            "explicit state_root must not equal project_root or specification_root".into(),
        ));
    }
    if cli.state_root.is_some()
        && roots.state_root.starts_with(&roots.project_root)
        && roots.state_root != roots.project_root
    {
        eprintln!(
            "WARNING — explicit state_root is inside project_root; external state is recommended"
        );
    }
    if cli.state_root.is_some()
        && roots.state_root.starts_with(&roots.specification_root)
        && roots.state_root != roots.specification_root
        && !roots.state_root.starts_with(&roots.project_root)
    {
        eprintln!(
            "WARNING — explicit state_root is inside specification_root; external state is recommended"
        );
    }
    let (version, timeout_ms) = read_config(&roots.specification_root)
        .or_else(|| read_config(&roots.project_root))
        .unwrap_or_else(|| ("4.13.4".into(), 10_000));

    match cli.command.as_str() {
        "scenario" => scenario_command(&roots, &cli, &version, timeout_ms),
        "native-test" => native_test_command(&roots, &cli),
        "provider" => provider_command(&roots, &cli),
        "bundle" => bundle_command(&roots, &cli),
        "model" => model_command(&roots, &cli),
        "correspondence" => correspondence_command(&roots, &cli),
        "diagnose" => diagnose_command(&roots, &cli, &version, timeout_ms),
        "explain" => {
            let query_id = cli
                .query_id
                .ok_or_else(|| Error::ProviderFailure("explain requires an ID".into()))?;
            if let Some(value) = explain_model(&roots, &query_id)?
                .or(explain_correspondence(&roots, &query_id)?)
                .or(explain_scenario_or_parent(
                    &roots, &query_id, &version, timeout_ms,
                )?)
            {
                println!("{}", serde_json::to_string_pretty(&value).unwrap());
                return Ok(0);
            }
            let value =
                adrproof::query::explain_with_roots(&roots, &query_id, &version, timeout_ms)?;
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            Ok(0)
        }
        "impact" => {
            let value = adrproof::query::heterogeneous_impact_with_roots(
                &roots,
                &cli.impact_path
                    .ok_or_else(|| Error::ProviderFailure("impact requires --path PATH".into()))?,
                &version,
                timeout_ms,
            )?;
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            Ok(0)
        }
        "status" => {
            let value = adrproof::query::status_with_roots(&roots, &version, timeout_ms)?;
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            Ok(0)
        }
        "facts" => facts(&roots, cli.json, cli.summary),
        _ => {
            let exe = std::env::var("ADRPROOF_Z3").unwrap_or_else(|_| "z3".into());
            let backend = Z3Backend {
                executable: exe,
                expected_version: version,
                timeout_ms,
            };
            report(adrproof::run_check_with_roots(&roots, &backend)?, cli.json)
        }
    }
}

fn explain_model(roots: &VerificationRoots, id: &str) -> Result<Option<serde_json::Value>, Error> {
    let definitions = adrproof::quint::discover(&roots.specification_root)?;
    if let Some(definition) = definitions
        .iter()
        .find(|definition| definition.id == id || format!("MODEL:{}", definition.id) == id)
    {
        return Ok(Some(serde_json::json!({
            "kind": "formal_model_obligation",
            "id": format!("MODEL:{}", definition.id),
            "definition": definition,
            "latest_evidence": adrproof::quint::latest_assessment(roots, definition)?,
            "dependency_edges": adrproof::quint::graph_edges(roots, &definitions, &[])?.into_iter()
                .filter(|edge| serde_json::to_string(edge).is_ok_and(|value| value.contains(&definition.id)))
                .collect::<Vec<_>>(),
            "soundness_boundary": "Formal model evidence does not establish implementation conformance",
        })));
    }
    let validations = adrproof::quint::discover_validations(&roots.specification_root)?;
    let Some(definition) = validations.iter().find(|definition| {
        definition.id == id || format!("MODEL-VALIDATION:{}", definition.id) == id
    }) else {
        return Ok(None);
    };
    Ok(Some(serde_json::json!({
        "kind": "scenario_model_validation_obligation",
        "id": format!("MODEL-VALIDATION:{}", definition.id),
        "definition": definition,
        "latest_evidence": adrproof::quint::latest_validation_assessment(roots, definition)?,
        "dependency_edges": adrproof::quint::graph_edges(roots, &definitions, &validations)?.into_iter()
            .filter(|edge| serde_json::to_string(edge).is_ok_and(|value| value.contains(&definition.id)))
            .collect::<Vec<_>>(),
        "soundness_boundary": "Cross-validation admits selected observed traces; it is not a refinement proof",
    })))
}

fn model_command(roots: &VerificationRoots, cli: &Cli) -> Result<i32, Error> {
    let definitions = adrproof::quint::discover(&roots.specification_root)?;
    let validations = adrproof::quint::discover_validations(&roots.specification_root)?;
    match cli.model_action.as_deref() {
        Some("list") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "model_checks": definitions,
                    "scenario_model_validations": validations,
                }))
                .expect("model definitions serialization")
            );
            Ok(0)
        }
        Some("check") => {
            let id = cli
                .model_id
                .as_deref()
                .ok_or_else(|| Error::ProviderFailure("model check requires an ID".into()))?;
            let definition = definitions
                .iter()
                .find(|definition| definition.id == id)
                .ok_or_else(|| Error::ProviderFailure(format!("unknown model check `{id}`")))?;
            let evidence = adrproof::quint::run(roots, definition)?;
            let evidence =
                adrproof::quint::store(&roots.state_root.join("model-evidence"), evidence)?;
            adrproof::quint::write_graph(roots, &definitions, &validations)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "model_id": evidence.model_id,
                    "model_check_id": evidence.model_check_id,
                    "property_id": evidence.property_id,
                    "result": evidence.result_at_execution,
                    "current_validity": "CURRENT",
                    "backend": evidence.backend,
                    "backend_version": evidence.backend_version,
                    "quint_version": evidence.quint_version,
                    "constants": evidence.constants,
                    "bounds": evidence.bounds,
                    "model_bindings": evidence.model_bindings,
                    "fairness": evidence.fairness,
                    "exhaustive_or_bounded": evidence.exploration,
                    "completion": evidence.completion,
                    "explored_state_stats": evidence.explored_state_stats,
                    "counterexample": evidence.counterexample,
                    "authority": evidence.authority,
                    "evidence_id": evidence.id,
                    "diagnostics": evidence.diagnostics,
                }))
                .expect("model check report serialization")
            );
            Ok(match evidence.result_at_execution {
                adrproof::evidence::VerificationStatus::Pass => 0,
                adrproof::evidence::VerificationStatus::Fail => 1,
                adrproof::evidence::VerificationStatus::Error => 6,
                _ => 3,
            })
        }
        Some("validate") => {
            let selected = cli.model_id.as_deref();
            let selected_definitions = validations
                .iter()
                .filter(|definition| selected.is_none_or(|id| definition.id == id))
                .collect::<Vec<_>>();
            if let Some(selected) = selected.filter(|_| selected_definitions.is_empty()) {
                return Err(Error::ProviderFailure(format!(
                    "unknown model validation `{selected}`"
                )));
            }
            if selected_definitions.is_empty() {
                return Err(Error::ProviderFailure(
                    "no scenario-model validation definitions were found".into(),
                ));
            }
            let mut evidence = Vec::new();
            for definition in selected_definitions {
                let item = adrproof::quint::run_validation(roots, definition)?;
                evidence.push(adrproof::quint::store_validation(
                    &roots.state_root.join("model-validation-evidence"),
                    item,
                )?);
            }
            adrproof::quint::write_graph(roots, &definitions, &validations)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&evidence)
                    .expect("model validation report serialization")
            );
            Ok(
                if evidence.iter().all(|item| {
                    item.result_at_execution == adrproof::evidence::VerificationStatus::Pass
                }) {
                    0
                } else if evidence.iter().any(|item| {
                    item.result_at_execution == adrproof::evidence::VerificationStatus::Fail
                }) {
                    1
                } else if evidence.iter().any(|item| {
                    item.result_at_execution == adrproof::evidence::VerificationStatus::Error
                }) {
                    6
                } else {
                    3
                },
            )
        }
        Some("status") => {
            let selected = cli.model_id.as_deref();
            let values = definitions
                .iter()
                .filter(|definition| selected.is_none_or(|id| definition.id == id))
                .map(|definition| {
                    Ok(serde_json::json!({
                        "model_check_id": definition.id,
                        "latest": adrproof::quint::latest_assessment(roots, definition)?,
                    }))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            if let Some(selected) = selected.filter(|_| values.is_empty()) {
                return Err(Error::ProviderFailure(format!(
                    "unknown model check `{selected}`"
                )));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "roots": roots.view(),
                    "model_checks": values,
                    "scenario_model_validations": validations.iter().map(|definition| {
                        Ok(serde_json::json!({
                            "validation_id": definition.id,
                            "latest": adrproof::quint::latest_validation_assessment(roots, definition)?,
                        }))
                    }).collect::<Result<Vec<_>, Error>>()?,
                }))
                .expect("model status serialization")
            );
            Ok(0)
        }
        _ => Err(Error::ProviderFailure(
            "model requires one of: list, check <ID>, validate [ID], status [ID]".into(),
        )),
    }
}

fn correspondence_command(roots: &VerificationRoots, cli: &Cli) -> Result<i32, Error> {
    let definitions = adrproof::correspondence::discover(&roots.specification_root)?;
    match cli.correspondence_action.as_deref() {
        Some("list") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&definitions)
                    .expect("correspondence definitions serialization")
            );
            Ok(0)
        }
        Some("check") => {
            let id = cli.correspondence_id.as_deref().ok_or_else(|| {
                Error::ProviderFailure("correspondence check requires an ID".into())
            })?;
            let definition = definitions
                .iter()
                .find(|definition| definition.id == id)
                .ok_or_else(|| Error::ProviderFailure(format!("unknown correspondence `{id}`")))?;
            let evidence = adrproof::correspondence::store(
                &roots.state_root.join("correspondence-evidence"),
                adrproof::correspondence::run(roots, definition)?,
            )?;
            adrproof::correspondence::write_graph(roots, &definitions)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "correspondence_id": evidence.correspondence_id,
                    "result": evidence.result_at_execution,
                    "current_validity": "CURRENT",
                    "transitions": evidence.transitions,
                    "authority": evidence.authority,
                    "does_not_prove": evidence.does_not_prove,
                    "diagnostics": evidence.diagnostics,
                    "evidence_id": evidence.id,
                }))
                .expect("correspondence check report serialization")
            );
            Ok(match evidence.result_at_execution {
                adrproof::evidence::VerificationStatus::Pass => 0,
                adrproof::evidence::VerificationStatus::Fail => 1,
                adrproof::evidence::VerificationStatus::Error => 6,
                _ => 3,
            })
        }
        Some("status") => {
            let selected = cli.correspondence_id.as_deref();
            let values = definitions
                .iter()
                .filter(|definition| selected.is_none_or(|id| definition.id == id))
                .map(|definition| {
                    Ok(serde_json::json!({
                        "correspondence_id": definition.id,
                        "latest": adrproof::correspondence::latest_assessment(roots, definition)?,
                    }))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            if let Some(selected) = selected.filter(|_| values.is_empty()) {
                return Err(Error::ProviderFailure(format!(
                    "unknown correspondence `{selected}`"
                )));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "roots": roots.view(),
                    "correspondence": values,
                }))
                .expect("correspondence status serialization")
            );
            Ok(0)
        }
        _ => Err(Error::ProviderFailure(
            "correspondence requires one of: list, check <ID>, status [ID]".into(),
        )),
    }
}

fn explain_correspondence(
    roots: &VerificationRoots,
    id: &str,
) -> Result<Option<serde_json::Value>, Error> {
    let definitions = adrproof::correspondence::discover(&roots.specification_root)?;
    let Some(definition) = definitions.iter().find(|definition| {
        definition.id == id || format!("CORRESPONDENCE:{}", definition.id) == id
    }) else {
        return Ok(None);
    };
    Ok(Some(serde_json::json!({
        "kind": "rust_quint_static_correspondence_obligation",
        "id": format!("CORRESPONDENCE:{}", definition.id),
        "definition": definition,
        "latest_evidence": adrproof::correspondence::latest_assessment(roots, definition)?,
        "dependency_edges": adrproof::correspondence::graph_edges(roots, &definitions)?
            .into_iter()
            .filter(|edge| serde_json::to_string(edge).is_ok_and(|value| value.contains(&definition.id)))
            .collect::<Vec<_>>(),
        "soundness_boundary": "AST-level syntactic correspondence is not a type-resolved call graph or implementation refinement proof",
    })))
}

fn explain_scenario_or_parent(
    roots: &VerificationRoots,
    id: &str,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<Option<serde_json::Value>, Error> {
    let definitions = adrproof::scenario::discover(&roots.specification_root)?;
    if let Some(definition) = definitions
        .iter()
        .find(|definition| definition.id == id || format!("SCENARIO:{}", definition.id) == id)
    {
        return Ok(Some(serde_json::json!({
            "kind": "scenario_obligation",
            "id": format!("SCENARIO:{}", definition.id),
            "definition": definition,
            "latest_evidence": adrproof::scenario::latest_assessment(roots, definition)?,
            "dependency_edges": adrproof::scenario::graph_edges(roots, &definitions, &[])?.into_iter()
                .filter(|edge| serde_json::to_string(edge).is_ok_and(|value| value.contains(&definition.id)))
                .collect::<Vec<_>>(),
        })));
    }
    let assessments = aggregate_parents(roots, &definitions, backend_version, timeout_ms)?;
    let Some(assessment) = assessments
        .into_iter()
        .find(|parent| parent.parent_id == id)
    else {
        return Ok(None);
    };
    let parents = adrproof::scenario::discover_parents(&roots.specification_root)?;
    let source = parents
        .iter()
        .find(|parent| parent.id == id)
        .map(|parent| roots.spec_identity(&parent.source));
    let edges = adrproof::scenario::graph_edges(roots, &definitions, &parents)?
        .into_iter()
        .filter(|edge| serde_json::to_string(edge).is_ok_and(|value| value.contains(id)))
        .collect::<Vec<_>>();
    Ok(Some(serde_json::json!({
        "kind": "parent_obligation",
        "source": source,
        "assessment": assessment,
        "dependency_edges": edges,
        "aggregation": "ALL required children must have PASS/CURRENT authoritative evidence",
    })))
}

fn scenario_command(
    roots: &VerificationRoots,
    cli: &Cli,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<i32, Error> {
    let definitions = adrproof::scenario::discover(&roots.specification_root)?;
    match cli.scenario_action.as_deref() {
        Some("list") => {
            let value = definitions
                .iter()
                .map(|definition| {
                    serde_json::json!({
                        "scenario_id": definition.id,
                        "version": definition.version,
                        "description": definition.description,
                        "fault_point": definition.coverage.fault_point,
                        "authority": definition.authority,
                        "does_not_prove": definition.does_not_prove,
                    })
                })
                .collect::<Vec<_>>();
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            Ok(0)
        }
        Some("run") => {
            let id = cli.scenario_id.as_deref().ok_or_else(|| {
                Error::ProviderFailure("scenario run requires a scenario ID".into())
            })?;
            let definition = definitions
                .iter()
                .find(|definition| definition.id == id)
                .ok_or_else(|| Error::ProviderFailure(format!("unknown scenario `{id}`")))?;
            let evidence = adrproof::scenario::run(roots, definition)?;
            let evidence =
                adrproof::scenario::store(&roots.state_root.join("scenario-evidence"), evidence)?;
            let parents = adrproof::scenario::discover_parents(&roots.specification_root)?;
            adrproof::scenario::write_graph(roots, &definitions, &parents)?;
            let value = serde_json::json!({
                "scenario_id": evidence.scenario_id,
                "result": evidence.result_at_execution,
                "current_validity": "CURRENT",
                "fault_point": evidence.fault_point,
                "implementation_fingerprint": evidence.implementation_fingerprint,
                "fixture_fingerprint": evidence.fixture_fingerprint,
                "postconditions": evidence.postconditions,
                "failed_postconditions": evidence.postconditions.iter().filter(|item| !item.passed).collect::<Vec<_>>(),
                "diagnostics": evidence.diagnostics,
                "evidence_id": evidence.id,
            });
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
            Ok(if evidence.result_at_execution.is_ci_pass() {
                0
            } else if evidence.result_at_execution == adrproof::evidence::VerificationStatus::Fail {
                1
            } else if evidence.result_at_execution == adrproof::evidence::VerificationStatus::Error
            {
                6
            } else {
                3
            })
        }
        Some("status") => {
            let selected = cli.scenario_id.as_deref();
            let mut assessments = Vec::new();
            for definition in definitions
                .iter()
                .filter(|definition| selected.is_none_or(|id| definition.id == id))
            {
                let assessment = adrproof::scenario::latest_assessment(roots, definition)?;
                assessments.push(serde_json::json!({
                    "scenario_id": definition.id,
                    "latest": assessment,
                }));
            }
            if let Some(selected) = selected.filter(|_| assessments.is_empty()) {
                return Err(Error::ProviderFailure(format!(
                    "unknown scenario `{selected}`"
                )));
            }
            let parents = aggregate_parents(roots, &definitions, backend_version, timeout_ms)?;
            let parent_definitions =
                adrproof::scenario::discover_parents(&roots.specification_root)?;
            adrproof::scenario::write_graph(roots, &definitions, &parent_definitions)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "roots": roots.view(),
                    "scenarios": assessments,
                    "parents": parents,
                }))
                .unwrap()
            );
            Ok(0)
        }
        _ => Err(Error::ProviderFailure(
            "scenario requires one of: list, run <ID>, status [ID]".into(),
        )),
    }
}

fn native_test_command(roots: &VerificationRoots, cli: &Cli) -> Result<i32, Error> {
    let definitions = adrproof::native_test::discover(&roots.specification_root)?;
    match cli.native_test_action.as_deref() {
        Some("list") => {
            println!(
                "{}",
                serde_json::to_string_pretty(&definitions)
                    .expect("native test definitions serialization")
            );
            Ok(0)
        }
        Some("import") => {
            let id = cli.native_test_id.as_deref().ok_or_else(|| {
                Error::ProviderFailure("native-test import requires an ID".into())
            })?;
            let report = cli.report_path.as_deref().ok_or_else(|| {
                Error::ProviderFailure("native-test import requires --report PATH".into())
            })?;
            let definition = definitions
                .iter()
                .find(|definition| definition.id == id)
                .ok_or_else(|| Error::ProviderFailure(format!("unknown native test `{id}`")))?;
            let evidence = adrproof::native_test::import(roots, definition, report)?;
            let evidence = adrproof::native_test::store(
                &roots.state_root.join("native-test-evidence"),
                evidence,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "native_test_id": evidence.definition_id,
                    "result": evidence.result_at_execution,
                    "current_validity": "CURRENT",
                    "passed": evidence.passed,
                    "failed": evidence.failed,
                    "skipped": evidence.skipped,
                    "non_vacuity": evidence.non_vacuity,
                    "diagnostics": evidence.diagnostics,
                    "evidence_id": evidence.id,
                }))
                .expect("native test report serialization")
            );
            Ok(if evidence.result_at_execution.is_ci_pass() {
                0
            } else {
                1
            })
        }
        Some("status") => {
            let selected = cli.native_test_id.as_deref();
            let values = definitions
                .iter()
                .filter(|definition| selected.is_none_or(|id| definition.id == id))
                .map(|definition| {
                    Ok(serde_json::json!({
                        "native_test_id": definition.id,
                        "latest": adrproof::native_test::latest_assessment(roots, definition)?,
                    }))
                })
                .collect::<Result<Vec<_>, Error>>()?;
            if let Some(selected) = selected.filter(|_| values.is_empty()) {
                return Err(Error::ProviderFailure(format!(
                    "unknown native test `{selected}`"
                )));
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "roots": roots.view(),
                    "native_tests": values,
                }))
                .expect("native test status serialization")
            );
            Ok(0)
        }
        _ => Err(Error::ProviderFailure(
            "native-test requires one of: list, import <ID> --report PATH, status [ID]".into(),
        )),
    }
}

fn bundle_command(roots: &VerificationRoots, cli: &Cli) -> Result<i32, Error> {
    match cli.bundle_action.as_deref() {
        Some("create") => {
            let output = cli.output_path.as_deref().ok_or_else(|| {
                Error::ProviderFailure("bundle create requires --output PATH".into())
            })?;
            let signing_key = cli
                .signing_key_path
                .as_deref()
                .map(|path| adrproof::bundle::read_key(path, "signing key"))
                .transpose()?;
            let manifest = match signing_key.as_ref() {
                Some(key) => adrproof::bundle::create_signed(roots, output, key)?,
                None => adrproof::bundle::create(roots, output)?,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "bundle": output,
                    "manifest": manifest,
                }))
                .expect("bundle create serialization")
            );
            Ok(0)
        }
        Some("verify") => {
            let path = cli
                .bundle_path
                .as_deref()
                .ok_or_else(|| Error::ProviderFailure("bundle verify requires a PATH".into()))?;
            let public_key = cli
                .public_key_path
                .as_deref()
                .map(|path| adrproof::bundle::read_key(path, "public key"))
                .transpose()?;
            let verification = adrproof::bundle::verify_with_key(
                path,
                public_key.as_ref(),
                cli.require_signature,
            )?;
            println!(
                "{}",
                serde_json::to_string_pretty(&verification)
                    .expect("bundle verification serialization")
            );
            Ok(if verification.valid { 0 } else { 1 })
        }
        _ => Err(Error::ProviderFailure(
            "bundle requires one of: create --output PATH, verify PATH".into(),
        )),
    }
}

fn diagnose_command(
    roots: &VerificationRoots,
    cli: &Cli,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<i32, Error> {
    use adrproof::evidence::{EvidenceValidity, VerificationStatus};

    fn attention(status: &VerificationStatus, validity: &EvidenceValidity) -> bool {
        status != &VerificationStatus::Pass || validity != &EvidenceValidity::Current
    }

    let mut findings = Vec::new();
    let scenarios = adrproof::scenario::discover(&roots.specification_root)?;
    for definition in &scenarios {
        match adrproof::scenario::latest_assessment(roots, definition)? {
            Some(value)
                if attention(
                    &value.evidence.result_at_execution,
                    &value.current_validity,
                ) => findings.push(serde_json::json!({
                    "kind": "scenario",
                    "id": format!("SCENARIO:{}", definition.id),
                    "source": roots.spec_identity(&definition.source),
                    "status": value.evidence.result_at_execution,
                    "validity": value.current_validity,
                    "failed_postconditions": value.evidence.postconditions.iter().filter(|item| !item.passed).collect::<Vec<_>>(),
                    "diagnostics": value.evidence.diagnostics,
                    "trace": value.evidence.trace,
                })),
            None => findings.push(serde_json::json!({
                "kind": "scenario",
                "id": format!("SCENARIO:{}", definition.id),
                "source": roots.spec_identity(&definition.source),
                "status": "UNVERIFIED",
                "diagnostics": ["no scenario evidence"],
            })),
            _ => {}
        }
    }

    for definition in adrproof::native_test::discover(&roots.specification_root)? {
        match adrproof::native_test::latest_assessment(roots, &definition)? {
            Some(value)
                if attention(&value.evidence.result_at_execution, &value.current_validity) =>
            {
                findings.push(serde_json::json!({
                    "kind": "native_test",
                    "id": format!("NATIVE-TEST:{}", definition.id),
                    "source": roots.spec_identity(&definition.source),
                    "status": value.evidence.result_at_execution,
                    "validity": value.current_validity,
                    "non_vacuity": value.evidence.non_vacuity,
                    "diagnostics": value.evidence.diagnostics,
                }))
            }
            None => findings.push(serde_json::json!({
                "kind": "native_test",
                "id": format!("NATIVE-TEST:{}", definition.id),
                "source": roots.spec_identity(&definition.source),
                "status": "UNVERIFIED",
                "diagnostics": ["no imported native test evidence"],
            })),
            _ => {}
        }
    }

    for definition in adrproof::quint::discover(&roots.specification_root)? {
        match adrproof::quint::latest_assessment(roots, &definition)? {
            Some(value)
                if attention(&value.evidence.result_at_execution, &value.current_validity) =>
            {
                findings.push(serde_json::json!({
                    "kind": "model",
                    "id": format!("MODEL:{}", definition.id),
                    "source": roots.spec_identity(&definition.source),
                    "status": value.evidence.result_at_execution,
                    "validity": value.current_validity,
                    "counterexample": value.evidence.counterexample,
                    "diagnostics": value.evidence.diagnostics,
                }))
            }
            None => findings.push(serde_json::json!({
                "kind": "model",
                "id": format!("MODEL:{}", definition.id),
                "source": roots.spec_identity(&definition.source),
                "status": "UNVERIFIED",
                "diagnostics": ["no model evidence"],
            })),
            _ => {}
        }
    }

    for definition in adrproof::quint::discover_validations(&roots.specification_root)? {
        match adrproof::quint::latest_validation_assessment(roots, &definition)? {
            Some(value)
                if attention(&value.evidence.result_at_execution, &value.current_validity) =>
            {
                findings.push(serde_json::json!({
                    "kind": "model_validation",
                    "id": format!("MODEL-VALIDATION:{}", definition.id),
                    "source": roots.spec_identity(&definition.source),
                    "status": value.evidence.result_at_execution,
                    "validity": value.current_validity,
                    "mappings": value.evidence.mappings,
                    "diagnostics": value.evidence.diagnostics,
                }))
            }
            None => findings.push(serde_json::json!({
                "kind": "model_validation",
                "id": format!("MODEL-VALIDATION:{}", definition.id),
                "source": roots.spec_identity(&definition.source),
                "status": "UNVERIFIED",
                "diagnostics": ["no model-validation evidence"],
            })),
            _ => {}
        }
    }

    for definition in adrproof::correspondence::discover(&roots.specification_root)? {
        match adrproof::correspondence::latest_assessment(roots, &definition)? {
            Some(value)
                if attention(&value.evidence.result_at_execution, &value.current_validity) =>
            {
                findings.push(serde_json::json!({
                    "kind": "correspondence",
                    "id": format!("CORRESPONDENCE:{}", definition.id),
                    "source": roots.spec_identity(&definition.source),
                    "status": value.evidence.result_at_execution,
                    "validity": value.current_validity,
                    "transitions": value.evidence.transitions,
                    "diagnostics": value.evidence.diagnostics,
                }))
            }
            None => findings.push(serde_json::json!({
                "kind": "correspondence",
                "id": format!("CORRESPONDENCE:{}", definition.id),
                "source": roots.spec_identity(&definition.source),
                "status": "UNVERIFIED",
                "diagnostics": ["no correspondence evidence"],
            })),
            _ => {}
        }
    }

    for parent in aggregate_parents(roots, &scenarios, backend_version, timeout_ms)? {
        if parent.status != VerificationStatus::Pass {
            findings.push(serde_json::json!({
                "kind": "parent",
                "id": parent.parent_id,
                "status": parent.status,
                "children_requiring_attention": parent.children.iter().filter(|child| {
                    child.status != VerificationStatus::Pass
                        || child.validity != Some(EvidenceValidity::Current)
                }).collect::<Vec<_>>(),
                "authority": parent.authority,
            }));
        }
    }
    let original_finding_count = findings.len();
    let assessment = match cli.policy_path.as_deref() {
        Some(path) => adrproof::policy::apply(
            findings,
            &adrproof::policy::load(path)?,
            adrproof::policy::now_unix_seconds(),
        ),
        None => adrproof::policy::PolicyAssessment {
            unwaived_finding_count: findings.len(),
            findings,
            applied_waivers: Vec::new(),
            diagnostics: Vec::new(),
        },
    };
    if let Some(path) = cli.sarif_path.as_deref() {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&adrproof::policy::sarif(&assessment.findings))
                .expect("SARIF serialization"),
        )
        .map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let policy_clean = assessment.diagnostics.is_empty();
    let accepted = assessment.unwaived_finding_count == 0 && policy_clean;
    let result = if original_finding_count == 0 && policy_clean {
        "PASS"
    } else if accepted {
        "WAIVED_ATTENTION"
    } else {
        "ATTENTION"
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema_version": "adrproof-diagnostics-v1",
            "roots": roots.view(),
            "result": result,
            "finding_count": original_finding_count,
            "unwaived_finding_count": assessment.unwaived_finding_count,
            "findings": assessment.findings,
            "applied_waivers": assessment.applied_waivers,
            "policy_diagnostics": assessment.diagnostics,
            "sarif": cli.sarif_path,
            "soundness_boundary": "Diagnostics and waivers correlate recorded evidence; waivers never change the underlying verifier verdicts.",
        }))
        .expect("diagnostic report serialization")
    );
    Ok(if accepted { 0 } else { 1 })
}

fn aggregate_parents(
    roots: &VerificationRoots,
    definitions: &[adrproof::scenario::ScenarioDefinition],
    backend_version: &str,
    timeout_ms: u64,
) -> Result<Vec<adrproof::scenario::ParentAssessment>, Error> {
    use adrproof::evidence::VerificationStatus;
    use adrproof::project::EvidenceId;
    use adrproof::scenario::{ChildEvidenceKind, ChildStatus};

    let parents = adrproof::scenario::discover_parents(&roots.specification_root)?;
    let relational_status = adrproof::query::status_with_roots(roots, backend_version, timeout_ms)?;
    let relational_latest = relational_status.latest_evidence.first();
    let model_definitions = adrproof::quint::discover(&roots.specification_root)?;
    let validation_definitions = adrproof::quint::discover_validations(&roots.specification_root)?;
    let correspondence_definitions = adrproof::correspondence::discover(&roots.specification_root)?;
    let native_test_definitions = adrproof::native_test::discover(&roots.specification_root)?;
    parents
        .iter()
        .map(|parent| {
            let mut children = Vec::new();
            for required in &parent.required_children {
                match required.evidence_kind {
                    ChildEvidenceKind::Scenario => {
                        let id = required
                            .obligation_id
                            .strip_prefix("SCENARIO:")
                            .unwrap_or(&required.obligation_id);
                        let assessment = definitions
                            .iter()
                            .find(|definition| definition.id == id)
                            .map(|definition| {
                                adrproof::scenario::latest_assessment(roots, definition)
                            })
                            .transpose()?
                            .flatten();
                        children.push(match assessment {
                            Some(assessment) => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: assessment.evidence.result_at_execution,
                                validity: Some(assessment.current_validity),
                                evidence_id: Some(assessment.evidence.id),
                            },
                            None => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: VerificationStatus::Unverified,
                                validity: None,
                                evidence_id: None,
                            },
                        });
                    }
                    ChildEvidenceKind::Relational => {
                        children.push(match relational_latest {
                            Some(value) => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: value.result_at_execution.clone(),
                                validity: Some(value.current_validity.clone()),
                                evidence_id: Some(EvidenceId(value.id.clone())),
                            },
                            None => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: VerificationStatus::Unverified,
                                validity: None,
                                evidence_id: None,
                            },
                        });
                    }
                    ChildEvidenceKind::NativeTest => {
                        let id = required
                            .obligation_id
                            .strip_prefix("NATIVE-TEST:")
                            .unwrap_or(&required.obligation_id);
                        let assessment = native_test_definitions
                            .iter()
                            .find(|definition| definition.id == id)
                            .map(|definition| {
                                adrproof::native_test::latest_assessment(roots, definition)
                            })
                            .transpose()?
                            .flatten();
                        children.push(match assessment {
                            Some(assessment) => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: assessment.evidence.result_at_execution,
                                validity: Some(assessment.current_validity),
                                evidence_id: Some(assessment.evidence.id),
                            },
                            None => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: VerificationStatus::Unverified,
                                validity: None,
                                evidence_id: None,
                            },
                        });
                    }
                    ChildEvidenceKind::Model => {
                        let id = required
                            .obligation_id
                            .strip_prefix("MODEL:")
                            .unwrap_or(&required.obligation_id);
                        let assessment = model_definitions
                            .iter()
                            .find(|definition| definition.id == id)
                            .map(|definition| adrproof::quint::latest_assessment(roots, definition))
                            .transpose()?
                            .flatten();
                        children.push(match assessment {
                            Some(assessment) => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: assessment.evidence.result_at_execution,
                                validity: Some(assessment.current_validity),
                                evidence_id: Some(assessment.evidence.id),
                            },
                            None => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: VerificationStatus::Unverified,
                                validity: None,
                                evidence_id: None,
                            },
                        });
                    }
                    ChildEvidenceKind::ModelValidation => {
                        let id = required
                            .obligation_id
                            .strip_prefix("MODEL-VALIDATION:")
                            .unwrap_or(&required.obligation_id);
                        let assessment = validation_definitions
                            .iter()
                            .find(|definition| definition.id == id)
                            .map(|definition| {
                                adrproof::quint::latest_validation_assessment(roots, definition)
                            })
                            .transpose()?
                            .flatten();
                        children.push(match assessment {
                            Some(assessment) => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: assessment.evidence.result_at_execution,
                                validity: Some(assessment.current_validity),
                                evidence_id: Some(assessment.evidence.id),
                            },
                            None => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: VerificationStatus::Unverified,
                                validity: None,
                                evidence_id: None,
                            },
                        });
                    }
                    ChildEvidenceKind::Correspondence => {
                        let id = required
                            .obligation_id
                            .strip_prefix("CORRESPONDENCE:")
                            .unwrap_or(&required.obligation_id);
                        let assessment = correspondence_definitions
                            .iter()
                            .find(|definition| definition.id == id)
                            .map(|definition| {
                                adrproof::correspondence::latest_assessment(roots, definition)
                            })
                            .transpose()?
                            .flatten();
                        children.push(match assessment {
                            Some(assessment) => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: assessment.evidence.result_at_execution,
                                validity: Some(assessment.current_validity),
                                evidence_id: Some(assessment.evidence.id),
                            },
                            None => ChildStatus {
                                obligation_id: required.obligation_id.clone(),
                                status: VerificationStatus::Unverified,
                                validity: None,
                                evidence_id: None,
                            },
                        });
                    }
                }
            }
            Ok(adrproof::scenario::aggregate(parent, children))
        })
        .collect()
}

fn facts(roots: &VerificationRoots, json: bool, summary: bool) -> Result<i32, Error> {
    let cargo = CargoMetadataProvider::discover(&roots.project_root)
        .map(|provider| provider.extract())
        .transpose()?;
    let sql =
        adrproof::sql_migrations::PostgresMigrationFactProvider::discover(&roots.project_root)
            .map(|provider| provider.extract())
            .transpose()?;
    let external = adrproof::external_provider::run_configured(roots)?;
    if cargo.is_none() && sql.is_none() && external.is_empty() {
        return Err(Error::ProviderFailure(format!(
            "{} has no supported fact source",
            roots.project_root.display()
        )));
    }
    if summary {
        let mut providers = serde_json::Map::new();
        if let Some(extracted) = &cargo {
            providers.insert(
                "CargoMetadataProvider".into(),
                serde_json::json!({
                    "provider_command": "cargo metadata --format-version 1 --no-deps --offline",
                    "fact_counts": extracted.fact_counts(),
                    "coverage": extracted.coverage,
                }),
            );
        }
        if let Some(extracted) = &sql {
            let mut counts = BTreeMap::<String, usize>::new();
            for fact in &extracted.facts {
                *counts.entry(fact.relation.clone()).or_default() += 1;
            }
            providers.insert(
                "PostgresMigrationFactProvider".into(),
                serde_json::json!({
                    "parser": "pg_query 6.2.0",
                    "migration_roots": [roots.project_identity(&roots.project_root.join("migrations"))],
                    "migration_count": extracted.migration_count,
                    "fact_counts": counts,
                    "coverage_summary": sql_coverage_summary(extracted),
                    "coverage": extracted.coverage,
                    "unsupported": extracted.unsupported,
                }),
            );
        }
        for extracted in &external {
            let mut counts = BTreeMap::<String, usize>::new();
            for fact in &extracted.facts {
                *counts.entry(fact.relation.clone()).or_default() += 1;
            }
            providers.insert(
                format!("ExternalProvider:{}", extracted.provider.id),
                serde_json::json!({
                    "protocol": adrproof::external_provider::PROTOCOL_VERSION,
                    "version": extracted.provider.version,
                    "executable": extracted.executable,
                    "elapsed_ms": extracted.elapsed_ms,
                    "fact_counts": counts,
                    "coverage": extracted.coverage,
                    "inputs": extracted.inputs.iter().map(|input| &input.identity).collect::<Vec<_>>(),
                    "diagnostics": extracted.diagnostics,
                }),
            );
        }
        let value = serde_json::json!({
            "roots": roots.view(),
            "providers": providers,
        });
        if json {
            println!("{}", serde_json::to_string_pretty(&value).unwrap());
        } else {
            for (provider, details) in providers {
                println!("{provider}");
                if let Some(counts) = details
                    .get("fact_counts")
                    .and_then(|value| value.as_object())
                {
                    for (relation, count) in counts {
                        println!("  {relation}: {count}");
                    }
                }
                if let Some(coverage) = details.get("coverage_summary") {
                    println!("  coverage: {}", serde_json::to_string(coverage).unwrap());
                }
            }
        }
    } else if json {
        let mut facts = cargo.map_or_else(Vec::new, |value| value.facts);
        facts.extend(sql.map_or_else(Vec::new, |value| value.facts));
        facts.extend(external.into_iter().flat_map(|value| value.facts));
        facts.sort_by(|a, b| a.id.cmp(&b.id));
        println!("{}", serde_json::to_string_pretty(&facts).unwrap());
    } else {
        let mut facts = cargo.map_or_else(Vec::new, |value| value.facts);
        facts.extend(sql.map_or_else(Vec::new, |value| value.facts));
        facts.extend(external.into_iter().flat_map(|value| value.facts));
        facts.sort_by(|a, b| a.id.cmp(&b.id));
        for fact in facts {
            println!("{}({})", fact.relation, fact.arguments.join(", "));
        }
    }
    Ok(0)
}

fn provider_command(roots: &VerificationRoots, cli: &Cli) -> Result<i32, Error> {
    if cli.provider_action.as_deref() != Some("check") {
        return Err(Error::ExternalProviderFailure {
            code: adrproof::external_provider::DIAGNOSTIC_CONFIGURATION,
            message: "usage: adrproof provider check [PROVIDER-ID] [--project-root PATH] [--spec-root PATH] [--state-root PATH] [--json]".into(),
        });
    }
    let runs = adrproof::external_provider::run_selected(roots, cli.provider_id.as_deref())?;
    if runs.is_empty() {
        return Err(Error::ExternalProviderFailure {
            code: adrproof::external_provider::DIAGNOSTIC_CONFIGURATION,
            message: "no external providers are configured".into(),
        });
    }
    let providers = runs
        .iter()
        .map(|run| {
            serde_json::json!({
                "provider": run.provider,
                "protocol": adrproof::external_provider::PROTOCOL_VERSION,
                "result": "PASS",
                "elapsed_ms": run.elapsed_ms,
                "facts": run.facts.len(),
                "artifacts": run.artifacts.len(),
                "coverage_claims": run.coverage.len(),
                "semantic_inputs": run.inputs.iter().map(|input| &input.identity).collect::<Vec<_>>(),
                "diagnostics": run.diagnostics,
            })
        })
        .collect::<Vec<_>>();
    if cli.json {
        let report = serde_json::json!({
            "schema_version": adrproof::external_provider::CHECK_REPORT_SCHEMA_VERSION,
            "protocol": adrproof::external_provider::PROTOCOL_VERSION,
            "result": "PASS",
            "providers": providers,
            "diagnostics": [],
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        for item in providers {
            println!(
                "{}: PASS ({} facts, {} coverage claims, {} ms)",
                item["provider"]["id"].as_str().unwrap_or("unknown"),
                item["facts"],
                item["coverage_claims"],
                item["elapsed_ms"]
            );
            if cli.summary {
                for input in item["semantic_inputs"].as_array().into_iter().flatten() {
                    println!("  input: {}", input.as_str().unwrap_or("<invalid>"));
                }
                for diagnostic in item["diagnostics"].as_array().into_iter().flatten() {
                    println!(
                        "  diagnostic: {}",
                        diagnostic.as_str().unwrap_or("<invalid>")
                    );
                }
            }
        }
    }
    Ok(0)
}

fn sql_coverage_summary(
    extracted: &adrproof::sql_migrations::SqlMigrationFacts,
) -> serde_json::Value {
    let model = adrproof::project::ProjectModel {
        fact_coverage: extracted.coverage.clone(),
        ..Default::default()
    };
    let tables = extracted
        .facts
        .iter()
        .filter(|fact| fact.relation == "table")
        .filter_map(|fact| fact.arguments.first().cloned())
        .collect::<Vec<_>>();
    let mut relations = BTreeMap::new();
    for relation in [
        "table",
        "column",
        "column_type",
        "column_not_null",
        "primary_key",
        "unique_constraint",
        "foreign_key",
        "check_constraint",
    ] {
        let closed = tables
            .iter()
            .filter(|table| {
                model.coverage_for(
                    relation,
                    &adrproof::project::CoverageScope::Table((*table).clone()),
                ) == Some(adrproof::project::WorldAssumption::Closed)
            })
            .count();
        relations.insert(
            relation,
            serde_json::json!({"closed_tables": closed, "total_tables": tables.len()}),
        );
    }
    let partial_scopes = extracted
        .coverage
        .iter()
        .filter(|coverage| coverage.world == adrproof::project::WorldAssumption::Partial)
        .map(|coverage| {
            serde_json::json!({
                "relation": coverage.relation,
                "scope": coverage.scope,
                "diagnostics": coverage.diagnostics,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "relations": relations,
        "partial_scopes": partial_scopes,
    })
}

fn read_config(root: &Path) -> Option<(String, u64)> {
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("adrproof.json")).ok()?).ok()?;
    Some((
        value.get("z3_version")?.as_str()?.into(),
        value
            .get("timeout_ms")
            .and_then(|item| item.as_u64())
            .unwrap_or(10_000),
    ))
}

fn report(report: adrproof::CheckReport, json: bool) -> Result<i32, Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap())
    } else {
        match report.verdict {
            Verdict::Sat => println!("SAT — project specification is consistent"),
            Verdict::Unsat => {
                println!(
                    "UNSAT — project specification is inconsistent\n\nConflicting constraints:\n"
                );
                for conflict in &report.conflicts {
                    println!(
                        "{}:{}\n  {}:{}:{}\n  {}\n",
                        conflict.adr_id,
                        conflict.clause_id,
                        conflict.span.filename.display(),
                        conflict.span.line,
                        conflict.span.column,
                        conflict.description
                    )
                }
            }
            ref verdict => println!("{verdict:?} — verification did not establish consistency"),
        }
        println!(
            "SMT artifact: {}\nProof ledger: {}",
            report.smt_artifact.display(),
            report.ledger_artifact.display()
        )
    }
    Ok(match report.verdict {
        Verdict::Sat => 0,
        Verdict::Unsat => 1,
        Verdict::InvalidInput => 2,
        Verdict::Unknown | Verdict::Unverified => 3,
        Verdict::Timeout => 4,
        Verdict::SolverFailure => 5,
    })
}
