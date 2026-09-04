use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRoots {
    pub project_root: PathBuf,
    pub specification_root: PathBuf,
    pub state_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootsView {
    pub project_root: PathBuf,
    pub specification_root: PathBuf,
    pub state_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticInput {
    pub identity: String,
    pub path: PathBuf,
}

impl VerificationRoots {
    pub fn legacy(root: &Path, state_root: &Path) -> Self {
        Self {
            project_root: normalize(root),
            specification_root: normalize(root),
            state_root: normalize(state_root),
        }
    }

    pub fn explicit(project_root: &Path, specification_root: &Path, state_root: &Path) -> Self {
        Self {
            project_root: normalize(project_root),
            specification_root: normalize(specification_root),
            state_root: normalize(state_root),
        }
    }

    pub fn view(&self) -> RootsView {
        RootsView {
            project_root: self.project_root.clone(),
            specification_root: self.specification_root.clone(),
            state_root: self.state_root.clone(),
        }
    }

    pub fn project_identity(&self, path: &Path) -> String {
        identity("project", &self.project_root, path)
    }

    pub fn spec_identity(&self, path: &Path) -> String {
        identity("spec", &self.specification_root, path)
    }

    pub fn resolve_identity(&self, identity: &str) -> Option<PathBuf> {
        identity
            .strip_prefix("project:")
            .map(|path| self.project_root.join(path))
            .or_else(|| {
                identity
                    .strip_prefix("spec:")
                    .map(|path| self.specification_root.join(path))
            })
    }
}

fn identity(namespace: &str, root: &Path, path: &Path) -> String {
    // Cargo may return canonical paths even when the caller used an alias
    // (notably /var -> /private/var on macOS). Keep lexical identities when
    // possible, then compare both physical paths before falling back.
    let relative = path
        .strip_prefix(root)
        .map(Path::to_path_buf)
        .ok()
        .or_else(|| {
            if !path.is_absolute() {
                return None;
            }
            let canonical_root = std::fs::canonicalize(root).ok()?;
            let canonical_path = std::fs::canonicalize(path).ok()?;
            canonical_path
                .strip_prefix(canonical_root)
                .ok()
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| path.to_path_buf());
    let value = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".into()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("{namespace}:{value}")
}

fn normalize(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    normalized
}
