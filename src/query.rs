use crate::evidence::{self, EvidenceValidity, VerificationStatus};
use crate::project::{GraphEdge, GraphNode, ProjectModel};
use crate::roots::{RootsView, VerificationRoots};
use crate::{Error, load_project_model_with_roots, relational_obligation};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct EvidenceView {
    pub id: String,
    pub result_at_execution: VerificationStatus,
    pub current_validity: EvidenceValidity,
    pub backend: String,
    pub backend_version: String,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct QueryReport {
    pub roots: RootsView,
    pub subject: String,
    pub provenance: Vec<String>,
    pub paths: Vec<Vec<String>>,
    pub evidence: Vec<EvidenceView>,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StatusReport {
    pub roots: RootsView,
    pub current: BTreeMap<String, usize>,
    pub latest_evidence: Vec<EvidenceView>,
    pub unverified_constraints: Vec<String>,
    pub unverified_intent: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HeterogeneousImpactReport {
    pub roots: RootsView,
    pub subject: String,
    pub paths: Vec<Vec<String>>,
    pub affected_obligations: Vec<String>,
    pub affected_evidence: Vec<String>,
    pub affected_parents: Vec<String>,
    pub relational_evidence: Vec<EvidenceView>,
}

fn label(node: &GraphNode) -> String {
    match node {
        GraphNode::Artifact(id) => format!("artifact:{}", id.0),
        GraphNode::Decision(id) => format!("decision:{}", id.0),
        GraphNode::Constraint(id) => format!("constraint:{}", id.0),
        GraphNode::Fact(id) => format!("fact:{}", id.0),
        GraphNode::ProofObligation(id) => format!("obligation:{}", id.0),
        GraphNode::Evidence(id) => format!("evidence:{}", id.0),
    }
}
fn edge_label(edge: &GraphEdge) -> String {
    format!(
        "{} --{:?}--> {}",
        label(&edge.from),
        edge.kind,
        label(&edge.to)
    )
}
fn context(
    roots: &VerificationRoots,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<(ProjectModel, Vec<EvidenceView>), Error> {
    let (model, inputs) = load_project_model_with_roots(roots)?;
    let mut obligation = relational_obligation(model);
    let inputs = crate::relevant_semantic_inputs(&obligation.model, &inputs);
    let mut fingerprints =
        evidence::fingerprint_semantic_files(&inputs).map_err(|source| Error::Io {
            path: roots.project_root.clone(),
            source,
        })?;
    fingerprints.push(evidence::fingerprint_bytes(
        "generated:effective.smt2",
        crate::obligation_to_smt(&obligation).as_bytes(),
    ));
    fingerprints.sort_by(|a, b| a.source.cmp(&b.source));
    let configuration = evidence::configuration_hash(timeout_ms, &["smt.core.minimize=true"]);
    let history =
        evidence::load_all(&roots.state_root.join("evidence")).map_err(|source| Error::Io {
            path: roots.state_root.join("evidence"),
            source,
        })?;
    let mut views = Vec::new();
    for item in history {
        let validity = evidence::assess(&item, &fingerprints, backend_version, &configuration);
        obligation.model.edges.push(GraphEdge {
            from: GraphNode::ProofObligation(item.obligation.clone()),
            kind: crate::project::LinkKind::EvidenceFor,
            to: GraphNode::Evidence(item.id.clone()),
        });
        views.push(EvidenceView {
            id: item.id.0,
            result_at_execution: item.result_at_execution,
            current_validity: validity,
            backend: item.backend,
            backend_version: item.backend_version,
        });
    }
    obligation.model.normalize();
    Ok((obligation.model, views))
}
fn node_for(model: &ProjectModel, id: &str) -> Option<GraphNode> {
    model
        .constraints
        .keys()
        .find(|key| key.0 == id)
        .cloned()
        .map(GraphNode::Constraint)
        .or_else(|| {
            model
                .facts
                .keys()
                .find(|key| key.0 == id || format!("CARGO:{}", key.0) == id)
                .cloned()
                .map(GraphNode::Fact)
        })
        .or_else(|| {
            model
                .artifacts
                .keys()
                .find(|key| key.0 == id)
                .cloned()
                .map(GraphNode::Artifact)
        })
        .or_else(|| {
            (id == "PO:project-consistency")
                .then(|| GraphNode::ProofObligation(crate::project::ProofObligationId(id.into())))
        })
        .or_else(|| {
            id.strip_prefix("EVIDENCE:")
                .map(|_| GraphNode::Evidence(crate::project::EvidenceId(id.into())))
        })
}
fn paths_from(model: &ProjectModel, start: &GraphNode) -> Vec<Vec<String>> {
    paths_from_edges(&model.edges, start)
}

fn paths_from_edges(edges: &[GraphEdge], start: &GraphNode) -> Vec<Vec<String>> {
    let mut queue = VecDeque::from([(start.clone(), Vec::<String>::new())]);
    let mut best = BTreeMap::from([(label(start), 0usize)]);
    let mut output = Vec::new();
    while let Some((node, path)) = queue.pop_front() {
        for edge in edges.iter().filter(|edge| edge.from == node) {
            let mut next = path.clone();
            next.push(edge_label(edge));
            let key = label(&edge.to);
            if best
                .get(&key)
                .is_none_or(|distance| next.len() <= *distance)
            {
                best.insert(key, next.len());
                if matches!(
                    edge.to,
                    GraphNode::Constraint(_)
                        | GraphNode::ProofObligation(_)
                        | GraphNode::Evidence(_)
                ) {
                    output.push(next.clone());
                }
                queue.push_back((edge.to.clone(), next));
            }
        }
    }
    output.sort();
    output.dedup();
    output
}

fn reachable_nodes(edges: &[GraphEdge], start: &GraphNode) -> Vec<GraphNode> {
    let mut queue = VecDeque::from([start.clone()]);
    let mut seen = BTreeMap::from([(label(start), start.clone())]);
    while let Some(node) = queue.pop_front() {
        for edge in edges.iter().filter(|edge| edge.from == node) {
            let key = label(&edge.to);
            if let std::collections::btree_map::Entry::Vacant(entry) = seen.entry(key) {
                entry.insert(edge.to.clone());
                queue.push_back(edge.to.clone());
            }
        }
    }
    seen.into_values().collect()
}

pub fn heterogeneous_impact_with_roots(
    roots: &VerificationRoots,
    path: &Path,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<HeterogeneousImpactReport, Error> {
    let (model, relational_evidence) = context(roots, backend_version, timeout_ms)?;
    let scenarios = crate::scenario::discover(&roots.specification_root)?;
    let parents = crate::scenario::discover_parents(&roots.specification_root)?;
    let models = crate::quint::discover(&roots.specification_root)?;
    let validations = crate::quint::discover_validations(&roots.specification_root)?;
    let correspondences = crate::correspondence::discover(&roots.specification_root)?;
    let native_tests = crate::native_test::discover(&roots.specification_root)?;
    let mut edges = model.edges;
    edges.extend(crate::scenario::graph_edges(roots, &scenarios, &parents)?);
    edges.extend(crate::quint::graph_edges(roots, &models, &validations)?);
    edges.extend(crate::correspondence::graph_edges(roots, &correspondences)?);
    edges.extend(crate::native_test::graph_edges(roots, &native_tests)?);
    let reverse_parent_edges = edges
        .iter()
        .filter(|edge| edge.kind == crate::project::LinkKind::Requires)
        .map(|edge| GraphEdge {
            from: edge.to.clone(),
            kind: crate::project::LinkKind::RequiredBy,
            to: edge.from.clone(),
        })
        .collect::<Vec<_>>();
    edges.extend(reverse_parent_edges);
    edges.sort_by_key(|edge| serde_json::to_string(edge).expect("impact edge serialization"));
    edges.dedup();

    let normalized = if path.to_string_lossy().starts_with("spec:") {
        path.display().to_string()
    } else {
        roots.project_identity(path)
    };
    let start = GraphNode::Artifact(crate::project::ArtifactId(normalized.clone()));
    let reachable = reachable_nodes(&edges, &start);
    let mut affected_obligations = reachable
        .iter()
        .filter_map(|node| match node {
            GraphNode::ProofObligation(id) => Some(id.0.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let parent_ids = parents
        .iter()
        .map(|parent| parent.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let mut affected_parents = affected_obligations
        .iter()
        .filter(|id| parent_ids.contains(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut affected_evidence = reachable
        .iter()
        .filter_map(|node| match node {
            GraphNode::Evidence(id) => Some(id.0.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    affected_obligations.sort();
    affected_obligations.dedup();
    affected_parents.sort();
    affected_parents.dedup();
    affected_evidence.sort();
    affected_evidence.dedup();
    Ok(HeterogeneousImpactReport {
        roots: roots.view(),
        subject: format!("artifact:{normalized}"),
        paths: paths_from_edges(&edges, &start),
        affected_obligations,
        affected_evidence,
        affected_parents,
        relational_evidence,
    })
}
fn paths_through(model: &ProjectModel, subject: &GraphNode) -> Vec<Vec<String>> {
    let marker = format!("--> {}", label(subject));
    let mut paths = paths_from(model, subject);
    for artifact in model.artifacts.keys() {
        paths.extend(
            paths_from(model, &GraphNode::Artifact(artifact.clone()))
                .into_iter()
                .filter(|path| path.iter().any(|edge| edge.ends_with(&marker))),
        );
    }
    paths.sort();
    paths.dedup();
    paths
}
pub fn impact(
    root: &Path,
    artifacts: &Path,
    path: &Path,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<QueryReport, Error> {
    impact_with_roots(
        &VerificationRoots::legacy(root, artifacts),
        path,
        backend_version,
        timeout_ms,
    )
}
pub fn impact_with_roots(
    roots: &VerificationRoots,
    path: &Path,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<QueryReport, Error> {
    let (model, evidence) = context(roots, backend_version, timeout_ms)?;
    let normalized = if path.to_string_lossy().starts_with("spec:") {
        path.display().to_string()
    } else {
        roots.project_identity(path)
    };
    let node = GraphNode::Artifact(crate::project::ArtifactId(normalized.clone()));
    Ok(QueryReport {
        roots: roots.view(),
        subject: format!("artifact:{normalized}"),
        provenance: vec![normalized],
        paths: paths_from(&model, &node),
        evidence,
    })
}
pub fn explain(
    root: &Path,
    artifacts: &Path,
    id: &str,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<QueryReport, Error> {
    explain_with_roots(
        &VerificationRoots::legacy(root, artifacts),
        id,
        backend_version,
        timeout_ms,
    )
}
pub fn explain_with_roots(
    roots: &VerificationRoots,
    id: &str,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<QueryReport, Error> {
    let (model, evidence) = context(roots, backend_version, timeout_ms)?;
    let node = node_for(&model, id)
        .ok_or_else(|| Error::ProviderFailure(format!("unknown graph id `{id}`")))?;
    let mut provenance = Vec::new();
    match &node {
        GraphNode::Constraint(key) => {
            let item = &model.constraints[key];
            provenance.push(format!(
                "{}: {}",
                item.provenance.source.display(),
                item.description
            ));
        }
        GraphNode::Fact(key) => {
            let item = &model.facts[key];
            provenance.push(format!(
                "{}: {}({})",
                item.provenance.source.display(),
                item.relation,
                item.arguments.join(", ")
            ));
        }
        GraphNode::Artifact(key) => {
            if let Some(item) = model.artifacts.get(key) {
                provenance.push(item.provenance.source.display().to_string())
            }
        }
        _ => {}
    }
    Ok(QueryReport {
        roots: roots.view(),
        subject: label(&node),
        provenance,
        paths: paths_through(&model, &node),
        evidence,
    })
}
pub fn status(
    root: &Path,
    artifacts: &Path,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<StatusReport, Error> {
    status_with_roots(
        &VerificationRoots::legacy(root, artifacts),
        backend_version,
        timeout_ms,
    )
}
pub fn status_with_roots(
    roots: &VerificationRoots,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<StatusReport, Error> {
    let (model, views) = context(roots, backend_version, timeout_ms)?;
    let mut latest: BTreeMap<String, EvidenceView> = BTreeMap::new();
    for view in views {
        latest.insert("PO:project-consistency".into(), view);
    }
    let mut counts = BTreeMap::new();
    for view in latest.values() {
        let key = match view.current_validity {
            EvidenceValidity::Stale => "STALE",
            EvidenceValidity::Current => match view.result_at_execution {
                VerificationStatus::Pass => "PASS",
                VerificationStatus::Fail => "FAIL",
                VerificationStatus::Unknown => "UNKNOWN",
                VerificationStatus::Error => "ERROR",
                VerificationStatus::Unverified => "UNVERIFIED",
                VerificationStatus::NotApplicable => "NOT_APPLICABLE",
                VerificationStatus::Stale => "STALE",
            },
        };
        *counts.entry(key.into()).or_insert(0) += 1;
    }
    let unverified = if latest.is_empty() {
        model.constraints.keys().map(|id| id.0.clone()).collect()
    } else {
        Vec::new()
    };
    let constrained = model
        .constraints
        .values()
        .map(|constraint| constraint.decision.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let unverified_intent = model
        .decisions
        .keys()
        .filter(|decision| !constrained.contains(*decision))
        .map(|decision| decision.0.clone())
        .collect();
    Ok(StatusReport {
        roots: roots.view(),
        current: counts,
        latest_evidence: latest.into_values().collect(),
        unverified_constraints: unverified,
        unverified_intent,
    })
}
