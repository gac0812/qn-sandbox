# qn-config

配置文件、`QN_*` 环境变量覆盖、平台路径与锁定常量的唯一权威（spec 01 §2；
R6 的 `#[cfg(target_os)]` 白名单位置之一）。

## 模块

| 模块 | 职责 |
| --- | --- |
| `paths` | 目录布局唯一权威（`QnPaths`：root/config.toml/token/sessions.json/profiles/audit/logs/cache）；`QN_HOME` 覆盖 > 平台默认（Windows `%LOCALAPPDATA%\qn`，Unix `~/.qn`）；**只解析不创建** |
| `settings` | `Config`：config.toml 加载 + 环境覆盖，语义 **env > toml > 默认**；首字段 `default_timeout_ms` |
| `consts` | 单点常量：`NONO_PINNED_VERSION`（锁版本跟版，spec 04 §6）、`WSL_MIN_KERNEL_SEMVER`（Landlock 网络下限，spike① #16） |
| `error`（私有 `mod`，经 crate 根 `pub use` 导出） | `Error::PlatformRootUnavailable` / `Error::EnvironmentNotUnicode` / `Error::Read` / `Error::Parse` |

## 约束

- **禁止凭证字段**：配置文件里出现密钥即违规（08 §4）。
- 布局变更必须同步架构拆分 §7 与 `paths` 文档。
- 新配置字段只增不改（serde 兼容），且必须保持三级覆盖语义。
- 环境读取是薄封装（`resolve`/`apply_env_overrides`），核心逻辑接受注入
  （`resolve_with`/`apply_env_overrides_with`）——edition 2024 下 `set_var`
  为 unsafe 且 workspace forbid，测试不许改进程环境。
- 环境变量存在但不是合法 UTF-8 时 fail closed，返回
  `Error::EnvironmentNotUnicode`，不回显非法原始字节。
- TOML 错误只暴露脱敏后的类别及行列，不转储配置源码、字段名或字段值。

## 非目标

- 运行时业务逻辑；profile 内容管理；路径创建（属使用方）。

`QnPaths.runtime` 是 `<root>/runtime`：前台会话的私有运行根、owner 记录和控制关联。
`platform_host_home()` 解析宿主 home 供 profile 物理路径防护；不可得或非 UTF-8 时返回
`None`，调用方必须 fail closed，不能跳过受保护路径检查。
