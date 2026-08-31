use crate::Error;
use crate::evidence::{EvidenceValidity, InputFingerprint, VerificationStatus};
use crate::project::{ArtifactId, EvidenceId, GraphEdge, GraphNode, LinkKind, ProofObligationId};
use crate::roots::{SemanticInput, VerificationRoots};
use quote::ToTokens;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use syn::visit::Visit;

pub const PROVIDER_NAME: &str = "rust_quint_static_correspondence";
pub const PROVIDER_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustFunctionSelector {
    pub file: PathBuf,
    pub function: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionCorrespondence {
    pub id: String,
    pub rust: RustFunctionSelector,
    pub model_actions: Vec<String>,
    #[serde(default)]
    pub required_calls: Vec<String>,
    #[serde(default)]
    pub ordered_calls: Vec<String>,
    #[serde(default)]
    pub required_string_fragments: Vec<String>,
    #[serde(default)]
    pub required_syntax_fragments: Vec<String>,
    pub authority: String,
    #[serde(default)]
    pub does_not_prove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrespondenceDefinition {
    pub id: String,
    pub claim: String,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub model: PathBuf,
    pub transitions: Vec<TransitionCorrespondence>,
    #[serde(skip)]
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionAssessment {
    pub id: String,
    pub rust_file: String,
    pub rust_function: String,
    pub model_actions: Vec<String>,
    pub observed_calls: Vec<String>,
    pub missing_requirements: Vec<String>,
    pub status: VerificationStatus,
    pub authority: String,
    pub does_not_prove: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrespondenceEvidence {
    pub id: EvidenceId,
    pub obligation: ProofObligationId,
    pub correspondence_id: String,
    pub provider: String,
    pub provider_version: String,
    pub claim: String,
    pub authority: String,
    pub does_not_prove: Vec<String>,
    pub configuration_sha256: String,
    pub inputs: Vec<InputFingerprint>,
    pub transitions: Vec<TransitionAssessment>,
    pub result_at_execution: VerificationStatus,
    pub diagnostics: Vec<String>,
    pub recorded_at_unix_nanos: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrespondenceAssessment {
    pub evidence: CorrespondenceEvidence,
    pub current_validity: EvidenceValidity,
}

#[derive(Default)]
struct FunctionFacts {
    calls: Vec<String>,
    strings: Vec<String>,
    syntax: String,
}

#[derive(Default)]
struct FactVisitor {
    calls: Vec<String>,
    strings: Vec<String>,
}

impl<'ast> Visit<'ast> for FactVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*node.func
            && let Some(segment) = path.path.segments.last()
        {
            self.calls.push(segment.ident.to_string());
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        self.calls.push(node.method.to_string());
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        self.strings.push(node.value());
        syn::visit::visit_lit_str(self, node);
    }
}

pub fn discover(root: &Path) -> Result<Vec<CorrespondenceDefinition>, Error> {
    let directory = root.join("correspondence/checks");
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(&directory)
        .map_err(|source| Error::Io {
            path: directory.clone(),
            source,
        })?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory,
            source,
        })?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    paths.sort();
    paths
        .into_iter()
        .map(|path| {
            let mut definition: CorrespondenceDefinition =
                serde_json::from_slice(&fs::read(&path).map_err(|source| Error::Io {
                    path: path.clone(),
                    source,
                })?)
                .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))?;
            definition.source = path;
            validate_definition(&definition)?;
            Ok(definition)
        })
        .collect()
}

fn validate_definition(definition: &CorrespondenceDefinition) -> Result<(), Error> {
    if definition.id.is_empty()
        || definition.model.as_os_str().is_empty()
        || definition.transitions.is_empty()
    {
        return Err(Error::ProviderFailure(format!(
            "{}: correspondence id, model, and transitions are required",
            definition.source.display()
        )));
    }
    let mut ids = BTreeSet::new();
    for transition in &definition.transitions {
        if transition.id.is_empty()
            || transition.rust.file.as_os_str().is_empty()
            || transition.rust.function.is_empty()
            || transition.model_actions.is_empty()
        {
            return Err(Error::ProviderFailure(format!(
                "{}: every transition requires an id, Rust function, and model action",
                definition.source.display()
            )));
        }
        if !ids.insert(&transition.id) {
            return Err(Error::ProviderFailure(format!(
                "{}: duplicate transition id `{}`",
                definition.source.display(),
                transition.id
            )));
        }
    }
    Ok(())
}

