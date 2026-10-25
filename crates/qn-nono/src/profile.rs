//! Closed offline profile preparation for pinned nono 0.80.0 (#75).
//! Linux denies cannot override ancestor grants; separation is checked first.
use crate::{NonoArgs, NonoError, RunArgs, RunMode, args::validate_path, launch::LaunchBinding};
use camino::{Utf8Path, Utf8PathBuf};
use qn_core::RunSpec;
use qn_infra::process::CmdSpec;
use serde_json::json;
use std::{fmt, fs};
const PROTECTED_HOME_PATHS: &[&str] = &[
    ".ssh",
    ".aws",
    ".gnupg",
    ".netrc",
    ".kube",
    ".docker",
    ".config",
    ".local/share/keyrings",
    "Library/Keychains",
    ".claude",
    ".codex",
    ".nono",
];
/// Trusted platform/runtime inputs supplied by the composition root.
#[derive(Clone)]
pub struct RuntimePaths {
    /// Private control directory outside writable projects; validated by the composition root.
    pub control_root: Utf8PathBuf,
    /// Every relevant host home in the execution namespace (including mounted homes).
    pub host_homes: Vec<Utf8PathBuf>,
    /// Checksum-verified engine executable, protected against project writes.
    pub engine_binary: Utf8PathBuf,
    /// Explicit absolute directories forming the child tool PATH.
    pub tool_dirs: Vec<Utf8PathBuf>,
}
impl fmt::Debug for RuntimePaths {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RuntimePaths(..)")
    }
}
/// Owns a private runtime layout and a network-blocked profile.
/// Keep it alive while running; call `persist` before retaining historical audit state.
pub struct PreparedProfile {
    root: Utf8PathBuf,
    temporary: Option<tempfile::TempDir>,
    spec: RunSpec,
    workdir: Utf8PathBuf,
    binary: Utf8PathBuf,
    tool_path: String,
}
impl fmt::Debug for PreparedProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedProfile")
            .field("id", &self.spec.id())
            .finish_non_exhaustive()
    }
}
impl PreparedProfile {
    /// Prepare via `spawn_blocking`; never copy user config, hooks, credentials or bypass flags.
    /// # Errors
    /// Rejects unsupported profiles/grants, invalid paths, physical overlap and I/O failure.
    pub fn prepare(spec: RunSpec, paths: &RuntimePaths) -> Result<Self, NonoError> {
        if spec.grant().is_some() {
            return Err(NonoError::GrantBindingRequired);
        }
        let (name, app_dir) = match spec.profile().as_str() {
            "qn/claude-code" => ("qn-claude-code", ".claude"),
            "qn/codex" => ("qn-codex", ".codex"),
            _ => return Err(NonoError::UnsupportedProfile),
        };
        let Layout {
            workdir,
            control,
            binary,
            tool_path,
        } = layout(&spec, paths)?;
        let temporary = tempfile::Builder::new()
            .prefix(&format!("{}-", spec.id()))
            .permissions(fs::metadata(&control).map_err(io_error)?.permissions())
            .tempdir_in(&control)
            .map_err(io_error)?;
        let root = utf8(temporary.path().to_owned())?;
        write_layout(&root, &workdir, name, app_dir)?;
        Ok(Self {
            root,
            temporary: Some(temporary),
            spec,
            workdir,
            binary,
            tool_path,
        })
    }
    /// Private host-side root, suitable for the scoped audit reader.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }
    /// Path to the generated policy; not an agent-writable configuration.
    #[must_use]
    pub fn profile_path(&self) -> Utf8PathBuf {
        self.root.join("profile.json")
    }
    /// Retain the layout for audit/history; the registry then owns explicit cleanup.
    #[must_use]
    pub fn persist(mut self) -> Self {
        if let Some(directory) = self.temporary.take() {
            // tempfile 3.27 keep() disables Drop cleanup and returns PathBuf, not Result.
            let _ = directory.keep();
        }
        self
    }
    /// Build isolated strict native schema validation; does not execute it.
    /// # Errors
    /// Returns argument-construction errors without echoing paths.
    pub fn validation_command(&self) -> Result<CmdSpec, NonoError> {
        Ok(self.command(&NonoArgs::profile_validate(&self.profile_path())?))
    }
    /// Build foreground invocation without supervisor/credentials; execution stays in infra.
    /// # Errors
    /// Returns argument-construction errors.
    pub fn run_command(&self) -> Result<CmdSpec, NonoError> {
        let mut args = RunArgs::from_spec(&self.spec, self.profile_path(), RunMode::Foreground);
        args.workdir = self.workdir.clone();
        Ok(self.command(&NonoArgs::run(args)?))
    }
    fn command(&self, args: &NonoArgs) -> CmdSpec {
        apply_environment(
            CmdSpec::new(self.binary.as_str())
                .args(args.as_slice().iter().cloned())
                .env_clear(),
            &self.root,
            &self.workdir,
            &self.tool_path,
        )
    }
}

