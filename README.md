# Viscos

> High-performance hybrid Discord client for Windows. Built with Rust + WebView2/CEF.

## Features

- ~200–300 MB RAM (vs Discord 500–1500 MB)
- < 2s cold start
- 15–25 MB binary (WebView2 backend) / ~240 MB (CEF backend)
- Native side panel (Faz 5)
- Auto-updater (Faz 8.0)
- Opt-in crash reporting (Faz 8.0)
- MSI installer + WinGet distribution (Faz 8.0)
- Pluggable render backend: WebView2 (light) ↔ CEF (leak-free) (Faz 8.5)

## Installation

### WinGet (recommended, post v0.1.0)

```powershell
winget install Winterus20.Viscos
```

### MSI installer

Download from [Releases](https://github.com/Winterus20/Viscos/releases) and run
`viscos-webview2.msi`. WebView2 Runtime will be installed automatically if
not present on the system.

### Build from source

```bash
git clone https://github.com/Winterus20/Viscos
cd Viscos
cargo build --release --bin viscos
./target/release/viscos.exe
```

## Disclaimer

Viscos is a third-party Discord client. Use at your own risk — Discord's
Terms of Service may prohibit automated clients. See
[`docs/AI-WORKFLOW.md`](docs/AI-WORKFLOW.md) and ADR-0011 for the canonical
disclaimer and risk discussion.

## Distribution Infrastructure

This repository ships with:

- **`installer/`** — WiX 3 MSI template (`viscos.wxs`) + PowerShell build script
  + WinGet manifest under `installer/winget/`.
- **`crates/viscos-distribution/`** — auto-updater, crash reporter stub, code
  signing stub, heap profiling (`dhat`), CEF backend management.
- **`config/default.toml`** — `[distribution]`, `[crash]`, `[profiling]`,
  `[chromium]`, `[cef]` sections for runtime configuration.

See [`docs/AI-WORKFLOW.md`](docs/AI-WORKFLOW.md) for release engineering
checklist (GUID generation, signtool, WinGet submission).

## Development

See [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) (TBD) and
[`.cursor/plans/viscos_index.md`](.cursor/plans/viscos_index.md) for the master
phase index.

## License

GPL-3.0