pub fn run(
    roots: &VerificationRoots,
    definition: &CorrespondenceDefinition,
) -> Result<CorrespondenceEvidence, Error> {
    let inputs = current_inputs(roots, definition)?;
    let configuration_sha256 = definition_fingerprint(definition);
    let model_path = roots.specification_root.join(&definition.model);
    let model = fs::read_to_string(&model_path).map_err(|source| Error::Io {
        path: model_path,
        source,
    })?;
    let model_actions = quint_actions(&model);
    let mut parsed = BTreeMap::<PathBuf, Result<syn::File, String>>::new();
    let mut assessments = Vec::new();
    let mut diagnostics = Vec::new();

    for transition in &definition.transitions {
        let path = roots.project_root.join(&transition.rust.file);
        let syntax = parsed
            .entry(transition.rust.file.clone())
            .or_insert_with(|| {
                fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", path.display()))
                    .and_then(|source| syn::parse_file(&source).map_err(|error| error.to_string()))
            });
        let mut missing = Vec::new();
        let mut observed_calls = Vec::new();
        let status = match syntax {
            Err(error) => {
                diagnostics.push(format!(
                    "{} could not parse {}: {error}",
                    transition.id,
                    transition.rust.file.display()
                ));
                VerificationStatus::Error
            }
            Ok(file) => match function_facts(file, &transition.rust.function) {
                Err(error) => {
                    missing.push(error);
                    VerificationStatus::Fail
                }
                Ok(facts) => {
                    observed_calls = facts.calls.clone();
                    for call in &transition.required_calls {
                        if !facts.calls.contains(call) {
                            missing.push(format!("missing Rust call `{call}`"));
                        }
                    }
                    if !is_subsequence(&transition.ordered_calls, &facts.calls) {
                        missing.push(format!(
                            "Rust calls do not contain required order [{}]",
                            transition.ordered_calls.join(" -> ")
                        ));
                    }
                    for fragment in &transition.required_string_fragments {
                        if !facts.strings.iter().any(|value| value.contains(fragment)) {
                            missing.push(format!("missing Rust string fragment `{fragment}`"));
                        }
                    }
                    for fragment in &transition.required_syntax_fragments {
                        if !facts.syntax.contains(&compact_syntax(fragment)) {
                            missing.push(format!("missing Rust AST fragment `{fragment}`"));
                        }
                    }
                    for action in &transition.model_actions {
                        if !model_actions.contains(action) {
                            missing.push(format!("missing Quint action `{action}`"));
                        }
                    }
                    if missing.is_empty() {
                        VerificationStatus::Pass
                    } else {
                        VerificationStatus::Fail
                    }
                }
            },
        };
        if status != VerificationStatus::Pass {
            diagnostics.extend(
                missing
                    .iter()
                    .map(|item| format!("{}: {item}", transition.id)),
            );
        }
        assessments.push(TransitionAssessment {
            id: transition.id.clone(),
            rust_file: roots.project_identity(&path),
            rust_function: transition.rust.function.clone(),
            model_actions: transition.model_actions.clone(),
            observed_calls,
            missing_requirements: missing,
            status,
            authority: transition.authority.clone(),
            does_not_prove: transition.does_not_prove.clone(),
        });
    }

    let result_at_execution = aggregate(&assessments);
    Ok(CorrespondenceEvidence {
        id: EvidenceId("pending".into()),
        obligation: ProofObligationId(format!("CORRESPONDENCE:{}", definition.id)),
        correspondence_id: definition.id.clone(),
        provider: PROVIDER_NAME.into(),
        provider_version: PROVIDER_VERSION.into(),
        claim: definition.claim.clone(),
        authority: definition.authority.clone(),
        does_not_prove: definition.does_not_prove.clone(),
        configuration_sha256,
        inputs,
        transitions: assessments,
        result_at_execution,
        diagnostics,
        recorded_at_unix_nanos: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
    })
}

