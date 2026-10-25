//! Runtime composition root for qn-sandbox.

use process_wrap::tokio::TokioCommandWrap;
use qn_core::profile::ProfileRegistry;
use qn_core::runtime::CleanupOutcome;
use qn_core::setup::EngineSetup;
use qn_infra::process::{ProcessPolicy, TerminationSafety};

mod cleanup;
use cleanup::{SessionCleanup, reap_session};

struct HostProcessPolicy;
impl ProcessPolicy for HostProcessPolicy {
    fn configure(&self, command: &mut TokioCommandWrap) -> TerminationSafety {
        #[cfg(unix)]
        {
            command.wrap(process_wrap::tokio::ProcessGroup::leader());
            TerminationSafety::WhileParentOwned
        }
        #[cfg(windows)]
        {
            command.wrap(process_wrap::tokio::JobObject);
            TerminationSafety::RetainedGroupHandle
        }
    }
}

/// Assemble native group/job lifecycle control without exposing platform branches.
///
/// ```no_run
/// # async fn example() -> Result<(), qn_infra::Error> {
/// use qn_infra::process::{CmdSpec, spawn_managed};
/// let spec = CmdSpec::new("/absolute/path/to/program").env_clear();
/// let process = spawn_managed(&spec, &qn_bootstrap::process_policy())?;
/// let output = process.wait(Default::default()).await?;
/// assert!(output.output().success);
/// # Ok(()) }
/// ```
#[must_use]
pub fn process_policy() -> impl ProcessPolicy {
    HostProcessPolicy
}

/// Compose the versioned official-profile catalog and host registry.
///
/// The returned registry performs bounded local storage work only; it never
/// starts an engine or resolves a credential.
///
/// # Errors
///
/// Rejects a relative profile root and an invalid embedded catalog.
pub fn profile_registry(
    paths: &qn_config::paths::QnPaths,
) -> Result<impl qn_core::profile::ProfileRegistry + use<>, qn_core::profile::ProfileError> {
    qn_nono::registry::HostProfileRegistry::new(paths.profiles.clone())
}

/// Assemble the pinned native installer/inspector under the resolved cache root.
///
/// # Errors
/// Rejects invalid cache roots and hosts requiring managed WSL2 installation.
///
///     let paths = qn_config::paths::QnPaths::from_root("/tmp/qn");
///     let setup = qn_bootstrap::engine_setup(&paths);
///
pub fn engine_setup(
    paths: &qn_config::paths::QnPaths,
) -> Result<impl qn_core::setup::EngineSetup + use<>, qn_core::setup::SetupError> {
    qn_nono::setup::NonoSetup::new(&paths.engine_cache())
}

/// Compose a bounded broker journal reader under the trusted host audit root.
/// # Errors
/// Rejects invalid roots; reads preserve missing/invalid/incomplete distinctions.
pub fn audit_query(
    root: camino::Utf8PathBuf,
) -> Result<impl qn_core::audit::AuditQuery + use<>, qn_core::audit::AuditQueryError> {
    qn_broker::audit_query::BrokerAuditQuery::new(root)
}

/// Compose a retained native query with trusted paths and the host process policy.
/// # Errors
/// Rejects invalid roots and hosts requiring the WSL engine installation route.
pub fn native_audit_query(
    paths: &qn_config::paths::QnPaths,
) -> Result<
    impl qn_core::native_audit::NativeAuditQuery + use<>,
    qn_core::native_audit::NativeAuditError,
> {
    qn_nono::native_audit::ManagedNativeAudit::new(
        paths.audit.clone(),
        &paths.engine_cache(),
        std::sync::Arc::new(process_policy()),
    )
}

/// Validate one resolved profile with the pinned engine's strict validator.
///
/// The managed binary's archive and bytes are verified before execution. The
/// asset is written to a private temporary file under `QN_HOME`; no credential
/// is resolved and no sandbox is started.
///
/// # Errors
///
/// Returns [`qn_core::profile::ProfileError`] for an unknown/uninstalled
/// profile, engine integrity failure, invalid asset or failed native validation.
pub async fn validate_profile(
    paths: &qn_config::paths::QnPaths,
    id: qn_core::ProfileRef,
) -> Result<qn_core::profile::ProfileSummary, qn_core::profile::ProfileError> {
    let registry = profile_registry(paths)?;
    let resolved = registry.resolve(id)?;
    validate_asset(paths, resolved.nono_profile()).await?;
    Ok(resolved.summary().clone())
}

