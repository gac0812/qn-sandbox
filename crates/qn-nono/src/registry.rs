//! Embedded official-profile catalog and host profile registry (#75/#59).
//!
//! The compiled catalog is the trusted source for the two official qn profiles.
//! Assets are embedded at build time; the manifest can only name those known
//! assets, so no publication path can escape the crate. The host registry stores
//! installed copies under `QN_HOME/profiles/<namespace>/<name>/versions/<v>/`
//! together with a SHA-256 digest and selects one version through `current`.
//!
//! Registry operations perform bounded local filesystem work only. They never
//! start an engine, resolve a credential or grant sandbox access.

use crate::NonoError;
use camino::{Utf8Path, Utf8PathBuf};
use qn_core::ProfileRef;
use qn_core::profile::{
    MAX_PROFILE_BYTES, ProfileError, ProfileRegistry, ProfileSource, ProfileSummary,
    ProfileVersion, ResolvedProfile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
};

const MANIFEST: &str = include_str!("../../../profiles/manifest.json");
const CLAUDE_ASSET: &str = include_str!("../../../profiles/claude-code.json");
const CODEX_ASSET: &str = include_str!("../../../profiles/codex.json");

const PROFILE_FILE: &str = "profile.json";
const VERSION_MANIFEST: &str = "manifest.json";
const CURRENT: &str = "current";
const STORED_SCHEMA: u8 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: u8,
    profiles: Vec<RawEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEntry {
    id: String,
    version: String,
    nono_asset: String,
    app: String,
    state_dir: String,
}

#[derive(Deserialize)]
struct RawAsset {
    meta: RawMeta,
    network: Option<RawNetwork>,
}

#[derive(Deserialize)]
struct RawMeta {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct RawNetwork {
    #[serde(default)]
    block: bool,
    #[serde(default)]
    allow_domain: Vec<serde_json::Value>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StoredManifest {
    schema_version: u8,
    id: String,
    version: String,
    sha256: String,
    state_dir: String,
}

struct CatalogEntry {
    summary: ProfileSummary,
    asset: &'static str,
    state_dir: String,
    hosts: Vec<String>,
}

/// Compiled official profiles; validation happens once at construction.
pub struct OfficialCatalog {
    entries: Vec<CatalogEntry>,
}

impl OfficialCatalog {
    /// Parse and validate the embedded manifest and both profile assets.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::InvalidCatalog`] when the manifest, an asset or
    /// its network/state metadata is malformed. No input text is echoed.
    pub fn embedded() -> Result<Self, ProfileError> {
        let raw: RawManifest =
            serde_json::from_str(MANIFEST).map_err(|_| ProfileError::InvalidCatalog)?;
        if raw.schema_version != 1 || raw.profiles.is_empty() {
            return Err(ProfileError::InvalidCatalog);
        }
        let mut entries: Vec<CatalogEntry> = Vec::with_capacity(raw.profiles.len());
        for entry in raw.profiles {
            let id = ProfileRef::try_from(entry.id.as_str())
                .map_err(|_| ProfileError::InvalidCatalog)?;
            if entries.iter().any(|current| current.summary.id() == &id) {
                return Err(ProfileError::InvalidCatalog);
            }
            let version = ProfileVersion::try_from(entry.version.as_str())
                .map_err(|_| ProfileError::InvalidCatalog)?;
            let asset =
                embedded_asset(entry.nono_asset.as_str()).ok_or(ProfileError::InvalidCatalog)?;
            if entry.app != app_name(entry.nono_asset.as_str()) {
                return Err(ProfileError::InvalidCatalog);
            }
            let (hosts, asset_version) = parse_asset(asset)?;
            if asset_version != version {
                return Err(ProfileError::InvalidCatalog);
            }
            entries.push(CatalogEntry {
                summary: ProfileSummary::new(id, version, ProfileSource::Builtin),
                asset,
                state_dir: entry.state_dir,
                hosts,
            });
        }
        Ok(Self { entries })
    }

