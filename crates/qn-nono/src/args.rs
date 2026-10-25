//! Constrained argv construction, checked against nono 0.80.0 CLI help (#43).
//!
//! This module does not execute commands or validate profile contents. The future
//! runner must verify profile policy and scrub inherited environment before spawn.

use std::fmt;

use camino::{Utf8Path, Utf8PathBuf};
use qn_core::{RunSpec, SessionId};

use crate::NonoError;

mod session;
pub use session::{NonoSessionId, NonoStopMode};

/// Whether nono attaches the terminal or leaves its supervisor running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunMode {
    /// Keep the foreground CLI attached.
    Foreground,
    /// Request nono's detached supervisor mode.
    Detached,
}

/// Inputs for an invocation whose permissions come solely from a managed profile.
///
/// This contains no generic nono flags. Command arguments belong to the child,
/// after the option terminator; callers must not supply credentials in them.
#[derive(Clone)]
pub struct RunArgs {
    /// Absolute path to a qn-generated profile, validated separately before spawn.
    pub profile: Utf8PathBuf,
    /// Existing core identity, used as the nono session name.
    pub name: SessionId,
    /// Absolute project directory used for nono's workdir expansion.
    pub workdir: Utf8PathBuf,
    /// Explicit foreground or detached mode.
    pub mode: RunMode,
    /// Executable followed by its arguments; values are never shell-interpreted.
    pub command: Vec<String>,
}

impl fmt::Debug for RunArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunArgs")
            .field("name", &self.name)
            .field("mode", &self.mode)
            .field("argument_count", &self.command.len())
            .finish_non_exhaustive()
    }
}

impl RunArgs {
    /// Translate validated core launch data after resolving/authorizing a managed profile.
    /// Grant handles are deliberately not encoded in argv; the runtime binds them separately.
    #[must_use]
    pub fn from_spec(spec: &RunSpec, profile: Utf8PathBuf, mode: RunMode) -> Self {
        Self {
            profile,
            name: spec.id(),
            workdir: spec.workdir().to_owned(),
            mode,
            command: spec.command().to_vec(),
        }
    }
}

/// Immutable nono arguments built from a limited set of supported operations.
///
/// No raw flag append API is provided. Debug omits paths and command values.
///
/// See the run constructor for a complete example.
#[derive(Clone, PartialEq, Eq)]
pub struct NonoArgs(Vec<String>);

impl NonoArgs {
    /// Build the version probe, without permissions or credential arguments.
    #[must_use]
    pub fn version() -> Self {
        Self(vec!["--version".to_owned()])
    }

    /// Build strict JSON validation for a managed profile file.
    ///
    /// # Errors
    /// Returns an error for a relative path or a path containing NUL.
    pub fn profile_validate(profile: &Utf8Path) -> Result<Self, NonoError> {
        validate_path(profile, "profile path")?;
        Ok(Self(vec![
            "profile".into(),
            "validate".into(),
            "--json".into(),
            "--strict".into(),
            "--".into(),
            profile.to_string(),
        ]))
    }

    /// Build a run invocation with structured diagnostics and a child delimiter.
    ///
    /// Permissions and credentials must be validated in the profile, not argv.
    /// This function only constructs arguments; it does not start a sandbox.
    ///
    /// # Errors
    /// Rejects relative/NUL paths, an empty program, and NUL in any child argument.
    ///
    /// ```
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use qn_nono::{NonoArgs, RunArgs, RunMode};
    /// let root = camino::Utf8PathBuf::from_path_buf(std::env::current_dir()?)
    ///     .map_err(|_| "working directory must be UTF-8")?;
    /// let args = NonoArgs::run(RunArgs {
    ///     profile: root.join("profile.json"), name: qn_core::SessionId::new(),
    ///     workdir: root, mode: RunMode::Foreground,
    ///     command: vec!["agent".into(), "--version".into()],
    /// })?;
    /// assert!(args.as_slice().iter().any(|arg| arg == "--"));
    /// # Ok(())
    /// # }
    /// # example().unwrap();
    /// ```
    pub fn run(input: RunArgs) -> Result<Self, NonoError> {
        Self::run_at_port(input, None)
    }