/// Validate a locally supplied official-profile asset and publish it as a new version.
///
/// The identity must be one of the compiled official profiles, so the app state
/// directory cannot be chosen by the caller. The asset must be an absolute,
/// bounded, regular UTF-8 file. It is validated by the pinned engine before any
/// registry write; publication then selects the new version atomically.
///
/// # Errors
///
/// Returns [`qn_core::profile::ProfileError`] for an unknown identity, an
/// invalid/oversized/non-regular file, a version mismatch, engine failure or a
/// conflicting existing version.
pub async fn publish_profile(
    paths: &qn_config::paths::QnPaths,
    id: qn_core::ProfileRef,
    version: &str,
    asset_path: &str,
) -> Result<qn_core::profile::ProfileSummary, qn_core::profile::ProfileError> {
    let version = qn_core::profile::ProfileVersion::try_from(version)?;
    let path = camino::Utf8Path::new(asset_path);
    if !path.is_absolute() {
        return Err(qn_core::profile::ProfileError::InvalidAssetPath);
    }
    let asset = read_asset(path)?;
    let registry = qn_nono::registry::HostProfileRegistry::new(paths.profiles.clone())?;
    let state_dir = registry.resolve(id.clone())?.state_dir().to_owned();
    validate_asset(paths, &asset).await?;
    registry.publish_asset(id, version, &asset, &state_dir)
}

async fn validate_asset(
    paths: &qn_config::paths::QnPaths,
    asset: &str,
) -> Result<(), qn_core::profile::ProfileError> {
    use std::io::Write as _;

    let setup = engine_setup(paths).map_err(qn_core::profile::ProfileError::Engine)?;
    let installation = setup
        .run(qn_core::setup::SetupOperation::Inspect)
        .await
        .map_err(qn_core::profile::ProfileError::Engine)?;
    std::fs::create_dir_all(&paths.cache).map_err(qn_core::profile::ProfileError::Io)?;
    let mut file = tempfile::NamedTempFile::new_in(&paths.cache)
        .map_err(qn_core::profile::ProfileError::Io)?;
    file.write_all(asset.as_bytes())
        .map_err(qn_core::profile::ProfileError::Io)?;
    file.as_file()
        .sync_all()
        .map_err(qn_core::profile::ProfileError::Io)?;
    let profile_path = camino::Utf8PathBuf::from_path_buf(file.path().to_owned())
        .map_err(|_| qn_core::profile::ProfileError::ValidationUnavailable)?;
    let args = qn_nono::NonoArgs::profile_validate(&profile_path)
        .map_err(|_| qn_core::profile::ProfileError::ValidationUnavailable)?;
    let spec = qn_infra::process::CmdSpec::new(installation.path.as_str())
        .args(args.as_slice().iter().cloned())
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
        .env("LANG", "C")
        .timeout(std::time::Duration::from_secs(15))
        .label("nono profile validate");
    let validation = qn_infra::process::run_checked(&spec).await;
    let closed = file.close();
    validation.map_err(|error| qn_core::profile::ProfileError::Validation(Box::new(error)))?;
    closed.map_err(qn_core::profile::ProfileError::Io)?;
    Ok(())
}

