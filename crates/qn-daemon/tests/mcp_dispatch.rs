//! MCP dispatch contracts over the shared daemon composition.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use qn_daemon::ControlPlane;
use qn_mcp::{ResultErrorCode, ToolName, ToolOutcome, ToolRequest};

fn plane() -> ControlPlane {
    ControlPlane::new(qn_config::paths::QnPaths::from_root(
        "/tmp/qn-daemon-mcp-contract",
    ))
    .unwrap()
}

#[test]
fn it_dispatches_profile_list_and_show_as_safe_success_values() {
    let result = plane().dispatch_mcp_request(&ToolRequest {
        schema_version: 1,
        request_id: "list-1".into(),
        tool: ToolName::ProfileList,
        arguments: serde_json::json!({}),
    });
    match result.outcome {
        ToolOutcome::Ok { value } => assert!(value["profiles"].is_array()),
        other @ ToolOutcome::Error { .. } => panic!("unexpected outcome: {other:?}"),
    }

    let result = plane().dispatch_mcp_request(&ToolRequest {
        schema_version: 1,
        request_id: "show-1".into(),
        tool: ToolName::ProfileShow,
        arguments: serde_json::json!({"id": "qn/claude-code"}),
    });
    match result.outcome {
        ToolOutcome::Ok { value } => assert_eq!(value["id"], "qn/claude-code"),
        other @ ToolOutcome::Error { .. } => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn it_denies_mutating_tools_and_rejects_credential_arguments() {
    for tool in [
        ToolName::SessionRun,
        ToolName::SessionStop,
        ToolName::McpRoutePlan,
    ] {
        let result = plane().dispatch_mcp_request(&ToolRequest {
            schema_version: 1,
            request_id: "deny-1".into(),
            tool,
            arguments: serde_json::json!({}),
        });
        assert!(matches!(
            result.outcome,
            ToolOutcome::Error {
                code: ResultErrorCode::Denied,
                ..
            }
        ));
    }
    let result = plane().dispatch_mcp_request(&ToolRequest {
        schema_version: 1,
        request_id: "secret-1".into(),
        tool: ToolName::ProfileList,
        arguments: serde_json::json!({"password": "secret-canary"}),
    });
    assert!(matches!(
        result.outcome,
        ToolOutcome::Error {
            code: ResultErrorCode::InvalidArguments,
            ..
        }
    ));
    let rendered = serde_json::to_string(&result).unwrap();
    assert!(!rendered.contains("secret-canary"));
}

#[test]
fn it_returns_closed_errors_for_missing_session_sources() {
    for tool in [ToolName::SessionStatus, ToolName::AuditQuery] {
        let result = plane().dispatch_mcp_request(&ToolRequest {
            schema_version: 1,
            request_id: "missing-1".into(),
            tool,
            arguments: serde_json::json!({"session": qn_core::SessionId::new().to_string()}),
        });
        assert!(matches!(
            result.outcome,
            ToolOutcome::Error {
                code: ResultErrorCode::Failed,
                ..
            }
        ));
    }
}
