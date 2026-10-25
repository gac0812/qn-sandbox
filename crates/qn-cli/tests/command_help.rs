//! Binary contracts for the top-level qn command surface.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_qn"))
        .args(args)
        .output()
        .expect("qn should execute")
}

#[test]
fn top_level_help_lists_all_current_commands() {
    let output = run(&["--help"]);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let text = String::from_utf8(output.stdout).expect("UTF-8 help");
    for command in ["install", "doctor", "profile", "audit"] {
        assert!(text.contains(command), "missing {command}: {text}");
    }
}

#[test]
fn command_help_keeps_the_standard_stream_and_exit_contract() {
    for args in [
        ["install", "--help"],
        ["doctor", "--help"],
        ["profile", "--help"],
        ["audit", "--help"],
    ] {
        let output = run(&args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
        assert!(
            String::from_utf8(output.stdout)
                .expect("UTF-8 help")
                .contains("Usage:")
        );
    }
}
