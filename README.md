# XTR Log Viewer

Fast native log viewer for XTR / MTF / CARIAD test logs.  
Single binary — no runtime dependencies, no Java, no Python, no Electron.

## Build (one time)

```bash
# macOS / Linux
chmod +x build.sh && ./build.sh

# Windows (PowerShell)
cargo build --release
```

Requires Rust ≥ 1.75. If not installed: https://rustup.rs  
All crates compile into the binary — no DLLs, no installs needed on target machines.

## Run

```bash
./target/release/logviewer              # empty viewer — drop files
./target/release/logviewer run.log      # open file directly
```

## Features

| Feature | Detail |
|---|---|
| **Virtual scroll** | Renders only visible rows — handles 1M+ line files smoothly |
| **Level colors** | ERR red · WRN amber · INF green · DBG blue · TRC gray |
| **Level toggles** | Click ERR/WRN/INF/DBG/TRC to show/hide each tier |
| **Module filter** | Dropdown auto-populated from all `[module.name]` tokens |
| **Search** | Full-text, Ctrl+F to focus, Esc to clear |
| **Detail panel** | Click any row to expand timestamp, module, full raw line |
| **Drag & drop** | Drop .log or .txt onto the window |
| **Font size** | A▲ / A▼ buttons |
| **Copy raw** | Detail panel → ⧉ copy raw |
| **Dark theme** | Permanent, no config needed |

## Supported log formats

- **XTR / MTF / CARIAD**: `[2026-03-30 14:30:17.022] [INF] [module] message`
- **TLS-Attacker (Java)**: `14:32:54 [Thread-1] DEBUG: WorkflowExecutor - message`
- **DiagnosticToolBox**: `2026-03-30 14:32:44.282  [Module] [DBG]  message`
- **Generic**: any format with `[ERR]` / `[WARN]` / `[INFO]` etc.

## Keyboard shortcuts

| Key | Action |
|---|---|
| `Ctrl+O` | Open file dialog |
| `Ctrl+F` | Focus search box |
| `Esc` | Clear search / deselect row |
| Click row | Open detail panel |
| Click selected row | Toggle detail panel |

## Binary size

~6 MB stripped release build (statically linked).  
Runs on Windows 10+, macOS 11+, Linux (glibc 2.17+).