    /// Compiled summaries in manifest order.
    #[must_use]
    pub fn summaries(&self) -> Vec<ProfileSummary> {
        self.entries
            .iter()
            .map(|entry| entry.summary.clone())
            .collect()
    }

    /// Resolve one compiled profile.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::UnknownProfile`] for an identity that is not in
    /// the embedded catalog.
    pub fn resolve(&self, id: &ProfileRef) -> Result<ResolvedProfile, ProfileError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.summary.id() == id)
            .ok_or(ProfileError::UnknownProfile)?;
        ResolvedProfile::new(
            entry.summary.clone(),
            entry.asset.to_owned(),
            entry.hosts.clone(),
            entry.state_dir.clone(),
        )
    }
}

fn embedded_asset(name: &str) -> Option<&'static str> {
    match name {
        "claude-code.json" => Some(CLAUDE_ASSET),
        "codex.json" => Some(CODEX_ASSET),
        _ => None,
    }
}

fn app_name(asset: &str) -> &'static str {
    match asset {
        "claude-code.json" => "claude-code",
        "codex.json" => "codex",
        _ => "",
    }
}

fn parse_asset(asset: &str) -> Result<(Vec<String>, ProfileVersion), ProfileError> {
    if asset.len() > MAX_PROFILE_BYTES {
        return Err(ProfileError::InvalidCatalog);
    }
    let raw: RawAsset = serde_json::from_str(asset).map_err(|_| ProfileError::InvalidCatalog)?;
    if raw.meta.name.is_empty() {
        return Err(ProfileError::InvalidCatalog);
    }
    let version = ProfileVersion::try_from(raw.meta.version.as_str())
        .map_err(|_| ProfileError::InvalidCatalog)?;
    let network = raw.network.ok_or(ProfileError::InvalidCatalog)?;
    if network.block || network.allow_domain.is_empty() {
        return Err(ProfileError::InvalidCatalog);
    }
    let mut hosts = Vec::with_capacity(network.allow_domain.len());
    for value in network.allow_domain {
        let host = value.as_str().ok_or(ProfileError::InvalidCatalog)?;
        if host == "*"
            || host.is_empty()
            || host.len() > 253
            || host
                .bytes()
                .any(|byte| !(byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'*')))
            || hosts.iter().any(|existing| existing == host)
        {
            return Err(ProfileError::InvalidCatalog);
        }
        hosts.push(host.to_owned());
    }
    Ok((hosts, version))
}

/// Host-owned installed profile registry rooted at `QN_HOME/profiles`.
pub struct HostProfileRegistry {
    root: Utf8PathBuf,
    catalog: OfficialCatalog,
}

impl HostProfileRegistry {
    /// Bind an absolute private registry root and load the embedded catalog.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::InvalidRoot`] for a relative/NUL root and
    /// [`ProfileError::InvalidCatalog`] when the embedded catalog is invalid.
    pub fn new(root: Utf8PathBuf) -> Result<Self, ProfileError> {
        if !root.is_absolute() || root.as_str().contains('\0') {
            return Err(ProfileError::InvalidRoot);
        }
        Ok(Self {
            root,
            catalog: OfficialCatalog::embedded()?,
        })
    }

    /// Build the strict native validator for one installed profile asset.
    ///
    /// The caller must verify engine integrity before executing the command.
    ///
    /// # Errors
    ///
    /// Returns [`NonoError`] when the resolved asset is unavailable or the
    /// profile path cannot be represented safely.
    pub fn validation_args(
        &self,
        id: &ProfileRef,
        asset_path: &Utf8Path,
    ) -> Result<crate::NonoArgs, NonoError> {
        self.catalog
            .resolve(id)
            .map_err(|_| NonoError::UnsupportedProfile)?;
        crate::NonoArgs::profile_validate(asset_path)
    }

