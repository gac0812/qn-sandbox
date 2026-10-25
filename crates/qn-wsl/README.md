# qn-wsl

Windows WSL2 供给链路适配器（spec 01 §2；R6 的 `#[cfg(target_os)]` 白名单位置）。
Windows-only：crate 根整体 cfg 门，Linux/macOS 只编译不执行。

## 模块

| 模块 | 职责 | 状态 |
| --- | --- | --- |
| `health` | 五项只读探测 → 结构化 `ProbeReport`（Ok/Warn/Fail + remediation）；**失败是数据不是错误** | ✅ M2 首块（#74） |
| `decode` | wsl.exe 双格式输出解码：UI 命令（`--version`/`-l -v`）是 UTF-16LE，发行版内命令是 UTF-8 透传——从 `CmdOutcome::stdout_raw` 取原始字节再解码（不能走 lossy 往返，本地化字符会被 U+FFFD 破坏） | ✅ |
| `kernel` | `uname -r` 解析（非严格 semver）+ ≥6.7 Landlock 网络下限判定 + ABI floor 映射（[内核文档](https://docs.kernel.org/userspace-api/landlock.html) + spike① 实测校准） | ✅ |
| `distro` | 受管发行版幂等供给：三态纯函数计划（`plan_provision`）+ 编排执行（`ensure_present`）；种子策略 Ubuntu-24.04，**仅自装种子才 unregister** | ✅ M2（#85） |
| `bootstrap` | 发行版内 nono 引导：宿主侧 `qn_nono::installer` staging（锁版+SHA256+Linux target）→ `/mnt/<盘>` 递入 → `install -m 0755` → 版本复验；快路径=已锁定即返回 | ✅ M2（#126） |
| `upgrade` | 内核升级闭环（#17 裁决）：`wsl --update` → `wsl --shutdown`（**破坏性，需用户确认**）→ 复验 ≥6.7；update 失败不中断（内核可能已达标，verify 裁决）；失败 → offline-only 降级 + 手动渠道指引 | ✅ M2（#167） |
| kernel 升级——自编译内核分发 | 兜底路径 | ⏳ 按需 |

## 关键事实（探测语义依据）

- wsl.exe UI 输出经管道为 UTF-16LE（字节含 NUL 交错）；`wsl -d <name> -- cmd` 为 UTF-8 透传。
- `wsl -l -v` 的表头与状态词是**本地化**的——名称匹配按首列（兼容 `*` 前缀），不判定运行态。
- `wsl -d <name>` 会按需 boot 停止状态的发行版（探测不改文件系统，但有此可观测变化）。
- Landlock 网络限制需内核 ≥6.7（ABI v4）；WSL 3.0.3 自带内核 6.18（ABI 7）——spike① #16。

## 已知限制（upgrade，实测驱动）

- <6.7 → 6.18 的**真实升级链路未在本机实证**（本机已 6.18；计划构造/失败语义有单测，shutdown 前后行为依据微软文档与 spike 渠道矩阵）。
- `wsl --update` 在"已最新"或 CDN 受限时失败——闭环对此容错（verify 裁决），但**不会假装升级成功**（AlreadySatisfied vs Updated 语义区分）。

## 供给事实（distro，实测 2026-10-25）

已知限制：种子=Missing 的四步链（install→export→import→unregister）**未实证**
（本机种子已存在，走的是两步链）；`wsl -l -v` 列表漏报已存在种子时 install 行为
未验证（按现有实现会走 install 路径——首列 ASCII 匹配，漏报概率极低）；
`ensure_present` 单飞契约由调用方保证（并发会在固定 tar 路径互踩）。

- `wsl --import` 要求安装目录**已存在**（"系统找不到指定的路径"），ensure_present 负责创建
  install_dir 与 tar 父目录；导出 tar 用后即删（安装产物是发行版本身）。
- wsl 错误面走 stdout 且为 UTF-16——StepFailed 的 detail 携带解码后的 stdout 首行。
- 集成测试 `QN_WSL_PROVISION_IT=1 --features provision-it`（本机 opt-in，分钟级）：
  真实供给 → 二次调用幂等 no-op → health Distro 探测转 Ok 三段验证。

## dump_probes example

```powershell
cargo run -p qn-wsl --example dump_probes
```

五项探测的人读输出，兼作 #74 验收证据与 doctor（#44）对接预览。

## 非目标

- doctor 的 CLI/UI 呈现（qn-cli/qn-ui，经 qn-bootstrap 桥接——消费方案在 #44/#66 开工前定）；
- 自编译内核分发（升级闭环已覆盖官方渠道）；跨平台行为。
