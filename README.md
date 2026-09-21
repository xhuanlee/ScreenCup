# ScreenCut

[English](./README.en.md) | 简体中文

一款用 Rust + Tauri v2 从零构建的跨平台录屏软件，支持 macOS 与 Windows。交互参考 CleanShot X，区域录制带全屏放大镜拾取器。

## 功能

- **录制源**：整个屏幕、指定应用窗口、自定义区域
- **音频**：可分别开关系统音频与麦克风，麦克风支持设备选择
- **CleanShot X 式区域选择**：全屏冻结画面 + 跟随光标的放大镜（实时坐标读数与中心十字）、全屏十字参考线、实时尺寸标签、四角拖拽调整、选区内拖动平移、方向键微调（Shift 大步进）、Enter 确认 / Esc 取消
- **输出**：画质预设（原画 / 1080P / 720P / 480P）、帧率（24 / 30 / 60）、鼠标光标开关、自定义保存目录
- **录制浮条**：悬浮于所有窗口之上，暂停 / 恢复、停止并保存、丢弃录制
- **中英双语**：在标题栏一键切换，选择会持久化
- **全局快捷键**：`⇧⌘R`（macOS）/ `Ctrl+Shift+R`（Windows）开始 / 停止，`⇧⌘P` / `Ctrl+Shift+P` 暂停

## 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面框架 | Tauri v2 |
| 屏幕捕获 | [scap](https://github.com/cap-Software/scap)（vendored 于 `vendor/scap`） |
| 视频编码 | ffmpeg（系统自带或 `SCREENCUT_FFMPEG` 指定） |
| 前端 | React 18 + TypeScript + Vite |
| 样式 | Tailwind CSS v4（CSS-first tokens） |
| 动画 | framer-motion |
| 状态 | zustand |

## 本地开发

需要 Rust（stable）、Node.js 与 ffmpeg。

```bash
# 安装依赖
pnpm install

# 开发模式（同时编译 Rust 与前端，热重载前端）
pnpm tauri dev

# 构建正式版
pnpm tauri build
```

macOS 上首次运行需授予屏幕录制权限；若从 DMG 直接打开会被系统拒绝，请拖入「应用程序」后再运行。

### 自动测试

仓库自带一套 E2E 自测通道，通过环境变量驱动真实窗口完成端到端验证：

```bash
SCREENCUT_E2E=1 SCREENCUT_E2E_MODE=<mode> cargo run --release
```

可用场景：

| 模式 | 验证内容 |
| --- | --- |
| `plain` | 整屏录制并探测产物 |
| `audio` / `mic` | 系统音频 / 麦克风录制 |
| `window` | 应用窗口模式 |
| `region` | 区域模式后端链路 |
| `regionui` | 驱动真实区域遮罩：拖拽、方向键微调、Enter 确认、录制 |
| `still` | 放大镜静态帧抓取并经 asset 协议读回 |
| `lang` | 中英语言切换与持久化 |
| `pause` | 暂停 / 恢复 |

Rust 单元与集成测试：

```bash
cd src-tauri && cargo test --offline
```

## 项目结构

```
src-tauri/src/   Rust 后端（捕获、编码、设置、窗口管理）
src/             前端
  components/    主面板组件
  region/        区域选择遮罩（放大镜拾取器）
  bar/           录制浮条
  i18n/          中英双语字典与运行时
  e2e/           E2E 自测场景
vendor/scap/     屏幕捕获库（vendored）
```

## 许可

MIT