    fn id_dir(&self, id: &ProfileRef) -> Utf8PathBuf {
        let (namespace, name) = id
            .as_str()
            .split_once('/')
            .unwrap_or((id.as_str(), id.as_str()));
        self.root.join(namespace).join(name)
    }

    fn read_current(id_dir: &Utf8Path) -> Result<Option<ProfileVersion>, ProfileError> {
        let path = id_dir.join(CURRENT);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(ProfileError::Io(error)),
            Ok(metadata) if !metadata.file_type().is_file() => Err(ProfileError::Integrity),
            Ok(_) => {
                let bytes = read_bounded(&path)?;
                let text = std::str::from_utf8(&bytes).map_err(|_| ProfileError::Integrity)?;
                let version = text.trim_end_matches('\n');
                if version.is_empty() || version.contains('\n') {
                    return Err(ProfileError::Integrity);
                }
                ProfileVersion::try_from(version)
                    .map(Some)
                    .map_err(|_| ProfileError::Integrity)
            }
        }
    }

    fn write_current(id_dir: &Utf8Path, version: &ProfileVersion) -> Result<(), ProfileError> {
        let mut file = tempfile::NamedTempFile::new_in(id_dir).map_err(ProfileError::Io)?;
        file.write_all(format!("{version}\n").as_bytes())
            .map_err(ProfileError::Io)?;
        file.as_file().sync_all().map_err(ProfileError::Io)?;
        file.persist(id_dir.join(CURRENT))
            .map_err(|error| ProfileError::Io(error.error))?;
        Ok(())
    }

    fn installed_versions(&self, id: &ProfileRef) -> Result<Vec<ProfileVersion>, ProfileError> {
        let versions = self.id_dir(id).join("versions");
        let directory = match fs::symlink_metadata(&versions) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(ProfileError::Io(error)),
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ProfileError::Integrity);
            }
            Ok(metadata) if !metadata.is_dir() => return Err(ProfileError::Integrity),
            Ok(_) => versions,
        };
        let mut found = Vec::new();
        for entry in fs::read_dir(&directory).map_err(ProfileError::Io)? {
            let entry = entry.map_err(ProfileError::Io)?;
            let file_type = entry.file_type().map_err(ProfileError::Io)?;
            if file_type.is_symlink() {
                return Err(ProfileError::Integrity);
            }
            if !file_type.is_dir() {
                continue;
            }
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| ProfileError::Integrity)?;
            let version =
                ProfileVersion::try_from(name.as_str()).map_err(|_| ProfileError::Integrity)?;
            found.push(version);
        }
        found.sort();
        Ok(found)
    }

    fn read_version(
        &self,
        id: &ProfileRef,
        version: &ProfileVersion,
    ) -> Result<ResolvedProfile, ProfileError> {
        let dir = self.id_dir(id).join("versions").join(version.as_str());
        let stored_bytes = read_bounded(&dir.join(VERSION_MANIFEST))?;
        let stored: StoredManifest =
            serde_json::from_slice(&stored_bytes).map_err(|_| ProfileError::Integrity)?;
        if stored.schema_version != STORED_SCHEMA
            || stored.id != id.as_str()
            || stored.version != version.as_str()
            || stored.sha256.len() != 64
        {
            return Err(ProfileError::Integrity);
        }
        let asset = read_bounded(&dir.join(PROFILE_FILE))?;
        if hex::encode(Sha256::digest(&asset)) != stored.sha256 {
            return Err(ProfileError::Integrity);
        }
        let text = std::str::from_utf8(&asset).map_err(|_| ProfileError::Integrity)?;
        let (hosts, asset_version) = parse_asset(text)?;
        if asset_version != *version {
            return Err(ProfileError::Integrity);
        }
        let summary = ProfileSummary::new(id.clone(), version.clone(), ProfileSource::Installed);
        ResolvedProfile::new(summary, text.to_owned(), hosts, stored.state_dir)
    }

    fn publish(
        &self,
        id: &ProfileRef,
        version: &ProfileVersion,
        asset: &str,
        state_dir: &str,
    ) -> Result<(), ProfileError> {
        let id_dir = self.id_dir(id);
        let versions = id_dir.join("versions");
        fs::create_dir_all(&versions).map_err(ProfileError::Io)?;
        let target = versions.join(version.as_str());
        let digest = hex::encode(Sha256::digest(asset.as_bytes()));
        let manifest = StoredManifest {
            schema_version: STORED_SCHEMA,
            id: id.as_str().to_owned(),
            version: version.to_string(),
            sha256: digest.clone(),
            state_dir: state_dir.to_owned(),
        };
        if fs::symlink_metadata(&target).is_ok() {
            let existing = read_bounded(&target.join(VERSION_MANIFEST))?;
            let existing: StoredManifest =
                serde_json::from_slice(&existing).map_err(|_| ProfileError::Integrity)?;
            if existing.sha256 != digest
                || existing.id != id.as_str()
                || existing.version != version.as_str()
                || existing.state_dir != state_dir
            {
                return Err(ProfileError::Conflict);
            }
            return Ok(());
        }
        let staging = tempfile::Builder::new()
            .prefix(".publish-")
            .tempdir_in(&versions)
            .map_err(ProfileError::Io)?;
        {
            // 句柄必须在此块内关闭：Windows 上重命名目录要求其下没有打开的
            // 文件句柄（error 5 / ACCESS_DENIED），POSIX 无此限制——macOS 上
            // 先前因句柄未关也能通过，故该缺陷在 macOS 不可见。
            let mut profile =
                fs::File::create(staging.path().join(PROFILE_FILE)).map_err(ProfileError::Io)?;
            profile
                .write_all(asset.as_bytes())
                .map_err(ProfileError::Io)?;
            profile.sync_all().map_err(ProfileError::Io)?;
            let mut stored = fs::File::create(staging.path().join(VERSION_MANIFEST))
                .map_err(ProfileError::Io)?;
            stored
                .write_all(
                    serde_json::to_string(&manifest)
                        .map_err(|_| ProfileError::InvalidCatalog)?
                        .as_bytes(),
                )
                .map_err(ProfileError::Io)?;
            stored.sync_all().map_err(ProfileError::Io)?;
        }
        fs::rename(staging.path(), &target).map_err(ProfileError::Io)?;
        Ok(())
    }

    /// Publish a validated asset as a new version of a compiled official profile.
    ///
    /// The caller must validate the asset with the pinned engine first. The
    /// identity and state directory must match the compiled catalog, so callers
    /// cannot introduce a new app state path. The asset meta version must match
    /// `version`; conflicting bytes for an existing version are refused.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError`] for an unknown identity, state-dir mismatch,
    /// malformed asset, version mismatch, conflict or IO failure.
    pub fn publish_asset(
        &self,
        id: ProfileRef,
        version: ProfileVersion,
        asset: &str,
        state_dir: &str,
    ) -> Result<ProfileSummary, ProfileError> {
        let known = self.catalog.resolve(&id)?;
        if known.state_dir() != state_dir {
            return Err(ProfileError::InvalidCatalog);
        }
        let (_, asset_version) = parse_asset(asset)?;
        if asset_version != version {
            return Err(ProfileError::InvalidVersion);
        }
        self.publish(&id, &version, asset, state_dir)?;
        Self::write_current(&self.id_dir(&id), &version)?;
        Ok(ProfileSummary::new(id, version, ProfileSource::Installed))
    }
}

