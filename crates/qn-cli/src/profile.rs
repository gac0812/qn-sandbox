//! Versioned official-profile catalog and registry presentation.
//!
//! This module renders bounded core summaries. It performs no engine launch,
//! network access or credential resolution.

use crate::Error;
use clap::Subcommand;
use qn_core::ProfileRef;
use qn_core::profile::{ProfileRegistry, ProfileSource, ProfileSummary, ResolvedProfile};
use std::{io::Write, time::Duration};

/// Profile catalog and host-registry operations.
#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// List compiled and installed profile versions.
    List {
        /// Emit a versioned machine-readable catalog.
        #[arg(long)]
        json: bool,
    },
    /// Show the resolved nono profile asset for one identity.
    Show {
        /// Canonical `namespace/name` profile identity.
        id: String,
        /// Emit a versioned machine-readable profile document.
        #[arg(long)]
        json: bool,
    },
    /// Copy the compiled version into the host registry and select it.
    Install {
        /// Canonical `namespace/name` profile identity.
        id: String,
    },
    /// Select the previous verified version as current.
    Rollback {
        /// Canonical `namespace/name` profile identity.
        id: String,
    },
    /// Validate the resolved asset with the pinned engine's strict validator.
    Validate {
        /// Canonical `namespace/name` profile identity.
        id: String,
        /// Emit a versioned machine-readable validation result.
        #[arg(long)]
        json: bool,
    },
    /// Validate a local asset with the pinned engine and publish it as a new version.
    Publish {
        /// Compiled official profile identity to update.
        #[arg(long)]
        id: String,
        /// Strict MAJOR.MINOR.PATCH version matching the asset meta.
        #[arg(long)]
        version: String,
        /// Absolute path to a nono profile JSON file.
        asset: String,
    },
}

pub(crate) fn run(command: &ProfileCommand, output: &mut dyn Write) -> Result<(), Error> {
    let paths = qn_config::paths::QnPaths::resolve()?;
    let control = qn_daemon::ControlPlane::new(paths.clone()).map_err(daemon_error)?;
    match command {
        ProfileCommand::List { json } => list(control.profiles(), *json, output)?,
        ProfileCommand::Show { id, json } => {
            let id = ProfileRef::try_from(id.as_str())?;
            show(&control.resolve_profile(id)?, *json, output)?;
        }
        ProfileCommand::Install { id } => {
            let id = ProfileRef::try_from(id.as_str())?;
            let summary = control.install_profile(id)?;
            writeln!(output, "installed {}", escaped_summary(&summary))?;
        }
        ProfileCommand::Rollback { id } => {
            let id = ProfileRef::try_from(id.as_str())?;
            let summary = control.rollback_profile(id)?;
            writeln!(output, "rolled back to {}", escaped_summary(&summary))?;
        }
        ProfileCommand::Publish { id, version, asset } => {
            let id = ProfileRef::try_from(id.as_str())?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(Error::Runtime)?;
            let result = runtime.block_on(control.publish_profile(id, version, asset));
            runtime.shutdown_timeout(Duration::from_millis(100));
            let summary = result?;
            writeln!(output, "published {}", escaped_summary(&summary))?;
        }
        ProfileCommand::Validate { id, json } => {
            let id = ProfileRef::try_from(id.as_str())?;
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(Error::Runtime)?;
            let result = runtime.block_on(control.validate_profile(id));
            runtime.shutdown_timeout(Duration::from_millis(100));
            let summary = result?;
            if *json {
                let encoded = serde_json::to_string(&serde_json::json!({
                    "schema_version": 1,
                    "validated": {
                        "id": summary.id().as_str(),
                        "version": summary.version().as_str(),
                        "source": source_label(summary.source()),
                    }
                }))?;
                writeln!(output, "{encoded}")?;
            } else {
                writeln!(
                    output,
                    "validated {} by the pinned engine",
                    escaped_summary(&summary)
                )?;
            }
        }
    }
    output.flush()?;
    Ok(())
}

fn list(registry: &dyn ProfileRegistry, json: bool, output: &mut dyn Write) -> Result<(), Error> {
    let summaries = registry.list()?;
    if json {
        let profiles: Vec<_> = summaries
            .iter()
            .map(|summary| {
                serde_json::json!({
                    "id": summary.id().as_str(),
                    "version": summary.version().as_str(),
                    "source": source_label(summary.source()),
                })
            })
            .collect();
        let encoded = serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "profiles": profiles,
        }))?;
        writeln!(output, "{encoded}")?;
        return Ok(());
    }
    writeln!(output, "profiles:")?;
    for summary in summaries {
        writeln!(output, "  {}", escaped_summary(&summary))?;
    }
    Ok(())
}

