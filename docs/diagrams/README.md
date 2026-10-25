# 首页架构图

`homepage-architecture.mmd` 是可编辑的 Mermaid 源码；`homepage-architecture.svg` 由 Mermaid CLI 生成，禁止手改。

首页直接引用 SVG，避免依赖 GitHub 的异步 Mermaid iframe。图内容沿用原首页目标架构，源码配置关闭 HTML labels，让文字以原生 SVG 文本保存；白底在 GitHub 深浅色主题中都保持可读。

在仓库根目录重新生成（需 Node.js，Mermaid CLI 会使用 Puppeteer 的浏览器）：

```bash
npx --yes --package @mermaid-js/mermaid-cli@12.0.0 mmdc \
  -i docs/diagrams/homepage-architecture.mmd \
  -o docs/diagrams/homepage-architecture.svg
```

若使用已安装的 Chrome，可在生成命令前设置 `PUPPETEER_EXECUTABLE_PATH` 指向 Chrome 可执行文件。本次生成工具仅用于文档，不加入产品依赖。
