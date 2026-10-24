//! qn-config 集成测试：路径布局、配置优先级矩阵、错误路径。
//!
//! 环境变量逻辑经 `apply_env_overrides_with` 注入 lookup 测试——
//! edition 2024 的 `set_var` 是 unsafe 且 workspace forbid unsafe，
//! 因此生产入口 `apply_env_overrides`/`QnPaths::resolve` 的 env 读取
//! 保持薄封装，不在测试中直接改进程环境。
#![allow(clippy::unwrap_used, clippy::expect_used)] // spec 02 §7：测试允许 unwrap/expect

use std::time::Duration;

use qn_config::paths::{QN_HOME_ENV, QnPaths};
use qn_config::{Config, DEFAULT_TIMEOUT_MS, ENV_PREFIX, Error, NONO_PINNED_VERSION};

#[test]
fn it_builds_the_standard_layout_from_root() {
    let p = QnPaths::from_root("/base/qn");
    assert_eq!(p.root, camino::Utf8PathBuf::from("/base/qn"));
    assert_eq!(
        p.config_file,
        camino::Utf8PathBuf::from("/base/qn/config.toml")
    );
    assert_eq!(p.token, camino::Utf8PathBuf::from("/base/qn/token"));
    assert_eq!(
        p.sessions,
        camino::Utf8PathBuf::from("/base/qn/sessions.json")
    );
    assert_eq!(p.profiles, camino::Utf8PathBuf::from("/base/qn/profiles"));
    assert_eq!(p.audit, camino::Utf8PathBuf::from("/base/qn/audit"));
    assert_eq!(p.logs, camino::Utf8PathBuf::from("/base/qn/logs"));
    assert_eq!(p.cache, camino::Utf8PathBuf::from("/base/qn/cache"));
}

#[test]
fn it_resolves_qn_home_override_over_platform_default() {
    let overridden =
        QnPaths::resolve_with(Some("/portable/qn-home".to_owned())).expect("override must win");
    assert_eq!(
        overridden.root,
        camino::Utf8PathBuf::from("/portable/qn-home"),
        "QN_HOME must take precedence over the platform default"
    );
}

#[test]
fn it_falls_back_to_platform_default_root() {
    let resolved = QnPaths::resolve_with(None).expect("platform default must resolve");
    if cfg!(windows) {
        let expected = std::env::var("LOCALAPPDATA")
            .map(|base| camino::Utf8PathBuf::from(base).join("qn"))
            .expect("LOCALAPPDATA must exist on windows runners");
        assert_eq!(resolved.root, expected);
    } else {
        let expected = std::env::var("HOME")
            .map(|base| camino::Utf8PathBuf::from(base).join(".qn"))
            .expect("HOME must exist on unix runners");
        assert_eq!(resolved.root, expected);
    }
}

#[test]
fn it_treats_blank_qn_home_as_unset() {
    let resolved = QnPaths::resolve_with(Some("   ".to_owned()))
        .expect("blank override must fall back, not error");
    assert_eq!(
        resolved.root,
        QnPaths::resolve_with(None).expect("default").root
    );
}

#[test]
fn it_uses_builtin_defaults_when_file_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = QnPaths::from_root(
        camino::Utf8PathBuf::from_path_buf(dir.path().to_owned()).expect("utf-8 temp path"),
    );
    let cfg = Config::load(&paths).expect("missing file must mean builtin defaults");
    assert_eq!(cfg, Config::builtin());
    assert_eq!(
        cfg.command_timeout(),
        Duration::from_millis(DEFAULT_TIMEOUT_MS)
    );
}

#[test]
fn it_loads_toml_overrides() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_owned()).expect("utf-8 temp path");
    std::fs::write(root.join("config.toml"), "default_timeout_ms = 5000\n").expect("write config");
    let cfg = Config::load(&QnPaths::from_root(root)).expect("valid toml must load");
    assert_eq!(cfg.default_timeout_ms, Some(5000));
    assert_eq!(cfg.command_timeout(), Duration::from_millis(5000));
}

#[test]
fn it_rejects_malformed_toml_with_typed_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_owned()).expect("utf-8 temp path");
    std::fs::write(root.join("config.toml"), "default_timeout_ms = [broken\n")
        .expect("write config");
    let err = Config::load(&QnPaths::from_root(root)).expect_err("malformed toml must fail");
    match err {
        Error::Parse { path, message } => {
            assert!(
                path.ends_with("config.toml"),
                "error must carry the path: {path}"
            );
            assert!(!message.is_empty(), "parse message must be actionable");
        }
        other => panic!("expected Error::Parse, got: {other:?}"),
    }
}

