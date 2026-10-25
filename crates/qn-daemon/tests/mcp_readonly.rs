//! Read-only MCP consumer contracts over the shared control plane.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use qn_daemon::ControlPlane;
use qn_mcp::{CATALOG_SCHEMA_VERSION, ResultErrorCode, ToolName, ToolOutcome, ToolRequest};
use std::path::Path;

fn plane(root: &Path) -> ControlPlane {
    ControlPlane::new(qn_config::paths::QnPaths::from_root(
        camino::Utf8PathBuf::from_path_buf(root.to_path_buf()).unwrap(),
    ))
    .unwrap()
}

fn request(tool: ToolName, arguments: serde_json::Value) -> ToolRequest {
    ToolRequest {
        schema_version: CATALOG_SCHEMA_VERSION,
        request_id: format!("readonly-{}", tool.as_str()),
        tool,
        arguments,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn profile_list_is_read_only_and_correlated() {
    let root = tempfile::tempdir().unwrap();
    let request = request(ToolName::ProfileList, serde_json::json!({}));
    let result = plane(root.path())
        .dispatch_mcp_request(request.clone())
        .await;
    assert_eq!(result.request_id, request.request_id);
    match result.outcome {
        ToolOutcome::Ok { value } => {
            assert_eq!(value["profiles"].as_array().unwrap().len(), 2);
            assert!(!value.to_string().contains("secret"));
        }
        other @ ToolOutcome::Error { .. } => panic!("unexpected outcome: {other:?}"),
    }
    assert!(!root.path().join("runtime").exists());
}

#[tokio::test(flavor = "current_thread")]
async fn profile_show_and_status_validate_arguments_without_raw_echo() {
    let root = tempfile::tempdir().unwrap();
    let profile = plane(root.path())
        .dispatch_mcp_request(request(
            ToolName::ProfileShow,
            serde_json::json!({"id": "qn/claude-code"}),
        ))
        .await;
    assert!(
        matches!(profile.outcome, ToolOutcome::Ok { .. }),
        "{profile:?}"
    );

    let status = plane(root.path())
        .dispatch_mcp_request(request(
            ToolName::SessionStatus,
            serde_json::json!({"session": "bad\nsecret"}),
        ))
        .await;
    match status.outcome {
        ToolOutcome::Error { code, message } => {
            assert_eq!(code, ResultErrorCode::InvalidArguments);
            assert!(!message.contains("secret"));
        }
        other @ ToolOutcome::Ok { .. } => panic!("unexpected outcome: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn audit_query_missing_source_is_a_safe_failure() {
    let root = tempfile::tempdir().unwrap();
    let id = qn_core::SessionId::new().to_string();
    let result = plane(root.path())
        .dispatch_mcp_request(request(
            ToolName::AuditQuery,
            serde_json::json!({"session": id}),
        ))
        .await;
    match result.outcome {
        ToolOutcome::Error { code, message } => {
            assert_eq!(code, ResultErrorCode::Failed);
            assert!(message.contains("audit journal"));
            assert!(!message.contains(root.path().to_str().unwrap()));
        }
        other @ ToolOutcome::Ok { .. } => panic!("unexpected outcome: {other:?}"),
    }
}
