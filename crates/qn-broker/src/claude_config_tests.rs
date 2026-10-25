#![allow(clippy::unwrap_used)]
use super::*;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::{error::Error, fs};

#[test]
fn it_creates_the_fixed_template_without_overwriting_an_existing_config() {
    let root = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(root.path()).unwrap();
    write_claude_mcp_config(root, TEMPLATE).unwrap();
    let target = root.join(".claude.json");
    assert_eq!(fs::read(&target).unwrap(), TEMPLATE);
    fs::write(&target, b"existing-user-config").unwrap();
    assert!(matches!(
        write_claude_mcp_config(root, TEMPLATE),
        Err(ManagedConfigError::Io(_))
    ));
    assert_eq!(fs::read(&target).unwrap(), b"existing-user-config");
    assert_eq!(fs::read_dir(root).unwrap().count(), 1);
}

#[test]
fn it_rejects_non_template_configuration_before_touching_the_filesystem() {
    let root = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(root.path()).unwrap();
    let template: Value = serde_json::from_slice(TEMPLATE).unwrap();
    let mut attacks = vec![b"{secret-canary".to_vec(), vec![b' '; 4097]];
    for (pointer, value) in [
        ("/mcpServers/qn_mcp/url", json!("https://secret-canary/mcp")),
        (
            "/mcpServers/qn_mcp/headers/Authorization",
            json!("Bearer secret-canary"),
        ),
        ("/mcpServers/qn_mcp/type", json!("stdio")),
        ("/mcpServers/qn_mcp/url", json!("${HOST_SECRET}/mcp")),
    ] {
        let mut changed = template.clone();
        *changed.pointer_mut(pointer).unwrap() = value;
        attacks.push(serde_json::to_vec(&changed).unwrap());
    }
    for pointer in [
        "",
        "/mcpServers",
        "/mcpServers/qn_mcp",
        "/mcpServers/qn_mcp/headers",
    ] {
        let mut changed = template.clone();
        changed
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("secret-canary".into(), json!("secret-canary"));
        attacks.push(serde_json::to_vec(&changed).unwrap());
        let object = template.pointer(pointer).unwrap().as_object().unwrap();
        let (key, value) = object.iter().next().unwrap();
        let mut duplicate = serde_json::to_string(object).unwrap();
        duplicate.pop();
        write!(duplicate, ",{key:?}:{value}}}").unwrap();
        let original = serde_json::to_string(object).unwrap();
        let complete = serde_json::to_string(&template).unwrap();
        attacks.push(complete.replacen(&original, &duplicate, 1).into_bytes());
        let mut missing = template.clone();
        missing
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(key);
        attacks.push(serde_json::to_vec(&missing).unwrap());
    }
    for input in attacks {
        let failure = write_claude_mcp_config(root, &input).unwrap_err();
        let mut error: &dyn Error = &failure;
        loop {
            assert!(!format!("{error:?} {error}").contains("secret-canary"));
            match error.source() {
                Some(source) => error = source,
                None => break,
            }
        }
        assert_eq!(
            fs::read_dir(root).unwrap().count(),
            0,
            "invalid input created files"
        );
        fs::write(root.join(".claude.json"), b"known-good").unwrap();
        assert!(write_claude_mcp_config(root, &input).is_err());
        assert_eq!(fs::read(root.join(".claude.json")).unwrap(), b"known-good");
        assert_eq!(fs::read_dir(root).unwrap().count(), 1);
        fs::remove_file(root.join(".claude.json")).unwrap();
    }
}

#[test]
fn it_refuses_relative_roots_and_cleans_failed_publication() {
    assert!(matches!(
        write_claude_mcp_config(Utf8Path::new("relative"), TEMPLATE),
        Err(ManagedConfigError::InvalidRoot)
    ));
    let root = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(root.path()).unwrap();
    fs::create_dir(root.join(".claude.json")).unwrap();
    fs::write(root.join(".claude.json/sentinel"), b"untouched").unwrap();
    assert!(write_claude_mcp_config(root, TEMPLATE).is_err());
    assert_eq!(
        fs::read(root.join(".claude.json/sentinel")).unwrap(),
        b"untouched"
    );
    assert_eq!(fs::read_dir(root).unwrap().count(), 1);
}