#[test]
fn it_rejects_unknown_keys() {
    // deny_unknown_fields：拼错键名必须在加载期暴露，而不是被静默忽略。
    let dir = tempfile::tempdir().expect("tempdir");
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_owned()).expect("utf-8 temp path");
    let secret = "sentinel-config-secret";
    std::fs::write(
        root.join("config.toml"),
        format!("api_key = \"{secret}\"\n"),
    )
    .expect("write config");
    let err = Config::load(&QnPaths::from_root(root)).expect_err("typo'd key must fail");
    let rendered = err.to_string();
    assert!(rendered.contains("unknown config key"), "got: {rendered}");
    assert!(
        !rendered.contains(secret),
        "error must not echo the credential value: {rendered}"
    );
    assert!(
        !rendered.contains("api_key"),
        "error must not echo the credential field: {rendered}"
    );
}

#[test]
fn it_applies_env_over_toml() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_owned()).expect("utf-8 temp path");
    std::fs::write(root.join("config.toml"), "default_timeout_ms = 5000\n").expect("write config");
    let cfg = Config::load(&QnPaths::from_root(root))
        .and_then(|c| {
            c.apply_env_overrides_with(&|key| {
                (key == "QN_DEFAULT_TIMEOUT_MS").then(|| "9999".to_owned())
            })
        })
        .expect("env override must apply");
    assert_eq!(
        cfg.default_timeout_ms,
        Some(9999),
        "env must beat toml (env > toml > default)"
    );
}

#[test]
fn it_rejects_non_numeric_env_value() {
    let secret = "sentinel-env-value";
    let err = Config::builtin()
        .apply_env_overrides_with(&|key| {
            (key == "QN_DEFAULT_TIMEOUT_MS").then(|| secret.to_owned())
        })
        .expect_err("non-numeric env must fail");
    match err {
        Error::Parse { path, message } => {
            assert!(
                path.to_string().contains("QN_DEFAULT_TIMEOUT_MS"),
                "error must point at the env key: {path}"
            );
            assert!(
                message.contains("not a valid u64"),
                "message must explain the expected type"
            );
            assert!(
                !message.contains(secret),
                "message must not echo the bad value: {message}"
            );
        }
        other => panic!("expected Error::Parse, got: {other:?}"),
    }
}

#[test]
fn it_exposes_public_constants_consistently() {
    // 常量单点 + 与 spike① 报告一致（#16）。
    assert_eq!(NONO_PINNED_VERSION, "0.80.0");
    assert_eq!(ENV_PREFIX, "QN_");
    assert_eq!(QN_HOME_ENV, "QN_HOME");
}

#[test]
fn it_pins_the_wsl_kernel_floor_constant() {
    // 与 spike① 报告（#16）H3 结论一致：Landlock 网络需要内核 >= 6.7。
    assert_eq!(qn_config::WSL_MIN_KERNEL_SEMVER, "6.7.0");
}

#[test]
fn it_reports_read_error_when_config_file_is_a_directory() {
    // config.toml 被目录占据：read_to_string 必失败且非 NotFound → Error::Read。
    let dir = tempfile::tempdir().expect("tempdir");
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_owned()).expect("utf-8 temp path");
    std::fs::create_dir_all(root.join("config.toml")).expect("mkdir as config file");
    let err = Config::load(&QnPaths::from_root(root)).expect_err("dir-as-file must fail");
    match err {
        Error::Read { path, source: _ } => {
            assert!(
                path.ends_with("config.toml"),
                "error must carry the path: {path}"
            );
        }
        other => panic!("expected Error::Read, got: {other:?}"),
    }
}

#[test]
fn it_normalizes_env_values_in_the_injected_core() {
    // 归一化（trim + 空串=未设置）长在 apply_env_overrides_with 核心里：
    // 空白值不得覆盖 toml，带空白的有效值应被 trim 后生效。
    let base = Config {
        default_timeout_ms: Some(5000),
    };
    let untouched = base
        .clone()
        .apply_env_overrides_with(&|key| (key == "QN_DEFAULT_TIMEOUT_MS").then(|| "   ".to_owned()))
        .expect("blank env must be treated as unset");
    assert_eq!(
        untouched.default_timeout_ms,
        Some(5000),
        "blank env keeps the toml value"
    );

    let trimmed = base
        .apply_env_overrides_with(&|key| {
            (key == "QN_DEFAULT_TIMEOUT_MS").then(|| " 7000 ".to_owned())
        })
        .expect("padded numeric env must apply after trim");
    assert_eq!(trimmed.default_timeout_ms, Some(7000));
}