fn read_asset(path: &camino::Utf8Path) -> Result<String, qn_core::profile::ProfileError> {
    let metadata = std::fs::symlink_metadata(path).map_err(qn_core::profile::ProfileError::Io)?;
    if !metadata.file_type().is_file()
        || metadata.len() > qn_core::profile::MAX_PROFILE_BYTES as u64
    {
        return Err(qn_core::profile::ProfileError::InvalidCatalog);
    }
    let bytes = std::fs::read(path).map_err(qn_core::profile::ProfileError::Io)?;
    if bytes.len() > qn_core::profile::MAX_PROFILE_BYTES {
        return Err(qn_core::profile::ProfileError::InvalidCatalog);
    }
    String::from_utf8(bytes).map_err(|_| qn_core::profile::ProfileError::InvalidCatalog)
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SessionRecord {
    schema_version: u8,
    session_id: String,
    profile: String,
    native_id: String,
    runtime_root: String,
    phase: qn_core::runtime::RunPhase,
    end: Option<qn_core::runtime::RunEnd>,
    exit_code: Option<i32>,
    cleanup: qn_core::runtime::CleanupState,
    #[serde(default)]
    cleanup_terminated: u32,
}

/// Run one official profile in the foreground and return an exit observation.
///
/// The managed parent inherits the terminal. The engine archive/binary is
/// verified before execution, a predetermined native identity is reserved, and
/// the launch-owned audit association is published before spawn.
///
/// Cleanup is proved separately from the parent's exit (#93): a tracker records
/// descendants, and the reaper freezes, identity-checks and kills every
/// observed/marker-carrying process before reporting `Confirmed`. If that
/// bounded proof cannot run, the report stays `Unconfirmed`.
///
/// Cancel the token to request termination; the managed group is stopped and
/// awaited, then the same session-scoped reaper runs.
///
/// # Errors
///
/// Returns [`qn_core::runtime::SessionError`] for profile/engine failures,
/// unsupported hosts, invalid storage and native process errors.
pub async fn run_session(
    paths: &qn_config::paths::QnPaths,
    spec: qn_core::RunSpec,
    resolved: &qn_core::profile::ResolvedProfile,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<qn_core::runtime::RunReport, qn_core::runtime::SessionError> {
    use qn_core::runtime::{CleanupOutcome, RunEnd, RunPhase, RunReport, SessionError};

    if spec.profile() != resolved.summary().id() {
        return Err(SessionError::InvalidRequest);
    }
    let setup = engine_setup(paths).map_err(|error| SessionError::Engine(Box::new(error)))?;
    let installation = setup
        .run(qn_core::setup::SetupOperation::Inspect)
        .await
        .map_err(|error| SessionError::Engine(Box::new(error)))?;
    create_private_dir(&paths.runtime)?;
    create_private_dir(&paths.audit)?;
    let control = paths.runtime.join("control");
    create_private_dir(&control)?;
    let binding = qn_nono::launch::LaunchBinding::reserve(&control, spec.id())
        .map_err(|error| SessionError::Process(Box::new(error)))?;
    let root = paths.runtime.join(spec.id().to_string());
    create_private_dir(&root)?;
    let home = qn_config::paths::platform_host_home().ok_or(SessionError::Unsupported)?;
    let runtime_paths = qn_nono::profile::RuntimePaths {
        control_root: control,
        host_homes: vec![home],
        engine_binary: installation.path,
        tool_dirs: tool_dirs()?,
    };
    let prepared = qn_nono::profile::PreparedOfficial::prepare(
        spec.clone(),
        &runtime_paths,
        root.clone(),
        resolved.nono_profile(),
        resolved.state_dir(),
    )
    .map_err(|error| SessionError::Process(Box::new(error)))?;
    binding
        .publish_audit(&paths.audit, &paths.runtime)
        .map_err(|error| SessionError::Process(Box::new(error)))?;
    let command = prepared
        .bound_run_command(&binding)
        .map_err(|error| SessionError::Process(Box::new(error)))?;
    let process = qn_infra::process::spawn_managed(&command, &process_policy())
        .map_err(|error| SessionError::Process(Box::new(error)))?;
    let root_pid = process.pid().ok_or_else(|| {
        SessionError::Process(Box::new(std::io::Error::other(
            "managed process has no owned pid",
        )))
    })?;
    let cleanup = SessionCleanup::start(root_pid, prepared.cleanup_markers());

    let record = |phase, end, exit_code, cleanup: CleanupOutcome| SessionRecord {
        schema_version: 1,
        session_id: spec.id().to_string(),
        profile: spec.profile().as_str().to_owned(),
        native_id: binding.native_id().as_str().to_owned(),
        runtime_root: root.as_str().to_owned(),
        phase,
        end,
        exit_code,
        cleanup: cleanup.state(),
        cleanup_terminated: cleanup.terminated(),
    };
    if let Err(error) = write_record(
        paths,
        &record(RunPhase::Running, None, None, CleanupOutcome::unconfirmed()),
    ) {
        let _ = finish_cleanup(cleanup).await;
        return Err(error);
    }
    let output = match process.wait(cancel).await {
        Ok(output) => output,
        Err(error) => {
            let cleanup = finish_cleanup(cleanup).await;
            let _ = write_record(paths, &record(RunPhase::Failed, None, None, cleanup));
            return Err(SessionError::Process(Box::new(error)));
        }
    };
    let cleanup = finish_cleanup(cleanup).await;
    let end = match output.end {
        qn_infra::process::ProcessEnd::Exited => RunEnd::Exited,
        qn_infra::process::ProcessEnd::Cancelled => RunEnd::Cancelled,
        qn_infra::process::ProcessEnd::TimedOut => RunEnd::TimedOut,
    };
    let exit_code = output.output().exit_code;
    let report = RunReport::new(
        spec.id(),
        qn_core::ProfileRef::try_from(spec.profile().as_str())
            .map_err(|_| SessionError::InvalidRequest)?,
        binding.native_id().as_str().to_owned(),
        RunPhase::ExitedObserved,
        end,
        exit_code,
        cleanup,
    );
    write_record(
        paths,
        &record(RunPhase::ExitedObserved, Some(end), exit_code, cleanup),
    )?;
    Ok(report)
}

async fn finish_cleanup(cleanup: SessionCleanup) -> CleanupOutcome {
    match tokio::task::spawn_blocking(move || cleanup.finish()).await {
        Ok(outcome) => outcome,
        Err(_) => CleanupOutcome::unconfirmed(),
    }
}

/// Read the persisted owner record and, when available, live native state.
///
/// A missing engine or native query does not hide the owner record. The result
/// carries the persisted reaper outcome; it never claims a core `Stopped`
/// transition from a native observation.
///
/// # Errors
///
/// Returns [`qn_core::runtime::SessionError::UnknownSession`] when no record
/// exists and storage/validation errors for an unreadable record.
pub async fn status_session(
    paths: &qn_config::paths::QnPaths,
    id: qn_core::SessionId,
) -> Result<qn_core::runtime::SessionStatus, qn_core::runtime::SessionError> {
    use qn_core::runtime::{CleanupOutcome, SessionStatus};

    let record = read_record(paths, id)?;
    let profile = qn_core::ProfileRef::try_from(record.profile.as_str())
        .map_err(|_| qn_core::runtime::SessionError::InvalidRequest)?;
    let runtime_root = camino::Utf8PathBuf::from(record.runtime_root.as_str());
    let native = observe_native(paths, id, runtime_root).await.ok().flatten();
    Ok(SessionStatus::new(
        id,
        profile,
        record.phase,
        record.end,
        record.exit_code,
        CleanupOutcome::new(record.cleanup, record.cleanup_terminated),
        native,
    ))
}

/// Ask the native engine to stop a recorded session, then reap its tree.
///
/// The stop request uses the predetermined native ID from the owner record,
/// never a numeric PID or PGID. An already-exited record skips the native
/// request but still runs the session-scoped reaper, because a descendant may
/// have outlived the managed parent. The report is `Confirmed` only when the
/// reaper's marker/identity/quiescence proof completes.
///
/// # Errors
///
/// Returns [`qn_core::runtime::SessionError`] for unknown sessions, engine
/// failure and native stop failure.
pub async fn stop_session(
    paths: &qn_config::paths::QnPaths,
    id: qn_core::SessionId,
    mode: qn_core::runtime::StopMode,
) -> Result<qn_core::runtime::StopReport, qn_core::runtime::SessionError> {
    use qn_core::runtime::{RunPhase, SessionError, StopMode, StopReport};

    let mut record = read_record(paths, id)?;
    let runtime_root = camino::Utf8PathBuf::from(record.runtime_root.as_str());
    let markers = qn_nono::profile::cleanup_markers(&runtime_root);
    let requested = record.phase != RunPhase::ExitedObserved;

    if requested {
        let native_id = qn_nono::args::NonoSessionId::try_from(record.native_id.as_str())
            .map_err(|error| SessionError::Process(Box::new(error)))?;
        let setup = engine_setup(paths).map_err(|error| SessionError::Engine(Box::new(error)))?;
        let installation = setup
            .run(qn_core::setup::SetupOperation::Inspect)
            .await
            .map_err(|error| SessionError::Engine(Box::new(error)))?;
        let native_mode = match mode {
            StopMode::Graceful { timeout_secs } => qn_nono::args::NonoStopMode::Graceful {
                timeout: std::time::Duration::from_secs(timeout_secs),
            },
            StopMode::Force => qn_nono::args::NonoStopMode::Force,
        };
        let args = qn_nono::NonoArgs::stop(&native_id, native_mode)
            .map_err(|error| SessionError::Process(Box::new(error)))?;
        let command = session_environment(
            qn_infra::process::CmdSpec::new(installation.path.as_str())
                .args(args.as_slice().iter().cloned())
                .env_clear()
                .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
                .env("LANG", "C"),
            &runtime_root,
        )
        .timeout(std::time::Duration::from_secs(320))
        .label("nono stop");
        qn_infra::process::run_checked(&command)
            .await
            .map_err(|error| SessionError::Process(Box::new(error)))?;
    }

    let cleanup = finish_reap(markers).await;
    // Only the owner that awaited the managed parent writes the phase; a stop
    // request is not an exit observation. The record is written last so a
    // concurrent owner write stays authoritative, and a failed write leaves the
    // pre-stop record rather than a fabricated phase.
    record.cleanup = cleanup.state();
    record.cleanup_terminated = cleanup.terminated();
    let _ = write_record(paths, &record);
    Ok(StopReport::new(id, requested, cleanup))
}

async fn finish_reap(markers: Vec<String>) -> CleanupOutcome {
    match tokio::task::spawn_blocking(move || reap_session(&markers)).await {
        Ok(outcome) => outcome,
        Err(_) => CleanupOutcome::unconfirmed(),
    }
}

async fn observe_native(
    paths: &qn_config::paths::QnPaths,
    id: qn_core::SessionId,
    runtime_root: camino::Utf8PathBuf,
) -> Result<Option<qn_core::runtime::ObservedNativeState>, qn_core::runtime::SessionError> {
    use qn_core::runtime::{ObservedNativeState, SessionError};

    let setup = engine_setup(paths).map_err(|error| SessionError::Engine(Box::new(error)))?;
    let installation = setup
        .run(qn_core::setup::SetupOperation::Inspect)
        .await
        .map_err(|error| SessionError::Engine(Box::new(error)))?;
    let reader = qn_nono::session::SessionReader::new(
        installation.path,
        runtime_root,
        std::sync::Arc::new(process_policy()),
    )
    .map_err(|error| SessionError::Process(Box::new(error)))?;
    let observed = reader
        .find(id)
        .await
        .map_err(|error| SessionError::Process(Box::new(error)))?;
    Ok(observed.map(|session| match session.state() {
        qn_nono::session::NativeSessionState::Running => ObservedNativeState::Running,
        qn_nono::session::NativeSessionState::Paused => ObservedNativeState::Paused,
        qn_nono::session::NativeSessionState::Exited => ObservedNativeState::Exited,
    }))
}

fn session_environment(
    mut command: qn_infra::process::CmdSpec,
    root: &camino::Utf8Path,
) -> qn_infra::process::CmdSpec {
    for (key, dir) in [
        ("HOME", "home"),
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_STATE_HOME", "state"),
        ("XDG_CACHE_HOME", "cache"),
        ("TMPDIR", "tmp"),
    ] {
        command = command.env(key, root.join(dir).as_str());
    }
    command
}

fn create_private_dir(path: &camino::Utf8Path) -> Result<(), qn_core::runtime::SessionError> {
    std::fs::create_dir_all(path).map_err(qn_core::runtime::SessionError::Storage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(qn_core::runtime::SessionError::Storage)?;
    }
    Ok(())
}

fn tool_dirs() -> Result<Vec<camino::Utf8PathBuf>, qn_core::runtime::SessionError> {
    #[cfg(unix)]
    {
        let dirs: Vec<_> = ["/usr/bin", "/bin", "/usr/local/bin", "/opt/homebrew/bin"]
            .into_iter()
            .map(camino::Utf8PathBuf::from)
            .filter(|path| path.is_dir())
            .collect();
        if dirs.is_empty() {
            return Err(qn_core::runtime::SessionError::Unsupported);
        }
        Ok(dirs)
    }
    #[cfg(not(unix))]
    {
        Err(qn_core::runtime::SessionError::Unsupported)
    }
}

fn write_record(
    paths: &qn_config::paths::QnPaths,
    record: &SessionRecord,
) -> Result<(), qn_core::runtime::SessionError> {
    use std::io::Write as _;
    let mut file = tempfile::NamedTempFile::new_in(&paths.runtime)
        .map_err(qn_core::runtime::SessionError::Storage)?;
    file.write_all(
        serde_json::to_vec(record)
            .map_err(|_| qn_core::runtime::SessionError::InvalidRequest)?
            .as_slice(),
    )
    .map_err(qn_core::runtime::SessionError::Storage)?;
    file.as_file()
        .sync_all()
        .map_err(qn_core::runtime::SessionError::Storage)?;
    file.persist(paths.runtime.join(format!("{}.json", record.session_id)))
        .map_err(|error| qn_core::runtime::SessionError::Storage(error.error))?;
    Ok(())
}

fn read_record(
    paths: &qn_config::paths::QnPaths,
    id: qn_core::SessionId,
) -> Result<SessionRecord, qn_core::runtime::SessionError> {
    use qn_core::runtime::SessionError;
    let path = paths.runtime.join(format!("{id}.json"));
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            SessionError::UnknownSession
        } else {
            SessionError::Storage(error)
        }
    })?;
    if !metadata.file_type().is_file() || metadata.len() > 64 * 1024 {
        return Err(SessionError::Storage(std::io::Error::other(
            "invalid session record",
        )));
    }
    let bytes = std::fs::read(&path).map_err(SessionError::Storage)?;
    let record: SessionRecord =
        serde_json::from_slice(&bytes).map_err(|_| SessionError::InvalidRequest)?;
    if record.schema_version != 1 || record.session_id != id.to_string() {
        return Err(SessionError::InvalidRequest);
    }
    Ok(record)
}
