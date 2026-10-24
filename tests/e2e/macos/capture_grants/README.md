# Host capture / grant revoke native replay (#99)

本测试只使用合成认证值、临时项目和本机 HTTP upstream；需 macOS arm64、Rust 1.85、Python 3、系统 Ruby。
验证 nono 0.80.0 arm64 SHA-256，其他平台/版本拒绝运行。不是完整 Agent、MCP 或生产 broker 验收。

## 重放

先获取精确依赖提交（已存在时无需重复）：

```sh
git fetch origin feat/79-broker-grants feat/97-capture-channel feat/101-host-provider feat/103-capture-identity
python3 tests/e2e/macos/capture_grants/replay.py --nono /absolute/path/to/verified/nono
```

disposable worktree 由脚本自动创建/清理。原工作区和主分支不变；不推送或合并临时组合。
脚本使用 main 9fab276 + #80 a483fe1 + #98 54c7bda + #102 5819708 + #104 41248e8 的原始 library 文件，
只机械组合模块声明/manifest，并在临时 examples 放入本目录 fixture；不另写 grant/proxy。

## 已观察结果（2026-10-25）

macOS 26.4 arm64 / Rust 1.85.0 / nono 0.80.0：

- normal：200、503；upstream 仅收到一次正确的合成 Authorization；provider 只执行一次。
- race：provider 返回后、准入前 revoke；503、upstream 零请求。
- mismatch：预绑定 native ID 不匹配；503，provider 和 upstream 均零调用。
- sandbox：读取宿主 binding、provider-secret 和合成 ~/.ssh/canary 被拒；直连 broker 被拒；父环境秘密不存在；只有 phantom。
- provider 通过真实 CommandProvider→infra 执行 /bin/cat 读取临时宿主文件，凭证不进 argv/env。
- capture 缓存 TTL=0；真实 nono proxy 负责 phantom 校验、路由和认证附加。
- nono reverse route 的 request_host 实测为 qnprobe；以宿主存储的 route→origin 映射校验，而非信任自报 upstream。

## 限制

fixture 在 broker/nono 启动前生成随机 native ID，用 CaptureIdentity 严格验证每次请求；没有首次请求学习。
NONO_DETACHED_SESSION_ID 是锁版 main.rs/launch_runtime.rs 的私有启动机制，实测可绑定前台 capture，且沙箱环境不可见。
生产启动须在 qn-nono 内封装该版本依赖、保留拥有关系并验证状态；该重放不替代生产启动器/身份唯一性验收。
provider 使用合成文件而非开发者真实凭证库；未验证持久 serving、完整审计、Agent/MCP、Windows/WSL 或 #93 整树清理。
准入后的响应属于在途，normal 特意在发回复前 revoke：该已准入请求仍200，后续新请求503。
Python 仅编排测试 fixture；实际 nono 通过现有 qn-nono probe→qn-infra 启动。失败不打印秘密响应。