/// One persistent runtime layout for an official online profile.
///
/// The caller owns the root and keeps it alive for the session/audit lifetime.
/// Preparation validates the same physical separation as the offline profile.
pub struct PreparedOfficial {
    root: Utf8PathBuf,
    spec: RunSpec,
    workdir: Utf8PathBuf,
    binary: Utf8PathBuf,
    tool_path: String,
    state_dir: String,
}
impl fmt::Debug for PreparedOfficial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedOfficial")
            .field("id", &self.spec.id())
            .finish_non_exhaustive()
    }
}
impl PreparedOfficial {
    /// Write a validated official asset into a caller-owned private runtime root.
    ///
    /// The root must be absolute and outside the project; it is created if needed.
    /// The asset is expected to have passed the pinned engine validator already.
    /// # Errors
    /// Rejects invalid paths, physical overlap, an empty asset and I/O failure.
    pub fn prepare(
        spec: RunSpec,
        paths: &RuntimePaths,
        root: Utf8PathBuf,
        asset: &str,
        state_dir: &str,
    ) -> Result<Self, NonoError> {
        if !root.is_absolute()
            || root.as_str().contains('\0')
            || asset.is_empty()
            || state_dir.is_empty()
        {
            return Err(NonoError::InvalidArgument {
                field: "official profile layout",
            });
        }
        let Layout {
            workdir,
            binary,
            tool_path,
            ..
        } = layout(&spec, paths)?;
        protect(&workdir, &root)?;
        fs::create_dir_all(&root).map_err(io_error)?;
        write_official_layout(&root, state_dir, asset)?;
        Ok(Self {
            root,
            spec,
            workdir,
            binary,
            tool_path,
            state_dir: state_dir.to_owned(),
        })
    }
    /// Private host-side session root.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }
    /// Directory name for the application state under the isolated home.
    #[must_use]
    pub fn state_dir(&self) -> &str {
        &self.state_dir
    }
    /// Path to the official policy asset.
    #[must_use]
    pub fn profile_path(&self) -> Utf8PathBuf {
        self.root.join("profile.json")
    }
    /// Build a foreground command with the predetermined native identity.
    ///
    /// The child inherits the terminal so an interactive Agent can render. The
    /// grant is not encoded in argv; the broker checks it per request.
    /// # Errors
    /// Rejects identity drift and invalid native argument construction.
    pub fn bound_run_command(&self, binding: &LaunchBinding) -> Result<CmdSpec, NonoError> {
        self.bound_run_command_with_stdio(binding, qn_infra::process::StdioMode::Inherited)
    }

    /// Build the same bound command with an explicit stdio mode.
    ///
    /// Tests use `Captured` to assert bounded output; production callers use
    /// `Inherited` so an interactive Agent owns the terminal.
    /// # Errors
    /// Preserves every identity and argument check of `bound_run_command`.
    pub fn bound_run_command_with_stdio(
        &self,
        binding: &LaunchBinding,
        stdio: qn_infra::process::StdioMode,
    ) -> Result<CmdSpec, NonoError> {
        let mut args = RunArgs::from_spec(&self.spec, self.profile_path(), RunMode::Foreground);
        args.workdir = self.workdir.clone();
        let command = binding.command(&self.binary, args)?;
        let mut command = apply_environment(command, &self.root, &self.workdir, &self.tool_path)
            .timeout(std::time::Duration::from_secs(24 * 60 * 60));
        if stdio == qn_infra::process::StdioMode::Inherited {
            command = command.inherit_stdio();
        }
        Ok(command)
    }
}

struct Layout {
    workdir: Utf8PathBuf,
    control: Utf8PathBuf,
    binary: Utf8PathBuf,
    tool_path: String,
}

fn layout(spec: &RunSpec, paths: &RuntimePaths) -> Result<Layout, NonoError> {
    let workdir = directory(spec.workdir())?;
    let control = directory(&paths.control_root)?;
    let binary = canonical(&paths.engine_binary)?;
    if !fs::metadata(&binary).map_err(io_error)?.is_file() {
        return Err(NonoError::InvalidArgument {
            field: "engine executable",
        });
    }
    protect(&workdir, &control)?;
    protect(&workdir, &binary)?;
    protect(
        &workdir,
        &canonical(&utf8(std::env::current_exe().map_err(io_error)?)?)?,
    )?;
    if paths.host_homes.is_empty() {
        return Err(NonoError::InvalidArgument {
            field: "host homes",
        });
    }
    for home in &paths.host_homes {
        let home = directory(home)?;
        if ancestor(&workdir, &home)? {
            return Err(NonoError::UnsafeWorkdir);
        }
        for name in PROTECTED_HOME_PATHS {
            protect(&workdir, &home.join(name))?;
        }
    }
    if paths.tool_dirs.is_empty() {
        return Err(NonoError::InvalidArgument {
            field: "tool search path",
        });
    }
    for path in &paths.tool_dirs {
        validate_path(path, "tool search directory")?;
    }
    let tool_path = std::env::join_paths(&paths.tool_dirs)
        .map_err(|_| NonoError::InvalidArgument {
            field: "tool search path",
        })?
        .into_string()
        .map_err(|_| NonoError::InvalidArgument {
            field: "tool search path",
        })?;
    Ok(Layout {
        workdir,
        control,
        binary,
        tool_path,
    })
}