    /// Use an approved nonzero native proxy port matching managed Agent configuration.
    /// nono 0.80.0 exposes --proxy-port; binding failure must refuse startup (#137).
    /// This sets the native listener's bind port and grants no additional network access.
    ///
    /// ```no_run
    /// # fn example(input: qn_nono::RunArgs, port: std::num::NonZeroU16) -> Result<(), qn_nono::NonoError> {
    /// let args = qn_nono::NonoArgs::run_with_proxy_port(input, port)?;
    /// assert!(args.as_slice().iter().any(|arg| arg == "--proxy-port"));
    /// # Ok(()) }
    /// ```
    /// # Errors
    /// Applies all validation from the ordinary run constructor.
    pub fn run_with_proxy_port(
        input: RunArgs,
        port: std::num::NonZeroU16,
    ) -> Result<Self, NonoError> {
        Self::run_at_port(input, Some(port))
    }

    fn run_at_port(input: RunArgs, port: Option<std::num::NonZeroU16>) -> Result<Self, NonoError> {
        validate_path(&input.profile, "profile path")?;
        validate_path(&input.workdir, "workdir")?;
        if input.command.first().is_none_or(String::is_empty)
            || input.command.iter().any(|arg| arg.contains('\0'))
        {
            return Err(NonoError::InvalidArgument { field: "command" });
        }
        let mut args = vec![
            "run".into(),
            "--profile".into(),
            input.profile.into_string(),
            "--name".into(),
            input.name.to_string(),
            "--workdir".into(),
            input.workdir.into_string(),
            "--diagnostics-json".into(),
        ];
        if input.mode == RunMode::Detached {
            args.push("--detached".into());
        }
        if let Some(port) = port {
            args.extend(["--proxy-port".into(), port.to_string()]);
        }
        args.push("--".into());
        args.extend(input.command);
        Ok(Self(args))
    }

    /// Borrow literal UTF-8 arguments for the existing qn-infra process API.
    #[must_use]
    pub fn as_slice(&self) -> &[String] {
        &self.0
    }
}

