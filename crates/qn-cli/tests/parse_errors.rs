//! Binary contracts for parser diagnostics and informational output.

#![allow(clippy::unwrap_used, clippy::expect_used)] // spec 02 §7: test assertions

use std::process::Command;

fn qn() -> Command {
    Command::new(env!("CARGO_BIN_EXE_qn"))
}

#[test]
fn it_escapes_untrusted_text_in_parser_errors() {
    let untrusted = "bad\nconfiguration: OK\r\t\u{1b}[2J\u{85}\u{202e}";
    let escaped = "bad\\nconfiguration: OK\\r\\t\\u{1b}[2J\\u{85}\\u{202e}";
    let flag = format!("--{untrusted}");
    for arguments in [
        vec!["doctor", untrusted],
        vec![untrusted],
        vec!["doctor", &flag],
    ] {
        let output = qn().args(&arguments).output().expect("qn should execute");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty(), "parse errors belong on stderr");
        let stderr = String::from_utf8(output.stderr).expect("error must be UTF-8");
        assert!(
            stderr.contains(escaped),
            "unsafe argument must be escaped: {stderr:?}"
        );
        assert!(
            !stderr.contains("\nconfiguration: OK"),
            "argument must not forge a field"
        );
        for control in ['\r', '\t', '\u{1b}', '\u{85}', '\u{202e}'] {
            assert!(
                !stderr.contains(control),
                "raw control must not reach stderr: {stderr:?}"
            );
        }
        assert!(
            stderr.contains("\n\nUsage: qn"),
            "trusted usage layout must remain readable: {stderr:?}"
        );
    }
}

#[test]
fn it_preserves_help_and_version_output_streams() {
    for arguments in [vec!["--help"], vec!["doctor", "--help"], vec!["--version"]] {
        let output = qn().args(arguments).output().expect("qn should execute");
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty(), "help/version belong on stdout");
        let stdout = String::from_utf8(output.stdout).expect("output must be UTF-8");
        assert!(
            stdout.ends_with('\n'),
            "normal line endings must be preserved"
        );
        assert!(!stdout.contains("\\n"), "layout must not be escaped");
    }
}