fn apply_environment(
    mut command: CmdSpec,
    root: &Utf8Path,
    workdir: &Utf8Path,
    tool_path: &str,
) -> CmdSpec {
    command = command
        .env("PATH", tool_path)
        .env("LANG", "C")
        .cwd(workdir.to_owned());
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

fn write_official_layout(root: &Utf8Path, state_dir: &str, asset: &str) -> Result<(), NonoError> {
    for dir in ["home", "config", "state", "cache", "tmp"] {
        fs::create_dir_all(root.join(dir)).map_err(io_error)?;
    }
    for dir in [state_dir, ".config", ".cache", ".local/state", "tmp"] {
        fs::create_dir_all(root.join("home").join(dir)).map_err(io_error)?;
    }
    fs::write(root.join("profile.json"), asset).map_err(io_error)
}
fn write_layout(
    root: &Utf8Path,
    workdir: &Utf8Path,
    name: &str,
    app_dir: &str,
) -> Result<(), NonoError> {
    let writable = [app_dir, ".config", ".cache", ".local/state", "tmp"];
    for dir in ["home", "config", "state", "cache", "tmp"] {
        fs::create_dir_all(root.join(dir)).map_err(io_error)?;
    }
    let mut allow = vec![workdir.as_str().to_owned()];
    for dir in writable {
        let path = root.join("home").join(dir);
        fs::create_dir_all(&path).map_err(io_error)?;
        allow.push(path.into_string());
    }
    // nono 0.80.0 execution_runtime.rs: WSL2 refuses unenforceable proxy-only mode.
    let policy = json!({"meta":{"name":name,"version":"0.1.0"}, "extends":"default",
        "filesystem":{"allow":allow}, "network":{"block":true},
        "security":{"capability_elevation":false,"signal_mode":"isolated","process_info_mode":"isolated","wsl2_proxy_policy":"error"},
        "environment":{"allow_vars":["PATH","HOME","LANG"], "set_vars":{
            "TMPDIR":"$HOME/tmp", "XDG_CONFIG_HOME":"$HOME/.config", "XDG_STATE_HOME":"$HOME/.local/state", "XDG_CACHE_HOME":"$HOME/.cache"}}});
    fs::write(root.join("profile.json"), policy.to_string()).map_err(io_error)
}
fn utf8(path: std::path::PathBuf) -> Result<Utf8PathBuf, NonoError> {
    Utf8PathBuf::from_path_buf(path).map_err(|_| NonoError::InvalidArgument {
        field: "UTF-8 runtime path",
    })
}
fn canonical(path: &Utf8Path) -> Result<Utf8PathBuf, NonoError> {
    validate_path(path, "runtime path")?;
    utf8(fs::canonicalize(path).map_err(io_error)?)
}
fn directory(path: &Utf8Path) -> Result<Utf8PathBuf, NonoError> {
    let path = canonical(path)?;
    if !fs::metadata(&path).map_err(io_error)?.is_dir() {
        return Err(NonoError::InvalidArgument {
            field: "runtime directory",
        });
    }
    Ok(path)
}
fn ancestor(parent: &Utf8Path, child: &Utf8Path) -> Result<bool, NonoError> {
    for candidate in child.ancestors() {
        if candidate.try_exists().map_err(io_error)? {
            // Resolve redirected credential directories before comparing their ancestors.
            for resolved in canonical(candidate)?.ancestors() {
                if same_file::is_same_file(parent, resolved).map_err(io_error)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
    }
    Ok(false)
}
fn protect(workdir: &Utf8Path, protected: &Utf8Path) -> Result<(), NonoError> {
    if ancestor(workdir, protected)?
        || (protected.try_exists().map_err(io_error)? && ancestor(protected, workdir)?)
    {
        return Err(NonoError::UnsafeWorkdir);
    }
    Ok(())
}
fn io_error(source: std::io::Error) -> NonoError {
    NonoError::Io {
        step: "preparing managed profile",
        source,
    }
}
#[cfg(test)]
#[path = "profile_tests.rs"]
mod tests;