fn show(profile: &ResolvedProfile, json: bool, output: &mut dyn Write) -> Result<(), Error> {
    let summary = profile.summary();
    if json {
        let encoded = serde_json::to_string(&serde_json::json!({
            "schema_version": 1,
            "profile": {
                "id": summary.id().as_str(),
                "version": summary.version().as_str(),
                "source": source_label(summary.source()),
                "state_dir": profile.state_dir(),
                "network_hosts": profile.network_hosts(),
                "nono_profile": profile.nono_profile(),
            }
        }))?;
        writeln!(output, "{encoded}")?;
        return Ok(());
    }
    writeln!(output, "{}", escaped_summary(summary))?;
    writeln!(
        output,
        "state: {}",
        crate::escape_terminal_text(profile.state_dir())
    )?;
    let hosts: Vec<_> = profile
        .network_hosts()
        .iter()
        .map(|host| crate::escape_terminal_text(host))
        .collect();
    writeln!(output, "network allowlist: {}", hosts.join(", "))?;
    writeln!(output, "nono profile:")?;
    writeln!(output, "{}", profile.nono_profile())?;
    Ok(())
}

fn daemon_error(error: qn_daemon::Error) -> Error {
    match error {
        qn_daemon::Error::Setup(error) => Error::Setup(error),
        qn_daemon::Error::Profile(error) => Error::Profile(error),
        qn_daemon::Error::Audit(error) => Error::Audit(error),
        qn_daemon::Error::NativeAudit(error) => Error::NativeAudit(error),
        _ => Error::Runtime(std::io::Error::other("control-plane composition failed")),
    }
}

fn source_label(source: ProfileSource) -> &'static str {
    match source {
        ProfileSource::Builtin => "builtin",
        ProfileSource::Installed => "installed",
    }
}

fn escaped_summary(summary: &ProfileSummary) -> String {
    format!(
        "{}@{} ({})",
        crate::escape_terminal_text(summary.id().as_str()),
        crate::escape_terminal_text(summary.version().as_str()),
        source_label(summary.source())
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::{list, show};
    use qn_core::ProfileRef;
    use qn_core::profile::{
        ProfileError, ProfileRegistry, ProfileSource, ProfileSummary, ProfileVersion,
        ResolvedProfile,
    };

    struct Fixed(Vec<ProfileSummary>);

    impl ProfileRegistry for Fixed {
        fn list(&self) -> Result<Vec<ProfileSummary>, ProfileError> {
            Ok(self.0.clone())
        }
        fn resolve(&self, _: ProfileRef) -> Result<ResolvedProfile, ProfileError> {
            Err(ProfileError::NotInstalled)
        }
        fn install(&self, _: ProfileRef) -> Result<ProfileSummary, ProfileError> {
            Err(ProfileError::NotInstalled)
        }
        fn rollback(&self, _: ProfileRef) -> Result<ProfileSummary, ProfileError> {
            Err(ProfileError::NotInstalled)
        }
    }

    fn summary() -> ProfileSummary {
        ProfileSummary::new(
            ProfileRef::try_from("qn/claude-code").unwrap(),
            ProfileVersion::try_from("1.0.0").unwrap(),
            ProfileSource::Builtin,
        )
    }

    #[test]
    fn it_renders_a_versioned_catalog_without_paths() {
        let registry = Fixed(vec![summary()]);
        let mut text = Vec::new();
        list(&registry, false, &mut text).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("qn/claude-code@1.0.0 (builtin)"));
        let mut json = Vec::new();
        list(&registry, true, &mut json).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["profiles"][0]["id"], "qn/claude-code");
        assert_eq!(value["profiles"][0]["source"], "builtin");
    }

    #[test]
    fn it_renders_the_resolved_asset_and_allowlist() {
        let resolved = ResolvedProfile::new(
            summary(),
            "{\"meta\":{\"name\":\"qn-claude-code\"}}".into(),
            vec!["api.anthropic.com".into()],
            ".claude".into(),
        )
        .unwrap();
        let mut text = Vec::new();
        show(&resolved, false, &mut text).unwrap();
        let text = String::from_utf8(text).unwrap();
        assert!(text.contains("state: .claude"));
        assert!(text.contains("network allowlist: api.anthropic.com"));
        assert!(text.contains("nono profile:"));
        let mut json = Vec::new();
        show(&resolved, true, &mut json).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["profile"]["state_dir"], ".claude");
        assert_eq!(value["profile"]["network_hosts"][0], "api.anthropic.com");
    }
}
