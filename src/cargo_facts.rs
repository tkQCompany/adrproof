use crate::Error;
use crate::project::{
    Artifact, ArtifactId, FactCoverage, FactId, ProjectFact, Provenance, ProvenanceKind,
    WorldAssumption,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
}
#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<Dependency>,
}
#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
    kind: Option<String>,
    optional: bool,
    target: Option<String>,
    path: Option<PathBuf>,
    rename: Option<String>,
    source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CargoFacts {
    pub artifacts: Vec<Artifact>,
    pub facts: Vec<ProjectFact>,
    pub input_files: Vec<PathBuf>,
    pub coverage: Vec<FactCoverage>,
}
impl CargoFacts {
    pub fn fact_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for fact in &self.facts {
            *counts.entry(fact.relation.clone()).or_default() += 1;
        }
        counts
    }
}

pub struct CargoMetadataProvider {
    pub workspace_root: PathBuf,
}
impl CargoMetadataProvider {
    pub fn discover(root: &Path) -> Option<Self> {
        root.join("Cargo.toml").is_file().then(|| Self {
            workspace_root: root.to_path_buf(),
        })
    }
    pub fn extract(&self) -> Result<CargoFacts, Error> {
        let output = Command::new("cargo")
            .args([
                "metadata",
                "--format-version",
                "1",
                "--no-deps",
                "--offline",
            ])
            .current_dir(&self.workspace_root)
            .output()
            .map_err(|error| {
                Error::ProviderFailure(format!("cannot execute cargo metadata: {error}"))
            })?;
        if !output.status.success() {
            return Err(Error::ProviderFailure(format!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        let mut metadata: Metadata = serde_json::from_slice(&output.stdout).map_err(|error| {
            Error::ProviderFailure(format!("invalid cargo metadata JSON: {error}"))
        })?;
        metadata
            .packages
            .sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
        let members: BTreeSet<_> = metadata.workspace_members.into_iter().collect();
        let mut artifacts = Vec::new();
        let mut facts = Vec::new();
        let mut inputs = BTreeSet::new();
        inputs.insert(metadata.workspace_root.join("Cargo.toml"));
        for package in metadata.packages {
            let relative_manifest = package
                .manifest_path
                .strip_prefix(&metadata.workspace_root)
                .unwrap_or(&package.manifest_path)
                .to_path_buf();
            let provenance = Provenance {
                kind: ProvenanceKind::DeterministicallyExtracted,
                source: relative_manifest.clone(),
                span: None,
                extractor: Some("cargo metadata --format-version 1 --no-deps --offline".into()),
            };
            inputs.insert(package.manifest_path.clone());
            artifacts.push(Artifact {
                id: ArtifactId(format!("package:{}", package.name)),
                kind: "cargo_package".into(),
                provenance: provenance.clone(),
            });
            artifacts.push(Artifact {
                id: ArtifactId(relative_manifest.display().to_string()),
                kind: "cargo_manifest".into(),
                provenance: provenance.clone(),
            });
            facts.push(fact(
                format!("cargo:package:{}", package.name),
                "package",
                vec![package.name.clone()],
                BTreeMap::new(),
                provenance.clone(),
            ));
            if members.contains(&package.id) {
                facts.push(fact(
                    format!("cargo:workspace-member:{}", package.name),
                    "workspace_member",
                    vec![package.name.clone()],
                    BTreeMap::new(),
                    provenance.clone(),
                ));
            }
            let mut dependencies = package.dependencies;
            dependencies.sort_by(|a, b| {
                (&a.name, &a.kind, &a.target, &a.rename)
                    .cmp(&(&b.name, &b.kind, &b.target, &b.rename))
            });
            for dependency in dependencies {
                let kind = dependency.kind.clone().unwrap_or_else(|| "normal".into());
                let alias = dependency
                    .rename
                    .clone()
                    .unwrap_or_else(|| dependency.name.clone());
                let source_kind = if dependency.path.is_some() {
                    "path"
                } else if dependency
                    .source
                    .as_deref()
                    .is_some_and(|source| source.starts_with("git+"))
                {
                    "git"
                } else {
                    "registry"
                };
                let attributes = BTreeMap::from([
                    ("kind".into(), kind.clone()),
                    ("optional".into(), dependency.optional.to_string()),
                    (
                        "target".into(),
                        dependency.target.clone().unwrap_or_default(),
                    ),
                    ("declared_name".into(), alias.clone()),
                    ("actual_package".into(), dependency.name.clone()),
                    ("source_kind".into(), source_kind.into()),
                ]);
                let mut dependency_provenance = provenance.clone();
                dependency_provenance.span = dependency_line(&package.manifest_path, &alias);
                if let Some(span) = &mut dependency_provenance.span {
                    span.filename = relative_manifest.clone();
                }
                facts.push(fact(
                    format!(
                        "cargo:declared-direct:{}:{}:{}:{}",
                        package.name,
                        dependency.name,
                        kind,
                        dependency.target.clone().unwrap_or_default()
                    ),
                    "declares_direct_dependency",
                    vec![package.name.clone(), dependency.name],
                    attributes,
                    dependency_provenance,
                ));
            }
        }
        artifacts.sort_by(|a, b| a.id.cmp(&b.id));
        facts.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(CargoFacts{artifacts,facts,input_files:inputs.into_iter().collect(),coverage:vec![FactCoverage{relation:"declares_direct_dependency".into(),provider:"cargo_metadata".into(),world:WorldAssumption::Closed,scope:crate::project::CoverageScope::Global,qualifiers:BTreeMap::from([("kind".into(),"normal".into()),("optional".into(),"false".into()),("target".into(),"unconditional".into()),("identity".into(),"actual_package_name".into()),("sources".into(),"path,registry,git".into())]),statement:"all unconditional, non-optional normal direct dependency declarations in workspace manifests are enumerated".into(),diagnostics:Vec::new()}]})
    }
}
fn dependency_line(path: &Path, name: &str) -> Option<crate::SourceSpan> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().position(|line| {
        let line = line.trim_start();
        line.starts_with(&format!("{name} ")) || line.starts_with(&format!("{name}="))
    })? + 1;
    Some(crate::SourceSpan {
        filename: path.to_path_buf(),
        line,
        column: 1,
    })
}
fn fact(
    id: String,
    relation: &str,
    arguments: Vec<String>,
    attributes: BTreeMap<String, String>,
    provenance: Provenance,
) -> ProjectFact {
    ProjectFact {
        id: FactId(id),
        relation: relation.into(),
        arguments,
        value: true,
        attributes,
        provenance,
    }
}
impl crate::CodeFactProvider for CargoMetadataProvider {
    fn facts(&self) -> Result<Vec<ProjectFact>, Error> {
        Ok(self.extract()?.facts)
    }
}
