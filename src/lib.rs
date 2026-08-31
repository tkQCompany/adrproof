use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use thiserror::Error;

pub mod bundle;
pub mod cargo_facts;
pub mod correspondence;
pub mod evidence;
pub mod native_test;
pub mod policy;
pub mod project;
pub mod query;
pub mod quint;
pub mod roots;
pub mod scenario;
pub mod sql_migrations;
use project::{
    Applicability, ArtifactId, ConstraintId, Decision, DecisionId, GraphEdge, GraphNode,
    IntentConstraint, LinkKind, ProjectModel, ProofObligationId, Provenance, ProvenanceKind,
    RelationalFormula, RelationalProofObligation,
};

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}:{line}:{column}: {message}")]
    Diagnostic {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("invalid ADR reference `{reference}` in {adr_id}: target does not exist")]
    InvalidReference { adr_id: String, reference: String },
    #[error("solver executable `{0}` was not found")]
    SolverMissing(String),
    #[error("solver version mismatch: expected `{expected}`, got `{actual}`")]
    SolverVersion { expected: String, actual: String },
    #[error("solver timed out after {0} ms")]
    Timeout(u64),
    #[error("solver failed: {0}")]
    SolverFailure(String),
    #[error("fact provider failed: {0}")]
    ProviderFailure(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Proposed,
    Accepted,
    Deprecated,
    Superseded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SourceSpan {
    pub filename: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clause {
    pub adr_id: String,
    pub id: String,
    pub description: String,
    pub span: SourceSpan,
    pub expression: Expr,
}

#[derive(Debug, Clone)]
pub struct Adr {
    pub id: String,
    pub status: Status,
    pub supersedes: Vec<String>,
    pub amends: Vec<String>,
    pub exception_to: Vec<String>,
    pub clauses: Vec<Clause>,
    pub declarations: Vec<Decl>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    Bool(String),
    EntityType { name: String, members: Vec<String> },
    Relation { name: String, args: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    Bool(bool),
    Name(String),
    Call(String, Vec<String>),
    Eq(String, String),
    Ne(String, String),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Implies(Box<Expr>, Box<Expr>),
    Forall {
        var: String,
        ty: String,
        guard: Option<Box<Expr>>,
        body: Box<Expr>,
    },
    Exists {
        var: String,
        ty: String,
        guard: Option<Box<Expr>>,
        body: Box<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct EffectiveSpecification {
    pub clauses: Vec<Clause>,
    pub declarations: Vec<Decl>,
    pub active_adrs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum Verdict {
    Sat,
    Unsat,
    Unknown,
    Timeout,
    SolverFailure,
    InvalidInput,
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckReport {
    pub roots: roots::RootsView,
    pub verdict: Verdict,
    pub evidence_status: evidence::VerificationStatus,
    pub conflicts: Vec<Conflict>,
    pub solver: String,
    pub elapsed_ms: u128,
    pub smt_artifact: PathBuf,
    pub ledger_artifact: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Conflict {
    pub adr_id: String,
    pub clause_id: String,
    pub description: String,
    pub span: SourceSpan,
    pub origin_kind: ProvenanceKind,
}

pub trait ConstraintBackend {
    fn check(
        &self,
        obligation: &RelationalProofObligation,
        artifact: &Path,
    ) -> Result<BackendResult, Error>;
}
pub trait CodeFactProvider {
    fn facts(&self) -> Result<Vec<project::ProjectFact>, Error>;
}
pub trait RustProofProvider {
    fn verify_rust(&self) -> Result<ExternalEvidence, Error>;
}
pub trait TemporalProofProvider {
    fn verify_temporal(&self) -> Result<ExternalEvidence, Error>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalEvidence {
    pub provider: String,
    pub version: String,
    pub status: Verdict,
    pub assumptions: Vec<String>,
    pub artifacts: Vec<PathBuf>,
}
#[derive(Debug, Clone)]
pub struct BackendResult {
    pub verdict: Verdict,
    pub core: Vec<String>,
    pub solver_version: String,
    pub elapsed: Duration,
    pub timeout_ms: u64,
}

fn diag(path: &Path, line: usize, column: usize, message: impl Into<String>) -> Error {
    Error::Diagnostic {
        path: path.to_path_buf(),
        line,
        column,
        message: message.into(),
    }
}

pub fn load_adrs(root: &Path) -> Result<Vec<Adr>, Error> {
    let mut files = Vec::new();
    collect_markdown(root, &mut files)?;
    files.sort();
    files.into_iter().map(|p| parse_adr(&p)).collect()
}

fn collect_markdown(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), Error> {
    if path.is_file() {
        if path.extension().is_some_and(|x| x == "md") {
            out.push(path.to_path_buf());
        }
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let p = entry
            .map_err(|source| Error::Io {
                path: path.to_path_buf(),
                source,
            })?
            .path();
        if p.file_name()
            .is_some_and(|n| n == "target" || n == ".git" || n == ".adrproof")
        {
            continue;
        }
        if p.is_dir() {
            collect_markdown(&p, out)?;
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
    Ok(())
}

fn parse_adr(path: &Path) -> Result<Adr, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let lines: Vec<_> = text.lines().collect();
    if lines.first() != Some(&"---") {
        return Err(diag(path, 1, 1, "ADR must start with YAML front matter"));
    }
    let end = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| **l == "---")
        .map(|(i, _)| i)
        .ok_or_else(|| diag(path, 1, 1, "unterminated front matter"))?;
    let mut meta: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut list_key: Option<String> = None;
    for (i, line) in lines[1..end].iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some(item) = line.trim().strip_prefix("- ") {
            let key = list_key
                .as_ref()
                .ok_or_else(|| diag(path, i + 2, 1, "list item has no preceding metadata key"))?;
            meta.entry(key.clone())
                .or_default()
                .push(item.trim().trim_matches(['\'', '"']).to_string());
            continue;
        }
        if line.starts_with(' ') {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            return Err(diag(path, i + 2, 1, "invalid front matter entry"));
        };
        let vals = v
            .trim()
            .trim_matches(['[', ']'])
            .split(',')
            .map(|x| x.trim().trim_matches(['\'', '"']).to_string())
            .filter(|x| !x.is_empty())
            .collect();
        let key = k.trim().to_string();
        list_key = Some(key.clone());
        meta.insert(key, vals);
    }
    let one = |key: &str| meta.get(key).and_then(|v| v.first()).cloned();
    let id = one("id").ok_or_else(|| diag(path, 2, 1, "missing ADR id"))?;
    let status = match one("status").as_deref() {
        Some("proposed") => Status::Proposed,
        Some("accepted") => Status::Accepted,
        Some("deprecated") => Status::Deprecated,
        Some("superseded") => Status::Superseded,
        _ => {
            return Err(diag(
                path,
                2,
                1,
                "status must be proposed, accepted, deprecated, or superseded",
            ));
        }
    };
    let mut declarations = Vec::new();
    let mut clauses = Vec::new();
    let mut i = end + 1;
    while i < lines.len() {
        if lines[i].trim() == "```adrlogic" {
            let start = i + 2;
            i += 1;
            let mut block = String::new();
            while i < lines.len() && lines[i].trim() != "```" {
                block.push_str(lines[i]);
                block.push('\n');
                i += 1;
            }
            if i == lines.len() {
                return Err(diag(path, start, 1, "unterminated adrlogic block"));
            }
            parse_logic(&block, path, start, &id, &mut declarations, &mut clauses)?;
        }
        i += 1;
    }
    Ok(Adr {
        id,
        status,
        supersedes: meta.remove("supersedes").unwrap_or_default(),
        amends: meta.remove("amends").unwrap_or_default(),
        exception_to: meta.remove("exception_to").unwrap_or_default(),
        clauses,
        declarations,
    })
}

fn parse_logic(
    src: &str,
    path: &Path,
    base_line: usize,
    adr_id: &str,
    decls: &mut Vec<Decl>,
    clauses: &mut Vec<Clause>,
) -> Result<(), Error> {
    let mut offset = 0;
    while offset < src.len() {
        while offset < src.len() && src.as_bytes()[offset].is_ascii_whitespace() {
            offset += 1;
        }
        if offset == src.len() {
            break;
        }
        let line = base_line + src[..offset].bytes().filter(|b| *b == b'\n').count();
        if src[offset..].starts_with("bool ") {
            let end = src[offset..]
                .find(';')
                .ok_or_else(|| diag(path, line, 1, "missing `;`"))?
                + offset;
            let name = src[offset + 5..end].trim();
            validate_ident(name, path, line)?;
            decls.push(Decl::Bool(name.into()));
            offset = end + 1;
        } else if src[offset..].starts_with("entity ") {
            let end = src[offset..]
                .find(';')
                .ok_or_else(|| diag(path, line, 1, "missing `;`"))?
                + offset;
            let body = src[offset + 7..end].trim();
            let (name, members) = body
                .split_once('{')
                .ok_or_else(|| diag(path, line, 1, "entity syntax: entity Type { a, b };"))?;
            let members = members
                .trim_end_matches('}')
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
            decls.push(Decl::EntityType {
                name: name.trim().into(),
                members,
            });
            offset = end + 1;
        } else if src[offset..].starts_with("relation ") {
            let end = src[offset..]
                .find(';')
                .ok_or_else(|| diag(path, line, 1, "missing `;`"))?
                + offset;
            let body = src[offset + 9..end].trim();
            let open = body
                .find('(')
                .ok_or_else(|| diag(path, line, 1, "missing relation argument list"))?;
            let args = body[open + 1..]
                .trim_end_matches(')')
                .split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect();
            decls.push(Decl::Relation {
                name: body[..open].trim().into(),
                args,
            });
            offset = end + 1;
        } else if src[offset..].starts_with("rule ") {
            let open = src[offset..]
                .find('{')
                .ok_or_else(|| diag(path, line, 1, "missing rule body"))?
                + offset;
            let close = matching_brace(src, open)
                .ok_or_else(|| diag(path, line, 1, "unterminated rule body"))?;
            let header = src[offset + 5..open].trim();
            let (id, description) = parse_rule_header(header, path, line)?;
            let expr_text = src[open + 1..close].trim().trim_end_matches(';').trim();
            let expression = Parser::new(expr_text, path, line).parse()?;
            clauses.push(Clause {
                adr_id: adr_id.into(),
                id,
                description,
                span: SourceSpan {
                    filename: path.to_path_buf(),
                    line,
                    column: 1,
                },
                expression,
            });
            offset = close + 1;
        } else {
            return Err(diag(
                path,
                line,
                1,
                "expected bool, entity, relation, or rule",
            ));
        }
    }
    Ok(())
}

fn validate_ident(s: &str, path: &Path, line: usize) -> Result<(), Error> {
    if s.is_empty() || !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Err(diag(path, line, 1, format!("invalid identifier `{s}`")))
    } else {
        Ok(())
    }
}
fn matching_brace(s: &str, open: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, c) in s[open..].char_indices() {
        if c == '{' {
            depth += 1;
        }
        if c == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(open + i);
            }
        }
    }
    None
}
fn parse_rule_header(h: &str, path: &Path, line: usize) -> Result<(String, String), Error> {
    let q = h
        .find('"')
        .ok_or_else(|| diag(path, line, 1, "rule requires a quoted description"))?;
    let end = h
        .rfind('"')
        .filter(|e| *e > q)
        .ok_or_else(|| diag(path, line, 1, "unterminated rule description"))?;
    Ok((h[..q].trim().into(), h[q + 1..end].into()))
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Id(String),
    True,
    False,
    Not,
    And,
    Or,
    Imp,
    Eq,
    Ne,
    Lp,
    Rp,
    Comma,
    Colon,
    Forall,
    Exists,
    Where,
}
struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    path: &'a Path,
    line: usize,
}
impl<'a> Parser<'a> {
    fn new(s: &str, path: &'a Path, line: usize) -> Self {
        Self {
            toks: lex(s),
            pos: 0,
            path,
            line,
        }
    }
    fn parse(mut self) -> Result<Expr, Error> {
        let e = self.imp()?;
        if self.pos != self.toks.len() {
            return Err(diag(
                self.path,
                self.line,
                1,
                "unexpected token in expression",
            ));
        }
        Ok(e)
    }
    fn imp(&mut self) -> Result<Expr, Error> {
        let l = self.or()?;
        if self.eat(&Tok::Imp) {
            Ok(Expr::Implies(Box::new(l), Box::new(self.imp()?)))
        } else {
            Ok(l)
        }
    }
    fn or(&mut self) -> Result<Expr, Error> {
        let mut e = self.and()?;
        while self.eat(&Tok::Or) {
            e = Expr::Or(Box::new(e), Box::new(self.and()?));
        }
        Ok(e)
    }
    fn and(&mut self) -> Result<Expr, Error> {
        let mut e = self.unary()?;
        while self.eat(&Tok::And) {
            e = Expr::And(Box::new(e), Box::new(self.unary()?));
        }
        Ok(e)
    }
    fn unary(&mut self) -> Result<Expr, Error> {
        if self.eat(&Tok::Not) {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if matches!(self.peek(), Some(Tok::Forall | Tok::Exists)) {
            let forall = self.eat(&Tok::Forall);
            if !forall {
                self.expect(Tok::Exists)?;
            }
            let var = self.id()?;
            self.expect(Tok::Colon)?;
            let ty = self.id()?;
            let guard = if self.eat(&Tok::Where) {
                Some(Box::new(self.imp()?))
            } else {
                None
            };
            self.expect(Tok::Colon)?;
            let body = self.imp()?;
            return Ok(if forall {
                Expr::Forall {
                    var,
                    ty,
                    guard,
                    body: Box::new(body),
                }
            } else {
                Expr::Exists {
                    var,
                    ty,
                    guard,
                    body: Box::new(body),
                }
            });
        }
        self.atom()
    }
    fn atom(&mut self) -> Result<Expr, Error> {
        if self.eat(&Tok::Lp) {
            let e = self.imp()?;
            self.expect(Tok::Rp)?;
            return Ok(e);
        }
        if self.eat(&Tok::True) {
            return Ok(Expr::Bool(true));
        }
        if self.eat(&Tok::False) {
            return Ok(Expr::Bool(false));
        }
        let name = self.id()?;
        if self.eat(&Tok::Lp) {
            let mut a = Vec::new();
            if !self.eat(&Tok::Rp) {
                loop {
                    a.push(self.id()?);
                    if self.eat(&Tok::Rp) {
                        break;
                    }
                    self.expect(Tok::Comma)?;
                }
            }
            return Ok(Expr::Call(name, a));
        }
        if self.eat(&Tok::Eq) {
            return Ok(Expr::Eq(name, self.id()?));
        }
        if self.eat(&Tok::Ne) {
            return Ok(Expr::Ne(name, self.id()?));
        }
        Ok(Expr::Name(name))
    }
    fn id(&mut self) -> Result<String, Error> {
        match self.toks.get(self.pos).cloned() {
            Some(Tok::Id(x)) => {
                self.pos += 1;
                Ok(x)
            }
            _ => Err(diag(self.path, self.line, 1, "expected identifier")),
        }
    }
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, t: Tok) -> Result<(), Error> {
        if self.eat(&t) {
            Ok(())
        } else {
            Err(diag(self.path, self.line, 1, format!("expected {t:?}")))
        }
    }
}
fn lex(s: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut i = 0;
    let b = s.as_bytes();
    while i < b.len() {
        if b[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let rest = &s[i..];
        let (t, n) = if rest.starts_with("&&") {
            (Tok::And, 2)
        } else if rest.starts_with("||") {
            (Tok::Or, 2)
        } else if rest.starts_with("=>") {
            (Tok::Imp, 2)
        } else if rest.starts_with("!=") {
            (Tok::Ne, 2)
        } else if rest.starts_with("==") {
            (Tok::Eq, 2)
        } else {
            match b[i] {
                b'!' => (Tok::Not, 1),
                b'(' => (Tok::Lp, 1),
                b')' => (Tok::Rp, 1),
                b',' => (Tok::Comma, 1),
                b':' => (Tok::Colon, 1),
                _ => {
                    let mut j = i;
                    while j < b.len()
                        && (b[j].is_ascii_alphanumeric()
                            || b[j] == b'_'
                            || b[j] == b'-'
                            || b[j] == b'.')
                    {
                        j += 1
                    }
                    if j == i {
                        i += 1;
                        continue;
                    }
                    let w = &s[i..j];
                    let t = match w {
                        "true" => Tok::True,
                        "false" => Tok::False,
                        "forall" => Tok::Forall,
                        "exists" => Tok::Exists,
                        "where" => Tok::Where,
                        "and" => Tok::And,
                        "or" => Tok::Or,
                        "not" => Tok::Not,
                        "implies" => Tok::Imp,
                        _ => Tok::Id(w.into()),
                    };
                    out.push(t);
                    i = j;
                    continue;
                }
            }
        };
        out.push(t);
        i += n
    }
    out
}

pub fn effective(adrs: &[Adr], include_proposed: bool) -> Result<EffectiveSpecification, Error> {
    let ids: BTreeSet<_> = adrs.iter().map(|a| a.id.as_str()).collect();
    for a in adrs {
        for r in a.supersedes.iter().chain(&a.amends).chain(&a.exception_to) {
            if !ids.contains(r.as_str()) {
                return Err(Error::InvalidReference {
                    adr_id: a.id.clone(),
                    reference: r.clone(),
                });
            }
        }
    }
    let superseded: BTreeSet<_> = adrs
        .iter()
        .filter(|a| matches!(a.status, Status::Accepted))
        .flat_map(|a| a.supersedes.iter().cloned())
        .collect();
    let mut active: Vec<_> = adrs
        .iter()
        .filter(|a| {
            !superseded.contains(&a.id)
                && (matches!(a.status, Status::Accepted)
                    || include_proposed && matches!(a.status, Status::Proposed))
        })
        .collect();
    active.sort_by(|a, b| a.id.cmp(&b.id));
    let declarations = active.iter().flat_map(|a| a.declarations.clone()).collect();
    let mut clauses: Vec<_> = active.iter().flat_map(|a| a.clauses.clone()).collect();
    clauses.sort_by(|a, b| (&a.adr_id, &a.id, &a.span).cmp(&(&b.adr_id, &b.id, &b.span)));
    let spec = EffectiveSpecification {
        clauses,
        declarations,
        active_adrs: active.iter().map(|a| a.id.clone()).collect(),
    };
    typecheck(&spec)?;
    Ok(spec)
}

/// Lowers the ADR frontend output into the backend-neutral semantic model.
pub fn lower_to_project_model(adrs: &[Adr], spec: &EffectiveSpecification) -> ProjectModel {
    let mut model = ProjectModel {
        declarations: project::declarations_from(&spec.declarations),
        ..ProjectModel::default()
    };
    for adr in adrs {
        let id = DecisionId(adr.id.clone());
        let source = adr
            .clauses
            .first()
            .map(|c| c.span.filename.clone())
            .unwrap_or_default();
        let provenance = Provenance {
            kind: ProvenanceKind::HumanAuthored,
            source: source.clone(),
            span: None,
            extractor: None,
        };
        model.decisions.insert(
            id.clone(),
            Decision {
                id: id.clone(),
                status: format!("{:?}", adr.status).to_lowercase(),
                provenance: provenance.clone(),
                applicability: Applicability::default(),
            },
        );
        model.artifacts.insert(
            ArtifactId(source.display().to_string()),
            project::Artifact {
                id: ArtifactId(source.display().to_string()),
                kind: "adr_markdown".into(),
                provenance,
            },
        );
        for target in &adr.supersedes {
            model.edges.push(GraphEdge {
                from: GraphNode::Decision(id.clone()),
                kind: LinkKind::Supersedes,
                to: GraphNode::Decision(DecisionId(target.clone())),
            });
        }
        for target in &adr.amends {
            model.edges.push(GraphEdge {
                from: GraphNode::Decision(id.clone()),
                kind: LinkKind::Amends,
                to: GraphNode::Decision(DecisionId(target.clone())),
            });
        }
        for target in &adr.exception_to {
            model.edges.push(GraphEdge {
                from: GraphNode::Decision(id.clone()),
                kind: LinkKind::ExceptionTo,
                to: GraphNode::Decision(DecisionId(target.clone())),
            });
        }
    }
    for clause in &spec.clauses {
        let id = ConstraintId(format!("{}:{}", clause.adr_id, clause.id));
        model.constraints.insert(
            id.clone(),
            IntentConstraint {
                id: id.clone(),
                decision: DecisionId(clause.adr_id.clone()),
                description: clause.description.clone(),
                formula: RelationalFormula::from(&clause.expression),
                provenance: Provenance {
                    kind: ProvenanceKind::HumanAuthored,
                    source: clause.span.filename.clone(),
                    span: Some(clause.span.clone()),
                    extractor: None,
                },
                applicability: Applicability::default(),
            },
        );
        model.edges.push(GraphEdge {
            from: GraphNode::Decision(DecisionId(clause.adr_id.clone())),
            kind: LinkKind::Contains,
            to: GraphNode::Constraint(id),
        });
        model.edges.push(GraphEdge {
            from: GraphNode::Artifact(ArtifactId(clause.span.filename.display().to_string())),
            kind: LinkKind::Defines,
            to: GraphNode::Constraint(ConstraintId(format!("{}:{}", clause.adr_id, clause.id))),
        });
    }
    model.normalize();
    model
}

pub fn relational_obligation(mut model: ProjectModel) -> RelationalProofObligation {
    let obligation_id = ProofObligationId("PO:project-consistency".into());
    for constraint in model.constraints.keys() {
        model.edges.push(GraphEdge {
            from: GraphNode::Constraint(constraint.clone()),
            kind: LinkKind::ParticipatesIn,
            to: GraphNode::ProofObligation(obligation_id.clone()),
        });
    }
    for fact in model.facts.values() {
        for constraint in model.constraints.values() {
            if formula_references_fact(&constraint.formula, fact) {
                model.edges.push(GraphEdge {
                    from: GraphNode::Fact(fact.id.clone()),
                    kind: LinkKind::RelevantTo,
                    to: GraphNode::Constraint(constraint.id.clone()),
                });
            }
        }
    }
    model.normalize();
    RelationalProofObligation {
        id: obligation_id,
        model,
    }
}

fn formula_references_fact(formula: &RelationalFormula, fact: &project::ProjectFact) -> bool {
    fn visit(
        formula: &RelationalFormula,
        fact: &project::ProjectFact,
        variables: &BTreeSet<String>,
    ) -> bool {
        match formula {
            RelationalFormula::Relation(name, arguments) => {
                name == &fact.relation
                    && arguments.len() == fact.arguments.len()
                    && arguments
                        .iter()
                        .zip(&fact.arguments)
                        .all(|(pattern, value)| variables.contains(pattern) || pattern == value)
            }
            RelationalFormula::Not(value) => visit(value, fact, variables),
            RelationalFormula::And(a, b)
            | RelationalFormula::Or(a, b)
            | RelationalFormula::Implies(a, b) => {
                visit(a, fact, variables) || visit(b, fact, variables)
            }
            RelationalFormula::Forall {
                variable,
                guard,
                body,
                ..
            }
            | RelationalFormula::Exists {
                variable,
                guard,
                body,
                ..
            } => {
                let mut variables = variables.clone();
                variables.insert(variable.clone());
                guard
                    .as_deref()
                    .is_some_and(|guard| visit(guard, fact, &variables))
                    || visit(body, fact, &variables)
            }
            _ => false,
        }
    }
    visit(formula, fact, &BTreeSet::new())
}

fn typecheck(spec: &EffectiveSpecification) -> Result<(), Error> {
    let mut bools = BTreeSet::new();
    let mut entities: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut rels: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for d in &spec.declarations {
        match d {
            Decl::Bool(n) => {
                bools.insert(n.clone());
            }
            Decl::EntityType { name, members } => {
                entities
                    .entry(name.clone())
                    .or_default()
                    .extend(members.iter().cloned());
            }
            Decl::Relation { name, args } => {
                rels.insert(name.clone(), args.clone());
            }
        }
    }
    for c in &spec.clauses {
        check_expr(
            &c.expression,
            &bools,
            &entities,
            &rels,
            &mut BTreeMap::new(),
        )
        .map_err(|m| diag(&c.span.filename, c.span.line, c.span.column, m))?;
    }
    Ok(())
}
fn check_expr(
    e: &Expr,
    bools: &BTreeSet<String>,
    ents: &BTreeMap<String, BTreeSet<String>>,
    rels: &BTreeMap<String, Vec<String>>,
    env: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    match e {
        Expr::Bool(_) => Ok(()),
        Expr::Name(n) => {
            if bools.contains(n) {
                Ok(())
            } else {
                Err(format!("`{n}` is not a Bool"))
            }
        }
        Expr::Call(n, args) => {
            let sig = rels
                .get(n)
                .ok_or_else(|| format!("unknown relation `{n}`"))?;
            if sig.len() != args.len() {
                return Err(format!("relation `{n}` expects {} arguments", sig.len()));
            }
            for (a, t) in args.iter().zip(sig) {
                let valid = env.get(a) == Some(t) || ents.get(t).is_some_and(|m| m.contains(a));
                if !valid {
                    return Err(format!("argument `{a}` does not have type `{t}`"));
                }
            }
            Ok(())
        }
        Expr::Eq(a, b) | Expr::Ne(a, b) => {
            let ta = term_type(a, ents, env);
            let tb = term_type(b, ents, env);
            if ta.is_some() && ta == tb {
                Ok(())
            } else {
                Err(format!(
                    "cannot compare `{a}` and `{b}`: incompatible entity types"
                ))
            }
        }
        Expr::Not(x) => check_expr(x, bools, ents, rels, env),
        Expr::And(a, b) | Expr::Or(a, b) | Expr::Implies(a, b) => {
            check_expr(a, bools, ents, rels, env)?;
            check_expr(b, bools, ents, rels, env)
        }
        Expr::Forall {
            var,
            ty,
            guard,
            body,
        }
        | Expr::Exists {
            var,
            ty,
            guard,
            body,
        } => {
            if !ents.contains_key(ty) {
                return Err(format!("unknown finite entity type `{ty}`"));
            }
            env.insert(var.clone(), ty.clone());
            if let Some(g) = guard {
                check_expr(g, bools, ents, rels, env)?
            }
            let r = check_expr(body, bools, ents, rels, env);
            env.remove(var);
            r
        }
    }
}
fn term_type(
    n: &str,
    ents: &BTreeMap<String, BTreeSet<String>>,
    env: &BTreeMap<String, String>,
) -> Option<String> {
    env.get(n).cloned().or_else(|| {
        ents.iter()
            .find(|(_, m)| m.contains(n))
            .map(|(t, _)| t.clone())
    })
}

pub fn to_smt(spec: &EffectiveSpecification, selected: Option<&BTreeSet<String>>) -> String {
    let mut s = String::from(
        "(set-option :produce-unsat-cores true)\n(set-option :smt.core.minimize true)\n(set-option :print-success false)\n",
    );
    let (bools, ents, rels) = symbols(spec);
    for (name, members) in &ents {
        s.push_str(&format!(
            "(declare-datatypes (({} 0)) (({})))\n",
            sym(name),
            members.iter().map(|m| sym(m)).collect::<Vec<_>>().join(" ")
        ));
    }
    for b in bools {
        s.push_str(&format!("(declare-const {} Bool)\n", sym(&b)));
    }
    for (n, args) in rels {
        let sorts = args.iter().map(|x| sym(x)).collect::<Vec<_>>().join(" ");
        s.push_str(&format!("(declare-fun {} ({sorts}) Bool)\n", sym(&n)));
    }
    for c in &spec.clauses {
        let key = format!("{}:{}", c.adr_id, c.id);
        if selected.is_none_or(|x| x.contains(&key)) {
            s.push_str(&format!(
                "(assert (! {} :named {}))\n",
                emit(&c.expression, &ents, &BTreeMap::new()),
                sym(&key)
            ));
        }
    }
    s.push_str("(check-sat)\n(get-unsat-core)\n");
    s
}

pub fn obligation_to_smt(obligation: &RelationalProofObligation) -> String {
    use project::IntentDeclaration;
    let mut s = String::from(
        "(set-option :produce-unsat-cores true)\n(set-option :smt.core.minimize true)\n(set-option :print-success false)\n",
    );
    let mut entities: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut bools = BTreeSet::new();
    let mut relations = BTreeMap::new();
    for declaration in &obligation.model.declarations {
        match declaration {
            IntentDeclaration::Bool(name) => {
                bools.insert(name.clone());
            }
            IntentDeclaration::EntityType { name, members } => {
                entities
                    .entry(name.clone())
                    .or_default()
                    .extend(members.clone());
            }
            IntentDeclaration::Relation { name, arguments } => {
                relations.insert(name.clone(), arguments.clone());
            }
        }
    }
    for (name, members) in &entities {
        s.push_str(&format!(
            "(declare-datatypes (({} 0)) (({})))\n",
            sym(name),
            members.iter().map(|m| sym(m)).collect::<Vec<_>>().join(" ")
        ));
    }
    for name in bools {
        s.push_str(&format!("(declare-const {} Bool)\n", sym(&name)));
    }
    for (name, arguments) in &relations {
        s.push_str(&format!(
            "(declare-fun {} ({}) Bool)\n",
            sym(name),
            arguments
                .iter()
                .map(|argument| sym(argument))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    for constraint in obligation.model.constraints.values() {
        s.push_str(&format!(
            "(assert (! {} :named {}))\n",
            emit_intent(&constraint.formula, &entities, &BTreeMap::new()),
            sym(&constraint.id.0)
        ));
    }
    for fact in obligation.model.facts.values() {
        let relevant = obligation.model.edges.iter().any(|edge| {
            edge.from == GraphNode::Fact(fact.id.clone())
                && edge.kind == LinkKind::RelevantTo
                && matches!(edge.to, GraphNode::Constraint(_))
        });
        if !relevant {
            continue;
        }
        if !relations.contains_key(&fact.relation) || !fact_is_solver_supported(fact) {
            continue;
        }
        let atom = format!(
            "({} {})",
            sym(&fact.relation),
            fact.arguments
                .iter()
                .map(|argument| sym(argument))
                .collect::<Vec<_>>()
                .join(" ")
        );
        s.push_str(&format!(
            "(assert (! {} :named {}))\n",
            if fact.value {
                atom
            } else {
                format!("(not {atom})")
            },
            sym(&format!("FACT:{}", fact.id.0))
        ));
    }
    s.push_str("(check-sat)\n(get-unsat-core)\n");
    s
}

fn fact_is_solver_supported(fact: &project::ProjectFact) -> bool {
    fact.relation != "declares_direct_dependency"
        || (fact.attributes.get("kind").is_none_or(|v| v == "normal")
            && fact.attributes.get("optional").is_none_or(|v| v == "false")
            && fact.attributes.get("target").is_none_or(String::is_empty))
}

fn emit_intent(
    formula: &RelationalFormula,
    entities: &BTreeMap<String, BTreeSet<String>>,
    environment: &BTreeMap<String, String>,
) -> String {
    match formula {
        RelationalFormula::Bool(value) => value.to_string(),
        RelationalFormula::Name(name) => sym(name),
        RelationalFormula::Relation(name, arguments) => format!(
            "({} {})",
            sym(name),
            arguments
                .iter()
                .map(|argument| sym(environment.get(argument).unwrap_or(argument)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        RelationalFormula::Eq(a, b) => format!(
            "(= {} {})",
            sym(environment.get(a).unwrap_or(a)),
            sym(environment.get(b).unwrap_or(b))
        ),
        RelationalFormula::Ne(a, b) => format!(
            "(not (= {} {}))",
            sym(environment.get(a).unwrap_or(a)),
            sym(environment.get(b).unwrap_or(b))
        ),
        RelationalFormula::Not(value) => {
            format!("(not {})", emit_intent(value, entities, environment))
        }
        RelationalFormula::And(a, b) => format!(
            "(and {} {})",
            emit_intent(a, entities, environment),
            emit_intent(b, entities, environment)
        ),
        RelationalFormula::Or(a, b) => format!(
            "(or {} {})",
            emit_intent(a, entities, environment),
            emit_intent(b, entities, environment)
        ),
        RelationalFormula::Implies(a, b) => format!(
            "(=> {} {})",
            emit_intent(a, entities, environment),
            emit_intent(b, entities, environment)
        ),
        RelationalFormula::Forall {
            variable,
            entity_type,
            guard,
            body,
        } => expand_intent(
            true,
            variable,
            entity_type,
            guard.as_deref(),
            body,
            entities,
            environment,
        ),
        RelationalFormula::Exists {
            variable,
            entity_type,
            guard,
            body,
        } => expand_intent(
            false,
            variable,
            entity_type,
            guard.as_deref(),
            body,
            entities,
            environment,
        ),
    }
}

fn expand_intent(
    all: bool,
    variable: &str,
    entity_type: &str,
    guard: Option<&RelationalFormula>,
    body: &RelationalFormula,
    entities: &BTreeMap<String, BTreeSet<String>>,
    environment: &BTreeMap<String, String>,
) -> String {
    let operator = if all { "and" } else { "or" };
    let values = entities
        .get(entity_type)
        .into_iter()
        .flatten()
        .map(|value| {
            let mut environment = environment.clone();
            environment.insert(variable.into(), value.into());
            let body = emit_intent(body, entities, &environment);
            match guard {
                Some(guard) if all => {
                    format!("(=> {} {body})", emit_intent(guard, entities, &environment))
                }
                Some(guard) => format!(
                    "(and {} {body})",
                    emit_intent(guard, entities, &environment)
                ),
                None => body,
            }
        })
        .collect::<Vec<_>>();
    if values.is_empty() {
        if all { "true".into() } else { "false".into() }
    } else {
        format!("({operator} {})", values.join(" "))
    }
}
type Symbols = (
    BTreeSet<String>,
    BTreeMap<String, BTreeSet<String>>,
    BTreeMap<String, Vec<String>>,
);

fn symbols(spec: &EffectiveSpecification) -> Symbols {
    let mut b = BTreeSet::new();
    let mut e: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut r = BTreeMap::new();
    for d in &spec.declarations {
        match d {
            Decl::Bool(n) => {
                b.insert(n.clone());
            }
            Decl::EntityType { name, members } => {
                e.entry(name.clone()).or_default().extend(members.clone());
            }
            Decl::Relation { name, args } => {
                r.insert(name.clone(), args.clone());
            }
        }
    }
    (b, e, r)
}
fn sym(s: &str) -> String {
    format!("|{}|", s.replace('|', "_"))
}
fn emit(
    e: &Expr,
    ents: &BTreeMap<String, BTreeSet<String>>,
    env: &BTreeMap<String, String>,
) -> String {
    match e {
        Expr::Bool(v) => v.to_string(),
        Expr::Name(n) => sym(n),
        Expr::Call(n, a) => format!(
            "({} {})",
            sym(n),
            a.iter()
                .map(|x| sym(env.get(x).unwrap_or(x)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Expr::Eq(a, b) => format!(
            "(= {} {})",
            sym(env.get(a).unwrap_or(a)),
            sym(env.get(b).unwrap_or(b))
        ),
        Expr::Ne(a, b) => format!(
            "(not (= {} {}))",
            sym(env.get(a).unwrap_or(a)),
            sym(env.get(b).unwrap_or(b))
        ),
        Expr::Not(x) => format!("(not {})", emit(x, ents, env)),
        Expr::And(a, b) => format!("(and {} {})", emit(a, ents, env), emit(b, ents, env)),
        Expr::Or(a, b) => format!("(or {} {})", emit(a, ents, env), emit(b, ents, env)),
        Expr::Implies(a, b) => format!("(=> {} {})", emit(a, ents, env), emit(b, ents, env)),
        Expr::Forall {
            var,
            ty,
            guard,
            body,
        } => expand(true, var, ty, guard.as_deref(), body, ents, env),
        Expr::Exists {
            var,
            ty,
            guard,
            body,
        } => expand(false, var, ty, guard.as_deref(), body, ents, env),
    }
}
fn expand(
    all: bool,
    var: &str,
    ty: &str,
    guard: Option<&Expr>,
    body: &Expr,
    ents: &BTreeMap<String, BTreeSet<String>>,
    env: &BTreeMap<String, String>,
) -> String {
    let op = if all { "and" } else { "or" };
    let vals = ents
        .get(ty)
        .into_iter()
        .flatten()
        .map(|v| {
            let mut e = env.clone();
            e.insert(var.into(), v.into());
            let b = emit(body, ents, &e);
            match guard {
                Some(g) if all => format!("(=> {} {b})", emit(g, ents, &e)),
                Some(g) => format!("(and {} {b})", emit(g, ents, &e)),
                None => b,
            }
        })
        .collect::<Vec<_>>();
    if vals.is_empty() {
        if all { "true".into() } else { "false".into() }
    } else {
        format!("({op} {})", vals.join(" "))
    }
}

pub struct Z3Backend {
    pub executable: String,
    pub expected_version: String,
    pub timeout_ms: u64,
}
impl ConstraintBackend for Z3Backend {
    fn check(
        &self,
        obligation: &RelationalProofObligation,
        artifact: &Path,
    ) -> Result<BackendResult, Error> {
        let smt = obligation_to_smt(obligation);
        fs::write(artifact, &smt).map_err(|source| Error::Io {
            path: artifact.into(),
            source,
        })?;
        let actual = Command::new(&self.executable)
            .arg("--version")
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::SolverMissing(self.executable.clone())
                } else {
                    Error::SolverFailure(e.to_string())
                }
            })?;
        let version = String::from_utf8_lossy(&actual.stdout).trim().to_string();
        if !version.contains(&self.expected_version) {
            return Err(Error::SolverVersion {
                expected: self.expected_version.clone(),
                actual: version,
            });
        }
        let start = Instant::now();
        let mut child = Command::new(&self.executable)
            .args(["-in", &format!("-T:{}", self.timeout_ms.div_ceil(1000))])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Error::SolverFailure(e.to_string()))?;
        use std::io::Write;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(smt.as_bytes())
            .map_err(|e| Error::SolverFailure(e.to_string()))?;
        let out = child
            .wait_with_output()
            .map_err(|e| Error::SolverFailure(e.to_string()))?;
        let elapsed = start.elapsed();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let first = stdout.lines().next().unwrap_or("");
        if !out.status.success() && !matches!(first, "sat" | "unsat" | "unknown") {
            return Err(Error::SolverFailure(
                String::from_utf8_lossy(&out.stderr).into(),
            ));
        }
        let (verdict, core) = match first {
            "sat" => (Verdict::Sat, vec![]),
            "unsat" => (Verdict::Unsat, parse_core(&stdout)),
            "unknown" => {
                if elapsed.as_millis() >= self.timeout_ms as u128 {
                    (Verdict::Timeout, vec![])
                } else {
                    (Verdict::Unknown, vec![])
                }
            }
            _ => (Verdict::SolverFailure, vec![]),
        };
        Ok(BackendResult {
            verdict,
            core,
            solver_version: version,
            elapsed,
            timeout_ms: self.timeout_ms,
        })
    }
}
fn parse_core(s: &str) -> Vec<String> {
    let mut v = s
        .lines()
        .skip(1)
        .collect::<String>()
        .trim()
        .trim_matches(['(', ')'])
        .split_whitespace()
        .map(|x| x.trim_matches('|').to_string())
        .collect::<Vec<_>>();
    v.sort();
    v.dedup();
    v
}

#[derive(Serialize)]
struct Ledger {
    schema_version: u32,
    specification_sha256: String,
    adrproof_version: String,
    solver_version: String,
    solver_config: LedgerConfig,
    result: Verdict,
    evidence: evidence::Evidence,
    backends: [String; 1],
    elapsed_ms: u128,
    conflicts: Vec<Conflict>,
}
#[derive(Serialize)]
struct LedgerConfig {
    timeout_ms: u64,
}
pub fn run_check(
    root: &Path,
    backend: &dyn ConstraintBackend,
    artifacts: &Path,
) -> Result<CheckReport, Error> {
    run_check_with_roots(&roots::VerificationRoots::legacy(root, artifacts), backend)
}

pub fn run_check_with_roots(
    roots: &roots::VerificationRoots,
    backend: &dyn ConstraintBackend,
) -> Result<CheckReport, Error> {
    let artifacts = &roots.state_root;
    fs::create_dir_all(artifacts).map_err(|source| Error::Io {
        path: artifacts.into(),
        source,
    })?;
    let (model, mut input_files) = load_project_model_with_roots(roots)?;
    let model_path = artifacts.join("project-model.json");
    fs::write(&model_path, serde_json::to_vec_pretty(&model).unwrap()).map_err(|source| {
        Error::Io {
            path: model_path,
            source,
        }
    })?;
    let mut obligation = relational_obligation(model);
    let smt_path = artifacts.join("effective.smt2");
    let mut result = backend.check(&obligation, &smt_path)?;
    let unsupported = obligation.model.constraints.values().any(|constraint| {
        formula_requires_uncovered_absence(
            &obligation.model,
            &constraint.formula,
            false,
            &BTreeSet::new(),
        )
    });
    if unsupported && matches!(result.verdict, Verdict::Sat | Verdict::Unsat) {
        result.verdict = Verdict::Unverified;
    }
    let mut conflicts = result
        .core
        .iter()
        .filter_map(|key| conflict_for_key(key, &obligation.model))
        .collect::<Vec<_>>();
    conflicts.sort();
    let smt = fs::read(&smt_path).map_err(|source| Error::Io {
        path: smt_path.clone(),
        source,
    })?;
    input_files = relevant_semantic_inputs(&obligation.model, &input_files);
    let mut fingerprints =
        evidence::fingerprint_semantic_files(&input_files).map_err(|source| Error::Io {
            path: roots.project_root.clone(),
            source,
        })?;
    fingerprints.push(evidence::fingerprint_bytes(
        "generated:effective.smt2",
        &smt,
    ));
    fingerprints.sort_by(|a, b| a.source.cmp(&b.source));
    let configuration_sha256 =
        evidence::configuration_hash(result.timeout_ms, &["smt.core.minimize=true"]);
    let evidence_status = match result.verdict {
        Verdict::Sat => evidence::VerificationStatus::Pass,
        Verdict::Unsat => evidence::VerificationStatus::Fail,
        Verdict::Unknown | Verdict::Timeout => evidence::VerificationStatus::Unknown,
        Verdict::SolverFailure | Verdict::InvalidInput => evidence::VerificationStatus::Error,
        Verdict::Unverified => evidence::VerificationStatus::Unverified,
    };
    let proof_evidence = evidence::Evidence {
        id: project::EvidenceId("pending".into()),
        obligation: obligation.id.clone(),
        backend: "z3".into(),
        backend_version: result.solver_version.clone(),
        configuration_sha256,
        inputs: fingerprints,
        result_at_execution: evidence_status.clone(),
        recorded_at_unix_nanos: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
        diagnostics: conflicts
            .iter()
            .map(|conflict| format!("{}:{}", conflict.adr_id, conflict.clause_id))
            .collect(),
    };
    let proof_evidence =
        evidence::store(&artifacts.join("evidence"), proof_evidence).map_err(|source| {
            Error::Io {
                path: artifacts.join("evidence"),
                source,
            }
        })?;
    obligation.model.edges.push(GraphEdge {
        from: GraphNode::ProofObligation(obligation.id.clone()),
        kind: LinkKind::EvidenceFor,
        to: GraphNode::Evidence(proof_evidence.id.clone()),
    });
    obligation.model.normalize();
    fs::write(
        artifacts.join("project-model.json"),
        serde_json::to_vec_pretty(&obligation.model).unwrap(),
    )
    .map_err(|source| Error::Io {
        path: artifacts.join("project-model.json"),
        source,
    })?;
    let ledger_path = artifacts.join("proof-ledger.json");
    let ledger = Ledger {
        schema_version: 3,
        specification_sha256: format!("{:x}", Sha256::digest(&smt)),
        adrproof_version: env!("CARGO_PKG_VERSION").into(),
        solver_version: result.solver_version.clone(),
        solver_config: LedgerConfig {
            timeout_ms: result.timeout_ms,
        },
        result: result.verdict.clone(),
        evidence: proof_evidence,
        backends: ["z3".into()],
        elapsed_ms: result.elapsed.as_millis(),
        conflicts: conflicts.clone(),
    };
    fs::write(&ledger_path, serde_json::to_vec_pretty(&ledger).unwrap()).map_err(|source| {
        Error::Io {
            path: ledger_path.clone(),
            source,
        }
    })?;
    Ok(CheckReport {
        roots: roots.view(),
        verdict: result.verdict,
        evidence_status,
        conflicts,
        solver: result.solver_version,
        elapsed_ms: result.elapsed.as_millis(),
        smt_artifact: smt_path,
        ledger_artifact: ledger_path,
    })
}

pub fn load_project_model(root: &Path) -> Result<(ProjectModel, Vec<PathBuf>), Error> {
    let roots = roots::VerificationRoots::legacy(root, &root.join(".adrproof"));
    let (model, inputs) = load_project_model_with_roots(&roots)?;
    Ok((model, inputs.into_iter().map(|input| input.path).collect()))
}

pub fn load_project_model_with_roots(
    roots: &roots::VerificationRoots,
) -> Result<(ProjectModel, Vec<roots::SemanticInput>), Error> {
    let adrs = load_adrs(&roots.specification_root)?;
    let spec = effective(&adrs, false)?;
    let mut model = lower_to_project_model(&adrs, &spec);
    let mut input_files = spec
        .clauses
        .iter()
        .map(|clause| roots::SemanticInput {
            identity: roots.spec_identity(&clause.span.filename),
            path: clause.span.filename.clone(),
        })
        .collect::<Vec<_>>();
    namespace_model_artifacts(&mut model, "spec", &roots.specification_root);
    if let Some(provider) = cargo_facts::CargoMetadataProvider::discover(&roots.project_root) {
        let cargo = provider.extract()?;
        input_files.extend(cargo.input_files.iter().map(|path| roots::SemanticInput {
            identity: roots.project_identity(path),
            path: path.clone(),
        }));
        model.fact_coverage.extend(cargo.coverage);
        let mut cargo_model = ProjectModel::default();
        for artifact in cargo.artifacts {
            cargo_model.artifacts.insert(artifact.id.clone(), artifact);
        }
        cargo_model.add_facts(cargo.facts);
        namespace_model_artifacts(&mut cargo_model, "project", &roots.project_root);
        for artifact in cargo_model.artifacts.into_values() {
            model.artifacts.insert(artifact.id.clone(), artifact);
        }
        model.facts.extend(cargo_model.facts);
        model.edges.extend(cargo_model.edges);
        enrich_package_domain(&mut model);
        apply_closed_world_facts(&mut model);
    }
    if let Some(provider) =
        sql_migrations::PostgresMigrationFactProvider::discover(&roots.project_root)
    {
        let mut sql = provider.extract()?;
        input_files.extend(sql.input_files.iter().map(|path| roots::SemanticInput {
            identity: roots.project_identity(path),
            path: path.clone(),
        }));
        for coverage in &mut sql.coverage {
            for diagnostic in &mut coverage.diagnostics {
                let logical = PathBuf::from(
                    roots.project_identity(&roots.project_root.join(&diagnostic.provenance.source)),
                );
                diagnostic.provenance.source = logical.clone();
                if let Some(span) = &mut diagnostic.provenance.span {
                    span.filename = logical;
                }
            }
        }
        model.fact_coverage.extend(sql.coverage);
        let mut sql_model = ProjectModel::default();
        for artifact in sql.artifacts {
            sql_model.artifacts.insert(artifact.id.clone(), artifact);
        }
        sql_model.add_facts(sql.facts);
        namespace_model_artifacts(&mut sql_model, "project", &roots.project_root);
        for artifact in sql_model.artifacts.into_values() {
            model.artifacts.insert(artifact.id.clone(), artifact);
        }
        model.facts.extend(sql_model.facts);
        model.edges.extend(sql_model.edges);
    }
    apply_generic_closed_world_facts(&mut model);
    model.normalize();
    input_files.sort_by(|a, b| (&a.identity, &a.path).cmp(&(&b.identity, &b.path)));
    input_files.dedup_by(|a, b| a.identity == b.identity);
    Ok((model, input_files))
}

fn namespace_model_artifacts(model: &mut ProjectModel, namespace: &str, root: &Path) {
    let logical = |path: &Path| {
        let relative = path.strip_prefix(root).unwrap_or(path);
        PathBuf::from(format!(
            "{namespace}:{}",
            relative.display().to_string().replace('\\', "/")
        ))
    };
    for decision in model.decisions.values_mut() {
        decision.provenance.source = logical(&decision.provenance.source);
    }
    for constraint in model.constraints.values_mut() {
        constraint.provenance.source = logical(&constraint.provenance.source);
        if let Some(span) = &mut constraint.provenance.span {
            span.filename = constraint.provenance.source.clone();
        }
    }
    for fact in model.facts.values_mut() {
        fact.provenance.source = logical(&fact.provenance.source);
        if let Some(span) = &mut fact.provenance.span {
            span.filename = fact.provenance.source.clone();
        }
    }
    let old_artifacts = std::mem::take(&mut model.artifacts);
    let mut replacements = BTreeMap::new();
    for (old_id, mut artifact) in old_artifacts {
        artifact.provenance.source = logical(&artifact.provenance.source);
        let new_id = if artifact.kind == "cargo_package" {
            old_id.clone()
        } else {
            ArtifactId(artifact.provenance.source.display().to_string())
        };
        replacements.insert(old_id, new_id.clone());
        artifact.id = new_id.clone();
        model.artifacts.insert(new_id, artifact);
    }
    for edge in &mut model.edges {
        if let GraphNode::Artifact(id) = &mut edge.from
            && let Some(new_id) = replacements.get(id)
        {
            *id = new_id.clone();
        }
        if let GraphNode::Artifact(id) = &mut edge.to
            && let Some(new_id) = replacements.get(id)
        {
            *id = new_id.clone();
        }
    }
}

pub fn current_evidence_status(
    root: &Path,
    evidence_directory: &Path,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<evidence::VerificationStatus, Error> {
    current_evidence_status_with_roots(
        &roots::VerificationRoots::legacy(root, evidence_directory.parent().unwrap_or(root)),
        evidence_directory,
        backend_version,
        timeout_ms,
    )
}

pub fn current_evidence_status_with_roots(
    roots: &roots::VerificationRoots,
    evidence_directory: &Path,
    backend_version: &str,
    timeout_ms: u64,
) -> Result<evidence::VerificationStatus, Error> {
    let (model, inputs) = load_project_model_with_roots(roots)?;
    let obligation = relational_obligation(model);
    let inputs = relevant_semantic_inputs(&obligation.model, &inputs);
    let mut fingerprints =
        evidence::fingerprint_semantic_files(&inputs).map_err(|source| Error::Io {
            path: roots.project_root.clone(),
            source,
        })?;
    fingerprints.push(evidence::fingerprint_bytes(
        "generated:effective.smt2",
        obligation_to_smt(&obligation).as_bytes(),
    ));
    fingerprints.sort_by(|a, b| a.source.cmp(&b.source));
    let latest = evidence::latest(evidence_directory).map_err(|source| Error::Io {
        path: evidence_directory.to_path_buf(),
        source,
    })?;
    Ok(
        latest.map_or(evidence::VerificationStatus::Unverified, |item| {
            let config = evidence::configuration_hash(timeout_ms, &["smt.core.minimize=true"]);
            match evidence::assess(&item, &fingerprints, backend_version, &config) {
                evidence::EvidenceValidity::Current => item.result_at_execution,
                evidence::EvidenceValidity::Stale => evidence::VerificationStatus::Stale,
            }
        }),
    )
}

pub(crate) fn relevant_semantic_inputs(
    model: &ProjectModel,
    fallback: &[roots::SemanticInput],
) -> Vec<roots::SemanticInput> {
    let used_relations = model
        .constraints
        .values()
        .flat_map(|constraint| relations_in_formula(&constraint.formula))
        .collect::<BTreeSet<_>>();
    let uses_sql = model.fact_coverage.iter().any(|coverage| {
        coverage.provider == "postgres_migrations" && used_relations.contains(&coverage.relation)
    });
    let identities = model
        .constraints
        .values()
        .map(|constraint| constraint.provenance.source.display().to_string())
        .chain(model.edges.iter().filter_map(|edge| {
            match (&edge.from, &edge.kind) {
                (GraphNode::Fact(id), LinkKind::RelevantTo) => model
                    .facts
                    .get(id)
                    .map(|fact| fact.provenance.source.display().to_string()),
                _ => None,
            }
        }))
        .collect::<BTreeSet<_>>();
    let mut selected = fallback
        .iter()
        .filter(|input| {
            identities.contains(&input.identity)
                || (uses_sql
                    && input.identity.starts_with("project:migrations/")
                    && input.identity.ends_with(".sql"))
        })
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        selected.extend_from_slice(fallback);
    }
    selected.sort_by(|a, b| a.identity.cmp(&b.identity));
    selected.dedup_by(|a, b| a.identity == b.identity);
    selected
}

fn relations_in_formula(formula: &RelationalFormula) -> BTreeSet<String> {
    let mut relations = BTreeSet::new();
    fn visit(formula: &RelationalFormula, output: &mut BTreeSet<String>) {
        match formula {
            RelationalFormula::Relation(name, _) => {
                output.insert(name.clone());
            }
            RelationalFormula::Not(value) => visit(value, output),
            RelationalFormula::And(a, b)
            | RelationalFormula::Or(a, b)
            | RelationalFormula::Implies(a, b) => {
                visit(a, output);
                visit(b, output);
            }
            RelationalFormula::Forall { guard, body, .. }
            | RelationalFormula::Exists { guard, body, .. } => {
                if let Some(guard) = guard {
                    visit(guard, output);
                }
                visit(body, output);
            }
            _ => {}
        }
    }
    visit(formula, &mut relations);
    relations
}

fn coverage_scope_for_atom(
    relation: &str,
    arguments: &[String],
    bound: &BTreeSet<String>,
) -> project::CoverageScope {
    let Some(subject) = arguments.first() else {
        return project::CoverageScope::Global;
    };
    if bound.contains(subject) {
        return project::CoverageScope::Global;
    }
    match relation {
        "declares_direct_dependency" | "package" | "workspace_member" => {
            project::CoverageScope::Package(subject.clone())
        }
        "schema" => project::CoverageScope::Schema(subject.clone()),
        "materialized_view" => project::CoverageScope::MaterializedView(subject.clone()),
        _ => project::CoverageScope::Table(subject.clone()),
    }
}

fn formula_requires_uncovered_absence(
    model: &ProjectModel,
    formula: &RelationalFormula,
    negated: bool,
    bound: &BTreeSet<String>,
) -> bool {
    match formula {
        RelationalFormula::Relation(relation, arguments) => {
            let observed = model.facts.values().any(|fact| {
                fact.relation == *relation && fact.arguments == *arguments && fact.value
            });
            if observed {
                return false;
            }
            let scope = coverage_scope_for_atom(relation, arguments, bound);
            // Whether the absent atom is required positively or negatively, its truth
            // is machine-decidable only inside a closed completeness claim.
            let _ = negated;
            model.coverage_for(relation, &scope) != Some(project::WorldAssumption::Closed)
        }
        RelationalFormula::Not(value) => {
            formula_requires_uncovered_absence(model, value, !negated, bound)
        }
        RelationalFormula::And(left, right) | RelationalFormula::Or(left, right) => {
            formula_requires_uncovered_absence(model, left, negated, bound)
                || formula_requires_uncovered_absence(model, right, negated, bound)
        }
        RelationalFormula::Implies(left, right) => {
            formula_requires_uncovered_absence(model, left, !negated, bound)
                || formula_requires_uncovered_absence(model, right, negated, bound)
        }
        RelationalFormula::Forall {
            variable,
            guard,
            body,
            ..
        }
        | RelationalFormula::Exists {
            variable,
            guard,
            body,
            ..
        } => {
            let mut nested = bound.clone();
            nested.insert(variable.clone());
            guard.as_ref().is_some_and(|guard| {
                formula_requires_uncovered_absence(model, guard, negated, &nested)
            }) || formula_requires_uncovered_absence(model, body, negated, &nested)
        }
        _ => false,
    }
}

fn apply_generic_closed_world_facts(model: &mut ProjectModel) {
    let entities = model
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            project::IntentDeclaration::EntityType { name, members } => {
                Some((name.clone(), members.clone()))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let relations = model
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            project::IntentDeclaration::Relation { name, arguments }
                if name != "declares_direct_dependency" =>
            {
                Some((name.clone(), arguments.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let migration_artifacts = model
        .artifacts
        .values()
        .filter(|artifact| artifact.kind == "postgres_migration")
        .map(|artifact| artifact.id.clone())
        .collect::<Vec<_>>();
    for (relation, argument_types) in relations {
        let Some(domains) = argument_types
            .iter()
            .map(|name| entities.get(name).cloned())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let mut tuples = vec![Vec::<String>::new()];
        for domain in domains {
            tuples = tuples
                .into_iter()
                .flat_map(|prefix| {
                    domain.iter().map(move |value| {
                        let mut tuple = prefix.clone();
                        tuple.push(value.clone());
                        tuple
                    })
                })
                .collect();
        }
        let present = model
            .facts
            .values()
            .filter(|fact| fact.relation == relation && fact.value)
            .map(|fact| fact.arguments.clone())
            .collect::<BTreeSet<_>>();
        for tuple in tuples {
            if present.contains(&tuple) {
                continue;
            }
            let scope = coverage_scope_for_atom(&relation, &tuple, &BTreeSet::new());
            if model.coverage_for(&relation, &scope) != Some(project::WorldAssumption::Closed) {
                continue;
            }
            let id = project::FactId(format!("closed-world:{relation}:{}", tuple.join(":")));
            let nearest_provenance = model
                .facts
                .values()
                .find(|fact| {
                    fact.relation == "column"
                        && tuple.len() >= 2
                        && fact.arguments.first() == tuple.first()
                        && fact.arguments.get(1) == tuple.get(1)
                })
                .or_else(|| {
                    model.facts.values().find(|fact| {
                        fact.relation == "table" && fact.arguments.first() == tuple.first()
                    })
                })
                .map(|fact| fact.provenance.clone())
                .unwrap_or_else(|| Provenance {
                    kind: ProvenanceKind::DeterministicallyExtracted,
                    source: PathBuf::from("project:migrations"),
                    span: None,
                    extractor: Some("closed-world PostgreSQL migration closure".into()),
                });
            model.facts.insert(
                id.clone(),
                project::ProjectFact {
                    id: id.clone(),
                    relation: relation.clone(),
                    arguments: tuple,
                    value: false,
                    attributes: BTreeMap::from([("coverage".into(), "closed".into())]),
                    provenance: nearest_provenance,
                },
            );
            for artifact in &migration_artifacts {
                model.edges.push(GraphEdge {
                    from: GraphNode::Artifact(artifact.clone()),
                    kind: LinkKind::Produces,
                    to: GraphNode::Fact(id.clone()),
                });
            }
        }
    }
    model.normalize();
}

fn enrich_package_domain(model: &mut ProjectModel) {
    let packages = model
        .facts
        .values()
        .flat_map(|fact| fact.arguments.iter().cloned())
        .collect::<BTreeSet<_>>();
    for declaration in &mut model.declarations {
        if let project::IntentDeclaration::EntityType { name, members } = declaration
            && name == "Package"
        {
            members.extend(packages.clone());
        }
    }
}

fn apply_closed_world_facts(model: &mut ProjectModel) {
    use project::WorldAssumption;
    let covered = model.fact_coverage.iter().any(|coverage| {
        coverage.relation == "declares_direct_dependency"
            && coverage.world == WorldAssumption::Closed
    });
    if !covered {
        return;
    }
    let members = model
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            project::IntentDeclaration::EntityType { name, members } if name == "Package" => {
                Some(members.clone())
            }
            _ => None,
        })
        .unwrap_or_default();
    let sources = model
        .facts
        .values()
        .filter(|fact| fact.relation == "workspace_member")
        .filter_map(|fact| fact.arguments.first().cloned())
        .collect::<BTreeSet<_>>();
    let present = model
        .facts
        .values()
        .filter(|fact| {
            fact.relation == "declares_direct_dependency" && fact_is_solver_supported(fact)
        })
        .map(|fact| (fact.arguments[0].clone(), fact.arguments[1].clone()))
        .collect::<BTreeSet<_>>();
    let workspace_manifests = model
        .facts
        .values()
        .filter(|fact| fact.relation == "workspace_member")
        .filter_map(|fact| {
            fact.arguments
                .first()
                .map(|package| (package.clone(), fact.provenance.source.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut absent = Vec::new();
    for source in sources {
        for target in &members {
            if present.contains(&(source.clone(), target.clone())) {
                continue;
            }
            absent.push(project::ProjectFact {
                id: project::FactId(format!("cargo:absence:{source}:{target}")),
                relation: "declares_direct_dependency".into(),
                arguments: vec![source.clone(), target.clone()],
                value: false,
                attributes: BTreeMap::from([
                    ("kind".into(), "normal".into()),
                    ("optional".into(), "false".into()),
                    ("target".into(), String::new()),
                    ("coverage".into(), "closed".into()),
                ]),
                provenance: Provenance {
                    kind: ProvenanceKind::DeterministicallyExtracted,
                    source: workspace_manifests
                        .get(&source)
                        .cloned()
                        .unwrap_or_else(|| PathBuf::from("project:Cargo.toml")),
                    span: None,
                    extractor: Some("cargo manifest declaration closure".into()),
                },
            });
        }
    }
    model.add_facts(absent);
}

fn conflict_for_key(key: &str, model: &ProjectModel) -> Option<Conflict> {
    if let Some(constraint) = model.constraints.get(&ConstraintId(key.into())) {
        let span = constraint.provenance.span.clone()?;
        let (adr_id, clause_id) = key.split_once(':').unwrap_or((key, ""));
        return Some(Conflict {
            adr_id: adr_id.into(),
            clause_id: clause_id.into(),
            description: constraint.description.clone(),
            span,
            origin_kind: constraint.provenance.kind.clone(),
        });
    }
    let fact_key = key.strip_prefix("FACT:")?;
    let fact = model.facts.get(&project::FactId(fact_key.into()))?;
    Some(Conflict {
        adr_id: "CARGO".into(),
        clause_id: fact.id.0.clone(),
        description: format!(
            "{}{}({})",
            if fact.value { "" } else { "not " },
            fact.relation,
            fact.arguments.join(", ")
        ),
        span: fact.provenance.span.clone().unwrap_or(SourceSpan {
            filename: fact.provenance.source.clone(),
            line: 1,
            column: 1,
        }),
        origin_kind: fact.provenance.kind.clone(),
    })
}

#[cfg(test)]
mod tests;
