//! Security contracts for the transport-neutral MCP consumer boundary.
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

fn request(arguments: serde_json::Value) -> ToolRequest {
    ToolRequest {
        schema_version: CATALOG_SCHEMA_VERSION,
        request_id: "security-contract-1".into(),
        tool: ToolName::SessionStatus,
        arguments,
    }
}

#[test]
fn invalid_arguments_return_closed_error_without_echoing_payload() {
    let root = tempfile::tempdir().unwrap();
    let secret = "synthetic-mcp-secret";
    let result = plane(root.path()).validate_mcp_request(&request(
        serde_json::json!({"config": {"password": secret}, "path": "/private/host"}),
    ));
    match result.outcome {
        ToolOutcome::Error { code, message } => {
            assert_eq!(code, ResultErrorCode::InvalidArguments);
            assert!(!message.contains(secret));
            assert!(!message.contains("/private/host"));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn unknown_tool_is_rejected_without_argument_echo() {
    let root = tempfile::tempdir().unwrap();
    let mut request = request(serde_json::json!({}));
    request.tool = ToolName::McpRoutePlan;
    let result = plane(root.path()).validate_mcp_request(&request);
    match result.outcome {
        ToolOutcome::Error { code, message } => {
            assert_eq!(code, ResultErrorCode::Failed);
            assert!(!message.contains("secret"));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn read_only_dispatch_returns_denial_for_mutating_tools() {
    let root = tempfile::tempdir().unwrap();
    let mut request = request(serde_json::json!({}));
    request.tool = ToolName::SessionRun;
    let result = plane(root.path()).dispatch_mcp_request(request).await;
    match result.outcome {
        ToolOutcome::Error { code, message } => {
            assert_eq!(code, ResultErrorCode::Denied);
            assert!(!message.contains("token"));
        }
        other => panic!("unexpected outcome: {other:?}"),
    }
}
