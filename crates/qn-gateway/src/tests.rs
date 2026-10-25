#![allow(clippy::unwrap_used)]
use super::*;

#[test]
fn it_preserves_exact_http_routes_and_methods_without_authorizing_them() {
    for (endpoint, origin, path) in [
        ("https://mcp.example/mcp", "https://mcp.example", "/mcp"),
        (
            "https://mcp.example:8443/v1/mcp",
            "https://mcp.example:8443",
            "/v1/mcp",
        ),
        ("http://127.0.0.1:8192/mcp", "http://127.0.0.1:8192", "/mcp"),
        ("http://[::1]:8192/", "http://[::1]:8192", "/"),
    ] {
        let route = HttpMcpRouting.plan("github_1", endpoint).unwrap();
        assert_eq!(route.name, "github_1");
        assert_eq!(route.origin, origin);
        assert_eq!(route.path, path);
        assert_eq!(route.methods(), ["POST", "GET", "DELETE"]);
        assert_eq!(format!("{route:?}"), "McpHttpRoute(..)");
    }
}
#[test]
fn it_refuses_credentials_aliases_ambiguous_paths_and_insecure_non_loopback() {
    for endpoint in [
        "",
        "relative/mcp",
        "file:///mcp",
        "https://mcp.example",
        "http://mcp.example/mcp",
        "http://localhost/mcp",
        "http://192.168.1.1/mcp",
        "https://user:credential-canary@mcp.example/mcp",
        "https://user@mcp.example/mcp",
        "https://mcp.example/mcp?credential-canary",
        "https://mcp.example/mcp#credential-canary",
        "https://mcp.example/mcp?",
        "https://mcp.example/mcp#",
        "https://mcp.example:0/mcp",
        "https://mcp.example:443/mcp",
        "https://MCP.EXAMPLE/mcp",
        "https://mcp.example/a/../mcp",
        "https://mcp.example/%2e%2e/mcp",
        "https://mcp.example/%2fmcp",
        "https://mcp.example//mcp",
        "http://2130706433/mcp",
        "http://127.1/mcp",
        "https://mcp.example/mcp\n",
        "https://mcp.example/mcp *",
    ] {
        let error = HttpMcpRouting.plan("route", endpoint).unwrap_err();
        assert_eq!(error, McpRouteError::Endpoint);
        assert!(!format!("{error} {error:?}").contains("canary"));
    }
}
#[test]
fn it_refuses_unmanaged_names_and_bounds_endpoint_allocation() {
    for name in [
        "",
        "Route",
        "1route",
        "../canary",
        "route/canary",
        "route*",
        "route canary",
    ] {
        assert_eq!(
            HttpMcpRouting
                .plan(name, "https://mcp.example/mcp")
                .unwrap_err(),
            McpRouteError::Name
        );
    }
    assert!(
        HttpMcpRouting
            .plan(&"r".repeat(65), "https://mcp.example/mcp")
            .is_err()
    );
    assert!(
        HttpMcpRouting
            .plan(&"r".repeat(64), "https://mcp.example/mcp")
            .is_ok()
    );
    assert!(
        HttpMcpRouting
            .plan(
                "route",
                &format!("https://mcp.example/{}", "x".repeat(4096))
            )
            .is_err()
    );
}