impl fmt::Debug for NonoArgs {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NonoArgs")
            .field("argument_count", &self.0.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn validate_path(path: &Utf8Path, field: &'static str) -> Result<(), NonoError> {
    if !path.is_absolute() || path.as_str().contains('\0') {
        return Err(NonoError::InvalidArgument { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::{NonoArgs, RunArgs, RunMode};
    use crate::NonoError;
    use camino::{Utf8Path, Utf8PathBuf};
    use qn_core::SessionId;

    fn input() -> RunArgs {
        let root = Utf8PathBuf::from_path_buf(std::env::temp_dir()).unwrap();
        RunArgs {
            profile: root.join("qn profile.json"),
            name: SessionId::new(),
            workdir: root.join("project with spaces"),
            mode: RunMode::Foreground,
            command: vec!["agent".into(), "--version".into()],
        }
    }

    #[test]
    fn it_builds_only_the_expected_version_and_validation_options() {
        assert_eq!(NonoArgs::version().as_slice(), ["--version"]);
        let profile = input().profile;
        assert_eq!(
            NonoArgs::profile_validate(&profile).unwrap().as_slice(),
            [
                "profile",
                "validate",
                "--json",
                "--strict",
                "--",
                profile.as_str()
            ]
        );
    }

    #[test]
    fn it_constructs_run_with_exact_permissions_boundary() {
        let request = input();
        let name = request.name.to_string();
        let args = NonoArgs::run(request.clone()).unwrap();
        assert_eq!(
            args.as_slice(),
            [
                "run",
                "--profile",
                request.profile.as_str(),
                "--name",
                &name,
                "--workdir",
                request.workdir.as_str(),
                "--diagnostics-json",
                "--",
                "agent",
                "--version"
            ]
        );
    }

    #[test]
    fn it_adds_detached_only_before_the_child_command() {
        let mut request = input();
        request.mode = RunMode::Detached;
        let args = NonoArgs::run(request).unwrap();
        assert_eq!(
            &args.as_slice()[8..],
            ["--detached", "--", "agent", "--version"]
        );
    }

    #[test]
    fn it_places_only_the_approved_proxy_port_before_the_child_delimiter() {
        let mut request = input();
        request
            .command
            .extend(["--proxy-port".into(), "4321".into()]);
        let command = request.command.clone();
        let args = NonoArgs::run_with_proxy_port(request, std::num::NonZeroU16::new(1234).unwrap())
            .unwrap();
        assert_eq!(&args.as_slice()[8..11], ["--proxy-port", "1234", "--"]);
        assert_eq!(&args.as_slice()[11..], command);
        assert!(!format!("{args:?}").contains("1234"));
    }

    #[test]
    fn it_preserves_validation_with_a_fixed_proxy_port() {
        let port = std::num::NonZeroU16::new(u16::MAX).unwrap();
        assert!(std::num::NonZeroU16::new(0).is_none());
        for field in ["profile", "workdir", "empty_command", "nul_argument"] {
            let mut request = input();
            match field {
                "profile" => request.profile = "relative.json".into(),
                "workdir" => request.workdir = "relative".into(),
                "empty_command" => request.command.clear(),
                _ => request.command.push(String::from(char::from(0))),
            }
            assert!(NonoArgs::run_with_proxy_port(request, port).is_err());
        }
        let args = NonoArgs::run_with_proxy_port(input(), port).unwrap();
        assert_eq!(&args.as_slice()[8..11], ["--proxy-port", "65535", "--"]);
    }

    #[test]
    fn it_preserves_child_arguments_without_granting_nono_flags() {
        let mut request = input();
        request.command.extend(
            [
                "--env-credential",
                "--env-credential-map",
                "--allow-net",
                "--bypass-protection",
                "--trust-override",
                "--no-audit",
                "--allow",
                "--read",
                "a b",
                "quote'\"",
                "$(not-executed)",
                "",
                "--",
            ]
            .map(str::to_owned),
        );
        let command = request.command.clone();
        let args = NonoArgs::run(request).unwrap();
        let delimiter = args.as_slice().iter().position(|arg| arg == "--").unwrap();
        assert_eq!(&args.as_slice()[delimiter + 1..], command);
        for flag in [
            "--env-credential",
            "--env-credential-map",
            "--allow-net",
            "--bypass-protection",
            "--trust-override",
            "--no-audit",
            "--allow",
            "--read",
        ] {
            assert!(!args.as_slice()[..delimiter].iter().any(|arg| arg == flag));
        }
    }

    #[test]
    fn it_rejects_invalid_paths_before_building_arguments() {
        assert!(NonoArgs::profile_validate(Utf8Path::new("default")).is_err());
        for field in ["profile path", "workdir"] {
            let mut request = input();
            if field == "profile path" {
                request.profile = "relative.json".into();
            } else {
                request.workdir = "relative".into();
            }
            assert!(matches!(NonoArgs::run(request),
                Err(NonoError::InvalidArgument { field: actual }) if actual == field));
        }
        let nul_path = input().profile.into_string() + "\0";
        assert!(NonoArgs::profile_validate(Utf8Path::new(&nul_path)).is_err());
    }

    #[test]
    fn it_rejects_missing_program_and_nul_in_child_arguments() {
        for command in [
            vec![],
            vec![String::new()],
            vec!["agent\0".into()],
            vec!["agent".into(), "value\0".into()],
        ] {
            let mut request = input();
            request.command = command;
            assert!(matches!(
                NonoArgs::run(request),
                Err(NonoError::InvalidArgument { field: "command" })
            ));
        }
    }

    #[test]
    fn it_redacts_paths_and_values_from_debug() {
        let mut request = input();
        request.command.push("synthetic-sensitive-value".into());
        let args = NonoArgs::run(request.clone()).unwrap();
        let debug = format!("{request:?} {args:?}");
        for hidden in [
            request.profile.as_str(),
            request.workdir.as_str(),
            "synthetic-sensitive-value",
        ] {
            assert!(!debug.contains(hidden));
        }
    }

    #[test]
    fn it_consumes_core_launch_data_without_putting_grants_in_argv() {
        let input = input();
        let grant = qn_core::GrantHandle::new();
        let spec = qn_core::RunSpec::new(
            input.name,
            qn_core::ProfileRef::try_from("qn/codex").unwrap(),
            input.command.clone(),
            input.workdir.clone(),
        )
        .unwrap()
        .with_grant(grant);
        let args = NonoArgs::run(RunArgs::from_spec(&spec, input.profile, input.mode)).unwrap();
        assert_eq!(&args.as_slice()[9..], spec.command());
        assert!(
            !args
                .as_slice()
                .iter()
                .any(|value| value.contains(&grant.to_string()))
        );
    }
}