impl ProfileRegistry for HostProfileRegistry {
    fn list(&self) -> Result<Vec<ProfileSummary>, ProfileError> {
        let mut summaries = self.catalog.summaries();
        let namespaces = match fs::symlink_metadata(&self.root) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(summaries),
            Err(error) => return Err(ProfileError::Io(error)),
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(ProfileError::Integrity);
            }
            Ok(metadata) if !metadata.is_dir() => return Err(ProfileError::Integrity),
            Ok(_) => fs::read_dir(&self.root).map_err(ProfileError::Io)?,
        };
        let mut installed = Vec::new();
        for namespace in namespaces {
            let namespace = namespace.map_err(ProfileError::Io)?;
            if !is_component(&namespace.file_name()) {
                continue;
            }
            if namespace
                .file_type()
                .map_err(ProfileError::Io)?
                .is_symlink()
            {
                return Err(ProfileError::Integrity);
            }
            if !namespace.file_type().map_err(ProfileError::Io)?.is_dir() {
                continue;
            }
            let namespace_name = namespace
                .file_name()
                .into_string()
                .map_err(|_| ProfileError::Integrity)?;
            for name in fs::read_dir(namespace.path()).map_err(ProfileError::Io)? {
                let name = name.map_err(ProfileError::Io)?;
                if !is_component(&name.file_name()) {
                    continue;
                }
                if name.file_type().map_err(ProfileError::Io)?.is_symlink() {
                    return Err(ProfileError::Integrity);
                }
                if !name.file_type().map_err(ProfileError::Io)?.is_dir() {
                    continue;
                }
                let name_text = name
                    .file_name()
                    .into_string()
                    .map_err(|_| ProfileError::Integrity)?;
                let id = ProfileRef::try_from(format!("{namespace_name}/{name_text}").as_str())
                    .map_err(|_| ProfileError::Integrity)?;
                for version in self.installed_versions(&id)? {
                    let resolved = self.read_version(&id, &version)?;
                    installed.push(resolved.summary().clone());
                }
            }
        }
        installed.sort_by(|left, right| {
            left.id()
                .as_str()
                .cmp(right.id().as_str())
                .then_with(|| right.version().cmp(left.version()))
        });
        summaries.extend(installed);
        Ok(summaries)
    }

    fn resolve(&self, id: ProfileRef) -> Result<ResolvedProfile, ProfileError> {
        let id_dir = self.id_dir(&id);
        if let Some(version) = Self::read_current(&id_dir)? {
            return self.read_version(&id, &version);
        }
        self.catalog.resolve(&id)
    }

    fn install(&self, id: ProfileRef) -> Result<ProfileSummary, ProfileError> {
        let resolved = self.catalog.resolve(&id)?;
        let version = resolved.summary().version().clone();
        self.publish(&id, &version, resolved.nono_profile(), resolved.state_dir())?;
        Self::write_current(&self.id_dir(&id), &version)?;
        Ok(ProfileSummary::new(id, version, ProfileSource::Installed))
    }

    fn rollback(&self, id: ProfileRef) -> Result<ProfileSummary, ProfileError> {
        let id_dir = self.id_dir(&id);
        let current = Self::read_current(&id_dir)?.ok_or(ProfileError::NotInstalled)?;
        let versions = self.installed_versions(&id)?;
        let index = versions
            .iter()
            .position(|version| version == &current)
            .ok_or(ProfileError::Integrity)?;
        if index == 0 {
            return Err(ProfileError::NoRollback);
        }
        let previous = versions.get(index - 1).ok_or(ProfileError::Integrity)?;
        self.read_version(&id, previous)?;
        Self::write_current(&id_dir, previous)?;
        Ok(ProfileSummary::new(
            id,
            previous.clone(),
            ProfileSource::Installed,
        ))
    }
}

fn read_bounded(path: &Utf8Path) -> Result<Vec<u8>, ProfileError> {
    let metadata = fs::symlink_metadata(path).map_err(ProfileError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(ProfileError::Integrity);
    }
    let file = fs::File::open(path).map_err(ProfileError::Io)?;
    let mut bytes = Vec::new();
    file.take((MAX_PROFILE_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(ProfileError::Io)?;
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err(ProfileError::Integrity);
    }
    Ok(bytes)
}

fn is_component(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|name| {
        !name.is_empty()
            && name.len() <= 64
            && name
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphanumeric())
            && name.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            })
    })
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
