# qn sandbox 产品介绍片 · 源码

这支 2 分 23 秒的介绍片完全由代码生成：

- **画面**：每个镜头写成网页里的纯函数 `render(t)`。同一个 `t` 永远画出同一帧，用无头 Chromium 逐帧截图，再由 ffmpeg 合成。
- **声音**：配乐、音效、字幕由同一份时间轴生成，所以每个声音都对在动作那一帧上。
- **旁白**：MiniMax TTS 合成。

成片在 [`docs/media/intro/`](../../docs/media/intro/)。故事方案见 [TREATMENT.md](TREATMENT.md)，素材授权见 [CREDITS](CREDITS)。

## 分层

| 层 | 文件 | 说明 |
| --- | --- | --- |
| 2D 桌面与界面 | `film/desktop.js`、`lib/macui.js` | Mac 风格的菜单栏、程序坞、窗口、通知、控制面板；所有图标都是原创绘制 |
| 3D 世界 | `lib/world3d.js`、`lib/voxel.js` | three.js 卡通着色加描边：玻璃屋、体素角色、钥匙架、架构叠板 |
| 角色 | `lib/lib.js`、`lib/sprites.js` | Clawd（18×10 像素网格）以及原创的 Codex、dsh |
| 2D 特效 | `lib/fx.js` | 爆炸框、速度线、盖章、便签、字幕条 |
| 镜头与时间轴 | `film/shots.js` | 15 个镜头；`layout()` 按旁白实际时长计算每个镜头长度、字幕和音乐分段 |
| 声音 | `audio/mix.py` | CC0 采样器加程序化音效；旁白开口时音乐自动压低；两处静默 |
| 旁白 | `film/vo/lines.json`、`tools/tts.py` | 20 句台词，每句单独设定情绪 |

## 重建

```bash
sh fetch-assets.sh                  # 字体（OFL）、VCSL 采样（CC0）、three.js
MINIMAX_KEY=<你的 key> sh build.sh   # 合成旁白（有缓存）→ 渲染 → 混音 → 封装 → 字幕 → 封面
AUDIO_ONLY=1 sh build.sh            # 只改声音时，不重渲画面
npm run storyboard                  # 每镜关键帧拼成分镜图
```

需要：Node 20+、Playwright + Chromium、ffmpeg、Python 3（numpy、scipy、pillow）。

旁白音频不入库。密钥只从环境变量 `MINIMAX_KEY` 读取，不会写进任何文件。

## 自检工具

- `tools/qa.py`：逐帧扫描黑帧、纯色帧、异常色帧。
- `tools/psnr.mjs`：随机帧在新页面重渲，和成片帧比对 PSNR，用来排除残影。
- `tools/srt.py`：导出字幕，并按「中文字数 ÷ 4.5 + 1.5 秒」检查每条字幕的停留时间。
- `tools/mux.sh`：两遍 loudnorm，响度 -14 LUFS，真峰值不超过 -2 dBTP。
