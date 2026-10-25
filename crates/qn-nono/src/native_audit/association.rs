//! Private retained lookup metadata, never an authorization or process handle.
use crate::{NonoError, args::NonoSessionId, launch::LaunchBinding};
use camino::{Utf8Path, Utf8PathBuf};
use qn_core::{SessionId, native_audit::NativeAuditError};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
};

const MAX_ASSOCIATION_BYTES: u64 = 16 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    session_id: String,
    native_id: String,
    runtime_root: String,
}
pub(super) struct Association {
    pub native: NonoSessionId,
    pub root: Utf8PathBuf,
}

pub(crate) fn publish(
    audit: &Utf8Path,
    root: &Utf8Path,
    binding: &LaunchBinding,
) -> Result<(), NonoError> {
    crate::args::validate_path(audit, "audit association root")?;
    crate::args::validate_path(root, "audit runtime root")?;
    let root = fs::canonicalize(root).map_err(io_error)?;
    let root = Utf8PathBuf::from_path_buf(root).map_err(|_| NonoError::InvalidArgument {
        field: "UTF-8 runtime root",
    })?;
    if !fs::metadata(&root).map_err(io_error)?.is_dir() {
        return Err(NonoError::InvalidArgument {
            field: "audit runtime root",
        });
    }
    let record = Record {
        session_id: binding.session().to_string(),
        native_id: binding.native_id().as_str().to_owned(),
        runtime_root: root.into_string(),
    };
    let bytes = serde_json::to_vec(&record).map_err(|source| NonoError::ProtocolJson {
        operation: "audit association",
        source: crate::error::ProtocolJsonError(source),
    })?;
    if bytes.len() as u64 > MAX_ASSOCIATION_BYTES {
        return Err(NonoError::InvalidArgument {
            field: "audit association size",
        });
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(audit.join(format!("{}.native.json", binding.session())))
        .map_err(io_error)?;
    file.write_all(&bytes).map_err(io_error)?;
    file.sync_all().map_err(io_error)
}

pub(super) fn load(audit: &Utf8Path, session: SessionId) -> Result<Association, NativeAuditError> {
    let path = audit.join(format!("{session}.native.json"));
    let metadata = fs::symlink_metadata(&path).map_err(read_error)?;
    if !metadata.is_file() || metadata.len() > MAX_ASSOCIATION_BYTES {
        return Err(NativeAuditError::InvalidAssociation);
    }
    let file = fs::File::open(path).map_err(read_error)?;
    if !file.metadata().map_err(read_error)?.is_file() {
        return Err(NativeAuditError::InvalidAssociation);
    }
    let mut bytes = Vec::new();
    file.take(MAX_ASSOCIATION_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(read_error)?;
    if bytes.len() as u64 > MAX_ASSOCIATION_BYTES
        || bytes.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{')
    {
        return Err(NativeAuditError::InvalidAssociation);
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(|source| {
        super::failure(NonoError::ProtocolJson {
            operation: "audit association",
            source: crate::error::ProtocolJsonError(source),
        })
    })?;
    let runtime_root = Utf8PathBuf::from(record.runtime_root);
    if SessionId::try_from(record.session_id.as_str()).ok() != Some(session)
        || !runtime_root.is_absolute()
        || runtime_root.as_str().contains(char::from(0))
    {
        return Err(NativeAuditError::InvalidAssociation);
    }
    let native = NonoSessionId::try_from(record.native_id.as_str())
        .map_err(|_| NativeAuditError::InvalidAssociation)?;
    let resolved = fs::canonicalize(&runtime_root).map_err(read_error)?;
    if resolved != runtime_root.as_std_path()
        || !fs::metadata(&resolved).map_err(read_error)?.is_dir()
    {
        return Err(NativeAuditError::InvalidAssociation);
    }
    Ok(Association {
        native,
        root: runtime_root,
    })
}
fn io_error(source: std::io::Error) -> NonoError {
    NonoError::Io {
        step: "publishing audit association",
        source,
    }
}
fn read_error(source: std::io::Error) -> NativeAuditError {
    if source.kind() == std::io::ErrorKind::NotFound {
        NativeAuditError::MissingAssociation
    } else {
        super::failure(NonoError::Io {
            step: "reading audit association",
            source,
        })
    }
}
