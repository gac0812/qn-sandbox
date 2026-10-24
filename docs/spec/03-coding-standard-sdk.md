# 03 — SDK 编码规范（TypeScript / Python）

状态：Draft · 2026-10-24

## 0. 总原则

1. **契约先行**：SDK 的类型与 API 形态来自协议契约（ConnectRPC proto / OpenAPI，见 ADR-0003）。
   类型由生成器产出（`@connectrpc/connect` + `connect-query` / `connect-python`），手写代码只做"人体工学层"。
   **禁止手写与生成类型平行的重复定义**——发现生成类型难用，改契约，不改 SDK 手写层。
2. **三平台一个 API**：SDK 不暴露任何平台概念（WSL、VZ、镜像路径……）。平台差异在 daemon 端消化。
3. 参考 E2B SDK 的用户视角命名（`sandbox.run_code`、`sandbox.files.write`），但映射到本项目领域词（`Sandbox`/`Exec`/`Files`）。

## 1. 目录与生成物

```
sdks/typescript/
├── src/
│   ├── gen/            # 生成代码（R9：禁止手改，CI 校验与再生成结果一致）
│   ├── client.ts       # QnClient：连接管理、认证（token）、超时默认值
│   ├── sandbox.ts      # Sandbox 类：exec/files/lifecycle 人体工学层
│   ├── errors.ts       # 把 Connect 错误码映射为 QnError 子类
│   └── index.ts        # 公共出口（唯一 entry，internal/* 不导出）
├── package.json        # exports 字段严格；sideEffects: false
└── tsconfig.json       # strict: true, noUncheckedIndexedAccess: true
sdks/python/
├── src/qn_sandbox/
│   ├── _gen/           # 生成代码（同上，禁止手改）
│   ├── client.py       # QnClient
│   ├── sandbox.py      # Sandbox
│   └── errors.py
└── pyproject.toml      # ruff + mypy strict；py.typed 必须有
```

## 2. TypeScript 规则

- 顶层等待封装：公开方法全部返回 `Promise`；流式 exec 返回 `AsyncIterable<ExecEvent>`。
- 错误：`QnError` 基类 + `SandboxNotFoundError / ExecTimeoutError / PermissionDeniedError …`；
  每个错误带 `sandboxId`、`requestId` 字段方便报障。禁止把原始 RPC 错误对象抛给用户。
- 默认值集中在 `client.ts`（connect timeout 10s / exec 无超时但可传 / retry 只对幂等操作）。
- lint：eslint + `typescript-eslint` recommended-type-checked；`no-floating-promises: error`。
- 公共 API 100% TSDoc，示例进 `examples/` 并在 CI 中编译。

## 3. Python 规则

- 同步与异步双 API（`QnClient` / `AsyncQnClient`），实现共享同一通道层，禁止两份逻辑。
- 类型注解强制（mypy `strict = true`）；公开 API 100% docstring（Google style）。
- 错误层级与 TS 对齐（`QnError` 及子类），`__cause__` 保留原始异常链。
- lint/format：ruff（`select = ["E","F","I","UP","B","SIM"]`）。

## 4. 人体工学层允许做什么、不允许做什么

| 允许 | 不允许 |
| --- | --- |
| 合并"创建+等待就绪"为 `create(wait_ready=True)` | 隐藏状态（内存里缓存 sandbox 列表再过滤） |
| 便捷重载（字符串路径 → FilePath 对象） | 绕过契约新增 RPC 语义 |
| 把分页游标封装成 `async for` | 修改错误码语义（如把 4xx 吞掉变 None） |
| 本地校验（路径、超时非负）并提前报错 | 引入第二个 HTTP 客户端依赖（统一用契约生成器自带的） |

## 5. 版本与发布

- SDK 版本独立于宿主端（semver）；契约字段只增不改。
- 每次契约变更：重新生成 → diff review → SDK changelog 记"新增/废弃"，废弃项至少保留一个 minor 周期并在 doc 标注 `@deprecated`。
- 生成代码 CI 校验：`pnpm gen && git diff --exit-code` / 同理 Python。
