use crate::{Decl, Expr, SourceSpan};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub String);
        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }
        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.into())
            }
        }
    };
}
id_type!(ArtifactId);
id_type!(DecisionId);
id_type!(ConstraintId);
id_type!(FactId);
id_type!(ProofObligationId);
id_type!(EvidenceId);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceKind {
    HumanAuthored,
    DeterministicallyExtracted,
    Authoritative,
    LlmDerived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub kind: ProvenanceKind,
    pub source: PathBuf,
    pub span: Option<SourceSpan>,
    pub extractor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Applicability {
    pub scopes: Vec<String>,
    pub effective_since: Option<String>,
    pub effective_until: Option<String>,
    pub version: Option<String>,
    pub feature: Option<String>,
    pub target: Option<String>,
    pub jurisdiction: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub kind: String,
    pub provenance: Provenance,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    pub id: DecisionId,
    pub status: String,
    pub provenance: Provenance,
    pub applicability: Applicability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntentConstraint {
    pub id: ConstraintId,
    pub decision: DecisionId,
    pub description: String,
    pub formula: RelationalFormula,
    pub provenance: Provenance,
    pub applicability: Applicability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectFact {
    pub id: FactId,
    pub relation: String,
    pub arguments: Vec<String>,
    pub value: bool,
    pub attributes: BTreeMap<String, String>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldAssumption {
    Closed,
    Partial,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "name", rename_all = "snake_case")]
pub enum CoverageScope {
    #[default]
    Global,
    Schema(String),
    Table(String),
    MaterializedView(String),
    Package(String),
}

impl CoverageScope {
    pub fn covers(&self, requested: &Self) -> bool {
        match (self, requested) {
            (Self::Global, _) => true,
            (Self::Schema(expected), Self::Schema(actual)) => expected == actual,
            (Self::Schema(expected), Self::Table(actual))
            | (Self::Schema(expected), Self::MaterializedView(actual)) => actual
                .split_once('.')
                .is_some_and(|(schema, _)| schema == expected),
            (Self::Table(expected), Self::Table(actual))
            | (Self::MaterializedView(expected), Self::MaterializedView(actual))
            | (Self::Package(expected), Self::Package(actual)) => expected == actual,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageDiagnostic {
    pub reason: String,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactCoverage {
    pub relation: String,
    pub provider: String,
    pub world: WorldAssumption,
    #[serde(default)]
    pub scope: CoverageScope,
    pub qualifiers: BTreeMap<String, String>,
    pub statement: String,
    #[serde(default)]
    pub diagnostics: Vec<CoverageDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalFormula {
    Bool(bool),
    Name(String),
    Relation(String, Vec<String>),
    Eq(String, String),
    Ne(String, String),
    Not(Box<Self>),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Implies(Box<Self>, Box<Self>),
    Forall {
        variable: String,
        entity_type: String,
        guard: Option<Box<Self>>,
        body: Box<Self>,
    },
    Exists {
        variable: String,
        entity_type: String,
        guard: Option<Box<Self>>,
        body: Box<Self>,
    },
}

impl From<&Expr> for RelationalFormula {
    fn from(value: &Expr) -> Self {
        match value {
            Expr::Bool(v) => Self::Bool(*v),
            Expr::Name(v) => Self::Name(v.clone()),
            Expr::Call(n, a) => Self::Relation(n.clone(), a.clone()),
            Expr::Eq(a, b) => Self::Eq(a.clone(), b.clone()),
            Expr::Ne(a, b) => Self::Ne(a.clone(), b.clone()),
            Expr::Not(x) => Self::Not(Box::new(Self::from(x.as_ref()))),
            Expr::And(a, b) => Self::And(
                Box::new(Self::from(a.as_ref())),
                Box::new(Self::from(b.as_ref())),
            ),
            Expr::Or(a, b) => Self::Or(
                Box::new(Self::from(a.as_ref())),
                Box::new(Self::from(b.as_ref())),
            ),
            Expr::Implies(a, b) => Self::Implies(
                Box::new(Self::from(a.as_ref())),
                Box::new(Self::from(b.as_ref())),
            ),
            Expr::Forall {
                var,
                ty,
                guard,
                body,
            } => Self::Forall {
                variable: var.clone(),
                entity_type: ty.clone(),
                guard: guard.as_deref().map(Self::from).map(Box::new),
                body: Box::new(Self::from(body.as_ref())),
            },
            Expr::Exists {
                var,
                ty,
                guard,
                body,
            } => Self::Exists {
                variable: var.clone(),
                entity_type: ty.clone(),
                guard: guard.as_deref().map(Self::from).map(Box::new),
                body: Box::new(Self::from(body.as_ref())),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphNode {
    Artifact(ArtifactId),
    Decision(DecisionId),
    Constraint(ConstraintId),
    Fact(FactId),
    ProofObligation(ProofObligationId),
    Evidence(EvidenceId),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    Contains,
    Defines,
    Produces,
    RelevantTo,
    ParticipatesIn,
    Implements,
    DependsOn,
    Supersedes,
    Amends,
    ExceptionTo,
    VerifiedBy,
    DerivedFrom,
    EvidenceFor,
    Requires,
    RequiredBy,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: GraphNode,
    pub kind: LinkKind,
    pub to: GraphNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectModel {
    pub artifacts: BTreeMap<ArtifactId, Artifact>,
    pub decisions: BTreeMap<DecisionId, Decision>,
    pub constraints: BTreeMap<ConstraintId, IntentConstraint>,
    pub facts: BTreeMap<FactId, ProjectFact>,
    pub declarations: Vec<IntentDeclaration>,
    pub fact_coverage: Vec<FactCoverage>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentDeclaration {
    Bool(String),
    EntityType {
        name: String,
        members: BTreeSet<String>,
    },
    Relation {
        name: String,
        arguments: Vec<String>,
    },
}

impl ProjectModel {
    pub fn coverage_for(&self, relation: &str, scope: &CoverageScope) -> Option<WorldAssumption> {
        let matching = self
            .fact_coverage
            .iter()
            .filter(|coverage| coverage.relation == relation && coverage.scope.covers(scope))
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return None;
        }
        Some(
            if matching
                .iter()
                .any(|coverage| coverage.world == WorldAssumption::Partial)
            {
                WorldAssumption::Partial
            } else {
                WorldAssumption::Closed
            },
        )
    }

    pub fn normalize(&mut self) {
        self.edges
            .sort_by_key(|e| serde_json::to_string(e).expect("graph edge serialization"));
        self.edges.dedup();
    }
    pub fn add_facts(&mut self, facts: impl IntoIterator<Item = ProjectFact>) {
        for fact in facts {
            self.edges.push(GraphEdge {
                from: GraphNode::Artifact(ArtifactId(fact.provenance.source.display().to_string())),
                kind: LinkKind::Produces,
                to: GraphNode::Fact(fact.id.clone()),
            });
            self.facts.insert(fact.id.clone(), fact);
        }
        self.normalize();
    }
}

pub fn declarations_from(input: &[Decl]) -> Vec<IntentDeclaration> {
    let mut out = Vec::new();
    for d in input {
        match d {
            Decl::Bool(n) => out.push(IntentDeclaration::Bool(n.clone())),
            Decl::EntityType { name, members } => out.push(IntentDeclaration::EntityType {
                name: name.clone(),
                members: members.iter().cloned().collect(),
            }),
            Decl::Relation { name, args } => out.push(IntentDeclaration::Relation {
                name: name.clone(),
                arguments: args.clone(),
            }),
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct RelationalProofObligation {
    pub id: ProofObligationId,
    pub model: ProjectModel,
}
