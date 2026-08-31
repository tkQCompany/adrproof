use crate::Error;
use crate::roots::{RootsView, VerificationRoots};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BUNDLE_SCHEMA: &str = "adrproof-evidence-bundle-v1";
pub const SIGNATURE_SCHEMA: &str = "adrproof-bundle-signature-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleManifest {
    pub schema_version: String,
    pub adrproof_version: String,
    pub created_at_unix_nanos: u128,
    pub roots: RootsView,
    pub files: Vec<BundleFile>,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleVerification {
    pub schema_version: String,
    pub valid: bool,
    pub verified_files: usize,
    pub diagnostics: Vec<String>,
    pub signature: BundleSignatureVerification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleSignature {
    pub schema_version: String,
    pub algorithm: String,
    pub manifest_sha256: String,
    pub public_key: String,
    pub key_id: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleSignatureVerification {
    pub present: bool,
    pub cryptographically_valid: bool,
    pub trusted_key_match: Option<bool>,
    pub key_id: Option<String>,
}

pub fn create(roots: &VerificationRoots, output: &Path) -> Result<BundleManifest, Error> {
    create_with_signing_key(roots, output, None)
}

pub fn create_signed(
    roots: &VerificationRoots,
    output: &Path,
    secret_key: &[u8; 32],
) -> Result<BundleManifest, Error> {
    create_with_signing_key(roots, output, Some(secret_key))
}

fn create_with_signing_key(
    roots: &VerificationRoots,
    output: &Path,
    secret_key: Option<&[u8; 32]>,
) -> Result<BundleManifest, Error> {
    if output.exists() {
        return Err(Error::ProviderFailure(format!(
            "bundle output already exists: {}",
            output.display()
        )));
    }
    fs::create_dir_all(output.join("data")).map_err(|source| Error::Io {
        path: output.to_path_buf(),
        source,
    })?;
    let mut source_files = Vec::new();
    collect_files(&roots.state_root, &roots.state_root, &mut source_files)?;
    let mut files = Vec::new();
    for (relative, source) in source_files {
        let target = output.join("data").join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| Error::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::copy(&source, &target).map_err(|source| Error::Io {
            path: target.clone(),
            source,
        })?;
        let bytes = fs::read(&target).map_err(|source| Error::Io {
            path: target.clone(),
            source,
        })?;
        files.push(BundleFile {
            path: path_identity(&relative),
            sha256: hash(&bytes),
            bytes: bytes.len() as u64,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let manifest = BundleManifest {
        schema_version: BUNDLE_SCHEMA.into(),
        adrproof_version: env!("CARGO_PKG_VERSION").into(),
        created_at_unix_nanos: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos()),
        roots: roots.view(),
        files,
        authority: "Offline integrity of the copied ADRProof evidence artifacts; verification does not rerun or independently validate the underlying tools.".into(),
    };
    let target = output.join("bundle.json");
    let manifest_bytes =
        serde_json::to_vec_pretty(&manifest).expect("bundle manifest serialization");
    fs::write(&target, &manifest_bytes).map_err(|source| Error::Io {
        path: target,
        source,
    })?;
    if let Some(secret_key) = secret_key {
        let signing_key = SigningKey::from_bytes(secret_key);
        let public_key = signing_key.verifying_key().to_bytes();
        let signature = signing_key.sign(&manifest_bytes);
        let envelope = BundleSignature {
            schema_version: SIGNATURE_SCHEMA.into(),
            algorithm: "Ed25519".into(),
            manifest_sha256: hash(&manifest_bytes),
            public_key: STANDARD_NO_PAD.encode(public_key),
            key_id: hash(&public_key),
            signature: STANDARD_NO_PAD.encode(signature.to_bytes()),
        };
        let signature_path = output.join("bundle.sig.json");
        fs::write(
            &signature_path,
            serde_json::to_vec_pretty(&envelope).expect("bundle signature serialization"),
        )
        .map_err(|source| Error::Io {
            path: signature_path,
            source,
        })?;
    }
    Ok(manifest)
}

pub fn verify(bundle: &Path) -> Result<BundleVerification, Error> {
    verify_with_key(bundle, None, false)
}

pub fn verify_with_key(
    bundle: &Path,
    trusted_public_key: Option<&[u8; 32]>,
    require_signature: bool,
) -> Result<BundleVerification, Error> {
    let manifest_path = bundle.join("bundle.json");
    let manifest_bytes = fs::read(&manifest_path).map_err(|source| Error::Io {
        path: manifest_path.clone(),
        source,
    })?;
    let manifest: BundleManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| Error::ProviderFailure(format!("{}: {error}", manifest_path.display())))?;
    let mut diagnostics = Vec::new();
    if manifest.schema_version != BUNDLE_SCHEMA {
        diagnostics.push(format!(
            "expected schema {BUNDLE_SCHEMA}, observed {}",
            manifest.schema_version
        ));
    }
    let mut expected = manifest
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    expected.sort();
    expected.dedup();
    if expected.len() != manifest.files.len() {
        diagnostics.push("manifest contains duplicate paths".into());
    }
    for file in &manifest.files {
        let relative = safe_relative(&file.path)?;
        let path = bundle.join("data").join(relative);
        match fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() as u64 != file.bytes {
                    diagnostics.push(format!("{}: byte length mismatch", file.path));
                }
                if hash(&bytes) != file.sha256 {
                    diagnostics.push(format!("{}: SHA-256 mismatch", file.path));
                }
            }
            Err(error) => diagnostics.push(format!("{}: {error}", file.path)),
        }
    }
    let mut actual = Vec::new();
    let data = bundle.join("data");
    if data.is_dir() {
        collect_bundle_identities(&data, &data, &mut actual)?;
    }
    actual.sort();
    if actual != expected {
        diagnostics.push("bundle data contains missing or unlisted files".into());
    }
    let signature = verify_signature(
        bundle,
        &manifest_bytes,
        trusted_public_key,
        require_signature,
        &mut diagnostics,
    )?;
    Ok(BundleVerification {
        schema_version: manifest.schema_version,
        valid: diagnostics.is_empty(),
        verified_files: manifest.files.len(),
        diagnostics,
        signature,
    })
}

fn verify_signature(
    bundle: &Path,
    manifest_bytes: &[u8],
    trusted_public_key: Option<&[u8; 32]>,
    require_signature: bool,
    diagnostics: &mut Vec<String>,
) -> Result<BundleSignatureVerification, Error> {
    let path = bundle.join("bundle.sig.json");
    if !path.exists() {
        if require_signature || trusted_public_key.is_some() {
            diagnostics.push("bundle signature is required but bundle.sig.json is missing".into());
        }
        return Ok(BundleSignatureVerification {
            present: false,
            cryptographically_valid: false,
            trusted_key_match: trusted_public_key.map(|_| false),
            key_id: None,
        });
    }
    let envelope: BundleSignature =
        serde_json::from_slice(&fs::read(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?)
        .map_err(|error| Error::ProviderFailure(format!("{}: {error}", path.display())))?;
    let mut cryptographically_valid = true;
    if envelope.schema_version != SIGNATURE_SCHEMA {
        diagnostics.push(format!(
            "expected signature schema {SIGNATURE_SCHEMA}, observed {}",
            envelope.schema_version
        ));
        cryptographically_valid = false;
    }
    if envelope.algorithm != "Ed25519" {
        diagnostics.push(format!(
            "unsupported signature algorithm {}",
            envelope.algorithm
        ));
        cryptographically_valid = false;
    }
    if envelope.manifest_sha256 != hash(manifest_bytes) {
        diagnostics.push("signed manifest SHA-256 does not match bundle.json".into());
        cryptographically_valid = false;
    }
    let public_key = STANDARD_NO_PAD
        .decode(&envelope.public_key)
        .ok()
        .and_then(|value| <[u8; 32]>::try_from(value).ok());
    let signature = STANDARD_NO_PAD
        .decode(&envelope.signature)
        .ok()
        .and_then(|value| Signature::try_from(value.as_slice()).ok());
    match (public_key, signature) {
        (Some(public_key), Some(signature)) => {
            if envelope.key_id != hash(&public_key) {
                diagnostics.push("signature key_id does not match embedded public key".into());
                cryptographically_valid = false;
            }
            match VerifyingKey::from_bytes(&public_key) {
                Ok(key) if key.verify_strict(manifest_bytes, &signature).is_ok() => {}
                _ => {
                    diagnostics.push("Ed25519 signature verification failed".into());
                    cryptographically_valid = false;
                }
            }
        }
        _ => {
            diagnostics.push("signature or public key is not valid unpadded base64".into());
            cryptographically_valid = false;
        }
    }
    let trusted_key_match = trusted_public_key.map(|trusted| public_key.as_ref() == Some(trusted));
    if trusted_key_match == Some(false) {
        diagnostics.push("bundle signer does not match the trusted public key".into());
    }
    Ok(BundleSignatureVerification {
        present: true,
        cryptographically_valid,
        trusted_key_match,
        key_id: Some(envelope.key_id),
    })
}

pub fn read_key(path: &Path, label: &str) -> Result<[u8; 32], Error> {
    let bytes = fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if let Ok(key) = <[u8; 32]>::try_from(bytes.as_slice()) {
        return Ok(key);
    }
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim();
    if text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut key = [0u8; 32];
        for (index, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
                .map_err(|_| Error::ProviderFailure(format!("{label} is not valid hexadecimal")))?;
        }
        return Ok(key);
    }
    Err(Error::ProviderFailure(format!(
        "{label} must contain exactly 32 raw bytes or 64 hexadecimal characters: {}",
        path.display()
    )))
}

fn collect_files(
    directory: &Path,
    root: &Path,
    output: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), Error> {
    if !directory.is_dir() {
        return Ok(());
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?
        .map(|entry| entry.map(|item| item.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort();
    for path in entries {
        let metadata = fs::symlink_metadata(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(Error::ProviderFailure(format!(
                "bundle input contains a symbolic link: {}",
                path.display()
            )));
        }
        if metadata.is_dir() {
            collect_files(&path, root, output)?;
        } else if metadata.is_file()
            && !path.file_name().is_some_and(|name| {
                name.to_string_lossy().starts_with('.') || name.to_string_lossy().ends_with(".tmp")
            })
        {
            output.push((
                path.strip_prefix(root)
                    .expect("state file remains below root")
                    .to_path_buf(),
                path,
            ));
        }
    }
    Ok(())
}

fn collect_bundle_identities(
    directory: &Path,
    root: &Path,
    output: &mut Vec<String>,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(directory)
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?
        .map(|entry| entry.map(|item| item.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_bundle_identities(&path, root, output)?;
        } else if path.is_file() {
            output.push(path_identity(
                path.strip_prefix(root)
                    .expect("bundle file remains below data root"),
            ));
        }
    }
    Ok(())
}

fn safe_relative(value: &str) -> Result<PathBuf, Error> {
    let path = PathBuf::from(value);
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(Error::ProviderFailure(format!(
            "unsafe bundle path `{value}`"
        )));
    }
    Ok(path)
}

fn path_identity(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
