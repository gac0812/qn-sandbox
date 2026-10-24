//! Closed offline profile preparation for pinned nono 0.80.0 (#75).
//! Linux denies cannot override ancestor grants; separation is checked first.
use crate::{NonoArgs, NonoError, RunArgs, RunMode, args::validate_path};
use camino::{Utf8Path, Utf8PathBuf};
use qn_core::RunSpec;
use qn_infra::process::CmdSpec;
use serde_json::json;
use std::{fmt, fs};

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
    /// Prepare synchronously; async callers must use `spawn_blocking`.
    /// No user config, hooks, credential definitions or bypass flags are copied.
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
            for name in [
                ".ssh",
                ".aws",
                ".gnupg",
                ".config",
                ".local/share/keyrings",
                "Library/Keychains",
                ".claude",
                ".codex",
                ".nono",
            ] {
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
            let _ = directory.keep();
        }
        self
    }
    /// Build strict native schema validation in the isolated environment.
    /// # Errors
    /// Returns argument-construction errors without echoing paths.
    pub fn validation_command(&self) -> Result<CmdSpec, NonoError> {
        Ok(self.command(&NonoArgs::profile_validate(&self.profile_path())?))
    }
    /// Build a foreground invocation; no detached supervisor or credential binding.
    /// # Errors
    /// Returns argument-construction errors. Execution remains in the infra layer.
    pub fn run_command(&self) -> Result<CmdSpec, NonoError> {
        let mut args = RunArgs::from_spec(&self.spec, self.profile_path(), RunMode::Foreground);
        args.workdir = self.workdir.clone();
        Ok(self.command(&NonoArgs::run(args)?))
    }
    fn command(&self, args: &NonoArgs) -> CmdSpec {
        let mut command = CmdSpec::new(self.binary.as_str())
            .args(args.as_slice().iter().cloned())
            .env_clear()
            .env("PATH", &self.tool_path)
            .env("LANG", "C")
            .cwd(self.workdir.clone());
        for (key, dir) in [
            ("HOME", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_STATE_HOME", "state"),
            ("XDG_CACHE_HOME", "cache"),
            ("TMPDIR", "tmp"),
        ] {
            command = command.env(key, self.root.join(dir).as_str());
        }
        command
    }
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