fn aggregate(assessments: &[TransitionAssessment]) -> VerificationStatus {
    if assessments
        .iter()
        .any(|item| item.status == VerificationStatus::Error)
    {
        VerificationStatus::Error
    } else if assessments
        .iter()
        .any(|item| item.status == VerificationStatus::Fail)
    {
        VerificationStatus::Fail
    } else if !assessments.is_empty()
        && assessments
            .iter()
            .all(|item| item.status == VerificationStatus::Pass)
    {
        VerificationStatus::Pass
    } else {
        VerificationStatus::Unverified
    }
}

fn function_facts(file: &syn::File, selector: &str) -> Result<FunctionFacts, String> {
    let mut matches = Vec::<&syn::Block>::new();
    for item in &file.items {
        match item {
            syn::Item::Fn(function) if function.sig.ident == selector => {
                matches.push(&function.block);
            }
            syn::Item::Impl(item_impl) => {
                let owner = impl_owner(&item_impl.self_ty);
                for item in &item_impl.items {
                    if let syn::ImplItem::Fn(function) = item {
                        let qualified = owner
                            .as_ref()
                            .map(|owner| format!("{owner}::{}", function.sig.ident));
                        if function.sig.ident == selector || qualified.as_deref() == Some(selector)
                        {
                            matches.push(&function.block);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    match matches.as_slice() {
        [] => Err(format!("missing Rust function `{selector}`")),
        [block] => {
            let mut visitor = FactVisitor::default();
            visitor.visit_block(block);
            Ok(FunctionFacts {
                calls: visitor.calls,
                strings: visitor.strings,
                syntax: compact_syntax(&block.to_token_stream().to_string()),
            })
        }
        _ => Err(format!("ambiguous Rust function selector `{selector}`")),
    }
}

fn impl_owner(value: &syn::Type) -> Option<String> {
    let syn::Type::Path(path) = value else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_subsequence(required: &[String], observed: &[String]) -> bool {
    let mut required = required.iter();
    let Some(mut current) = required.next() else {
        return true;
    };
    for observed in observed {
        if observed == current {
            let Some(next) = required.next() else {
                return true;
            };
            current = next;
        }
    }
    false
}

fn quint_actions(model: &str) -> BTreeSet<String> {
    model
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix("action ")?
                .split(|character: char| character.is_whitespace() || character == '=')
                .next()
                .map(str::to_owned)
        })
        .collect()
}

fn compact_syntax(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn current_inputs(
    roots: &VerificationRoots,
    definition: &CorrespondenceDefinition,
) -> Result<Vec<InputFingerprint>, Error> {
    let mut semantic = vec![
        SemanticInput {
            identity: roots.spec_identity(&definition.source),
            path: definition.source.clone(),
        },
        SemanticInput {
            identity: roots.spec_identity(&roots.specification_root.join(&definition.model)),
            path: roots.specification_root.join(&definition.model),
        },
    ];
    for file in definition
        .transitions
        .iter()
        .map(|transition| &transition.rust.file)
        .collect::<BTreeSet<_>>()
    {
        let path = roots.project_root.join(file);
        semantic.push(SemanticInput {
            identity: roots.project_identity(&path),
            path,
        });
    }
    let mut inputs =
        crate::evidence::fingerprint_semantic_files(&semantic).map_err(|source| Error::Io {
            path: roots.project_root.clone(),
            source,
        })?;
    inputs.sort_by(|a, b| a.source.cmp(&b.source));
    Ok(inputs)
}

fn definition_fingerprint(definition: &CorrespondenceDefinition) -> String {
    hash(
        &serde_json::to_vec(&(PROVIDER_VERSION, definition))
            .expect("correspondence definition serialization"),
    )
}

pub fn store(
    directory: &Path,
    mut evidence: CorrespondenceEvidence,
) -> Result<CorrespondenceEvidence, Error> {
    fs::create_dir_all(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let seed = serde_json::to_vec(&evidence).expect("correspondence evidence serialization");
    evidence.id = EvidenceId(format!("CORRESPONDENCE-EVIDENCE:{}", &hash(&seed)[..24]));
    let target = directory.join(format!("{}.json", evidence.id.0));
    if !target.exists() {
        let temporary = directory.join(format!(".{}.tmp", evidence.id.0));
        fs::write(
            &temporary,
            serde_json::to_vec_pretty(&evidence).expect("correspondence evidence serialization"),
        )
        .map_err(|source| Error::Io {
            path: temporary.clone(),
            source,
        })?;
        fs::rename(&temporary, &target).map_err(|source| Error::Io {
            path: target,
            source,
        })?;
    }
    Ok(evidence)
}

pub fn load_all(directory: &Path) -> Result<Vec<CorrespondenceEvidence>, Error> {
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = fs::read_dir(directory)
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?
        .map(|entry| entry.map(|value| value.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
    paths.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    paths.sort();
    let mut evidence = paths
        .into_iter()
        .map(|path| {
            serde_json::from_slice::<CorrespondenceEvidence>(&fs::read(&path).map_err(
                |source| Error::Io {
                    path: path.clone(),
                    source,
                },
            )?)
            .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))
        })
        .collect::<Result<Vec<_>, _>>()?;
    evidence
        .sort_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)));
    Ok(evidence)
}

pub fn assess(
    roots: &VerificationRoots,
    definition: &CorrespondenceDefinition,
    evidence: &CorrespondenceEvidence,
) -> Result<EvidenceValidity, Error> {
    Ok(
        if evidence.provider_version == PROVIDER_VERSION
            && evidence.configuration_sha256 == definition_fingerprint(definition)
            && evidence.inputs == current_inputs(roots, definition)?
        {
            EvidenceValidity::Current
        } else {
            EvidenceValidity::Stale
        },
    )
}

pub fn latest_assessment(
    roots: &VerificationRoots,
    definition: &CorrespondenceDefinition,
) -> Result<Option<CorrespondenceAssessment>, Error> {
    let Some(evidence) = load_all(&roots.state_root.join("correspondence-evidence"))?
        .into_iter()
        .filter(|evidence| evidence.correspondence_id == definition.id)
        .max_by(|a, b| (a.recorded_at_unix_nanos, &a.id).cmp(&(b.recorded_at_unix_nanos, &b.id)))
    else {
        return Ok(None);
    };
    Ok(Some(CorrespondenceAssessment {
        current_validity: assess(roots, definition, &evidence)?,
        evidence,
    }))
}

pub fn graph_edges(
    roots: &VerificationRoots,
    definitions: &[CorrespondenceDefinition],
) -> Result<Vec<GraphEdge>, Error> {
    let evidence = load_all(&roots.state_root.join("correspondence-evidence"))?;
    let mut edges = Vec::new();
    for definition in definitions {
        let obligation = GraphNode::ProofObligation(ProofObligationId(format!(
            "CORRESPONDENCE:{}",
            definition.id
        )));
        edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(roots.spec_identity(&definition.source))),
            kind: LinkKind::Defines,
            to: obligation.clone(),
        });
        edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(
                roots.spec_identity(&roots.specification_root.join(&definition.model)),
            )),
            kind: LinkKind::RelevantTo,
            to: obligation.clone(),
        });
        for file in definition
            .transitions
            .iter()
            .map(|transition| &transition.rust.file)
            .collect::<BTreeSet<_>>()
        {
            edges.push(GraphEdge {
                from: GraphNode::Artifact(ArtifactId(
                    roots.project_identity(&roots.project_root.join(file)),
                )),
                kind: LinkKind::RelevantTo,
                to: obligation.clone(),
            });
        }
        for item in evidence
            .iter()
            .filter(|item| item.correspondence_id == definition.id)
        {
            edges.push(GraphEdge {
                from: obligation.clone(),
                kind: LinkKind::EvidenceFor,
                to: GraphNode::Evidence(item.id.clone()),
            });
        }
    }
    edges.sort_by_key(|edge| {
        serde_json::to_string(edge).expect("correspondence graph serialization")
    });
    edges.dedup();
    Ok(edges)
}

pub fn write_graph(
    roots: &VerificationRoots,
    definitions: &[CorrespondenceDefinition],
) -> Result<PathBuf, Error> {
    fs::create_dir_all(&roots.state_root).map_err(|source| Error::Io {
        path: roots.state_root.clone(),
        source,
    })?;
    let target = roots.state_root.join("correspondence-graph.json");
    let temporary = roots.state_root.join(".correspondence-graph.json.tmp");
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&graph_edges(roots, definitions)?)
            .expect("correspondence graph serialization"),
    )
    .map_err(|source| Error::Io {
        path: temporary.clone(),
        source,
    })?;
    fs::rename(&temporary, &target).map_err(|source| Error::Io {
        path: target.clone(),
        source,
    })?;
    Ok(target)
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
