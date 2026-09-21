# ScreenCut

English | [简体中文](./README.md)

A cross-platform screen recorder built from scratch with Rust + Tauri v2, targeting macOS and Windows. Region capture uses a CleanShot X–style full-screen magnifier picker.

## Features

- **Capture sources**: full screen, a specific application window, or a custom region
- **Audio**: independent toggles for system audio and microphone, with per-device microphone selection
- **CleanShot X–style region picking**: a frozen full-screen frame plus a cursor-following magnifier (live coordinate readout and centre crosshair), full-screen guide lines, a live W×H badge, corner handles for resizing, drag-to-move inside the selection, arrow-key nudging (Shift for bigger steps), Enter to confirm, Esc to cancel
- **Output**: quality presets (Original / 1080P / 720P / 480P), frame rate (24 / 30 / 60), mouse-cursor toggle, custom output directory
- **Floating recording bar**: stays above other windows; pause / resume, stop and save, or discard
- **Bilingual UI**: switch between Chinese and English from the title bar; the choice is persisted
- **Global shortcuts**: `⇧⌘R` (macOS) / `Ctrl+Shift+R` (Windows) to start / stop, `⇧⌘P` / `Ctrl+Shift+P` to pause

## Tech stack

| Layer | Technology |
| --- | --- |
| Desktop framework | Tauri v2 |
| Screen capture | [scap](https://github.com/cap-Software/scap) (vendored at `vendor/scap`) |
| Video encoding | ffmpeg (bundled with the system, or set via `SCREENCUT_FFMPEG`) |
| Frontend | React 18 + TypeScript + Vite |
| Styling | Tailwind CSS v4 (CSS-first tokens) |
| Animation | framer-motion |
| State | zustand |

## Development

You need Rust (stable), Node.js, and ffmpeg.

```bash
# Install dependencies
pnpm install

# Dev mode (compiles Rust and the frontend, with frontend hot reload)
pnpm tauri dev

# Production build
pnpm tauri build
```

On macOS the first launch needs screen-recording permission. An app opened straight from a mounted DMG will be refused by the system — drag it into Applications first.

### Automated testing

The repo ships an E2E self-test channel that drives the real windows end to end via environment variables:

```bash
SCREENCUT_E2E=1 SCREENCUT_E2E_MODE=<mode> cargo run --release
```

Available scenarios:

| Mode | What it verifies |
| --- | --- |
| `plain` | Full-screen recording, then probes the output |
| `audio` / `mic` | System audio / microphone recording |
| `window` | Application-window mode |
| `region` | Region-mode backend pipeline |
| `regionui` | Drives the real region overlay: drag, arrow-key nudge, Enter confirm, then records |
| `still` | Magnifier still-frame capture, read back through the asset protocol |
| `lang` | Chinese/English switching and persistence |
| `pause` | Pause / resume |

Rust unit and integration tests:

```bash
cd src-tauri && cargo test --offline
```

## Project layout

```
src-tauri/src/   Rust backend (capture, encoding, settings, window management)
src/             Frontend
  components/    Main panel components
  region/        Region selection overlay (magnifier picker)
  bar/           Floating recording bar
  i18n/          Chinese/English dictionaries and runtime
  e2e/           E2E self-test scenarios
vendor/scap/     Screen capture library (vendored)
```

## License

MIT
