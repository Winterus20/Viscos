# `viscos-webview`

Webview backend abstraction layer (Faz 1.0 → Faz 1.6 Dalga 1c).

ADR-0012 §2–§4, §6 / Faz 1.6 plan `phase-1.6-cef-default-rollout-dalga-1b.md`.

## Backends

| Backend   | Default? | Feature flag        | Status (Faz 1.6)         |
|-----------|----------|---------------------|--------------------------|
| WebView2  | ✓        | (none, always on)   | Real runtime (`wry`)     |
| CEF       | ✗        | `cef-backend`       | Feature-gated stub       |

`select_default_backend()` returns `WebView2` on:
- Windows 10
- Non-Windows targets
- Win11 builds with `cef-backend` feature **off** (B1 fallback)
- RDP session + `cef-backend` feature **off**

Returns `Cef` on:
- Windows 11 + `cef-backend` feature **on** (production build)
- RDP session + `cef-backend` feature **on**

CLI flag `--backend=webview2|cef|auto` always wins over config and platform default.

## Feature flags

| Feature                | Default | Purpose                                            |
|------------------------|---------|----------------------------------------------------|
| (none)                 | —       | WebView2 + CEF stub; everything cross-platform     |
| `cef-backend`          | OFF     | Real CEF runtime (`cef::BrowserHost::CreateBrowser`) |
| `test-cef-mock`        | OFF     | Mock `CefBrowser` handle for CI smoke              |
| `tao-runtime-tests`    | OFF     | Real `tao::EventLoop` window tests (skip headless) |

`cef-backend` implies CMake 3.21+, Ninja, Windows SDK 10.0.22621+ at build time.
Default build needs none of these (stub is pure Rust).

## Build prerequisites

| Tool        | Min version | Required when                  |
|-------------|-------------|--------------------------------|
| Rust        | 1.89         | Always                         |
| CMake       | 3.21         | `cef-backend` feature ON       |
| Ninja       | 1.10         | `cef-backend` feature ON       |
| Windows SDK | 10.0.22621   | `cef-backend` feature ON       |
| WebView2    | runtime      | WebView2 backend (default)     |

## Manual test checklist (Faz 1.6 Dalga 1b, 24h soak)

Run on dedicated hardware, monitor crashpad + telemetry:

- [ ] Win10 + WebView2 default, 24h no restart, no GDI leak (process working set stable)
- [ ] Win11 + WebView2 (cef-backend OFF), 24h, no leak
- [ ] Win11 + CEF (cef-backend ON), 24h, idle RAM delta < 150 MB
- [ ] RDP session + CEF forced, verify `select_default_backend()` returns `Cef`
- [ ] RDP session + WebView2 explicit override (`--backend=webview2`) is rejected
      by watchdog with telemetry event
- [ ] `cargo build --features viscos-webview/cef-backend` succeeds end-to-end
- [ ] `cargo build` (default) succeeds without CMake/Ninja installed

## Known issues

### WebView2 (default)

- **GDI region leak** under RDP: tracked in [WebView2Feedback #5266](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5266);
  watchdog throttles pointermove (Faz 1.5), CEF default in RDP (Faz 1.6).
- **DOM churn**: `frontend/src/bridge.ts` must use `[aria-label]`/`[role]`/`[data-*]`
  selectors — never hashed classnames (ADR-0012 §2).
- **IPC pull-by-default**: WebView2 buffer bloat risk; Rust → JS push only for
  small events (tray badge, native notification).

### CEF (feature-gated)

- **Startup time** 1.5–2.5s (Chromium initialization cost).
- **Binary size** 220–300 MB → Faz 8.5 self-update zorunlu.
- **Idle RAM** +50–100 MB vs WebView2.
- **Disk cache** +150 MB at `%APPDATA%/Viscos/cef-cache`.
- **Subprocess routing**, **V8 bridge**, **crashpad** — out of PR-2 scope
  (FOLLOW-UP-REAL-WORLD-WORK.md §B, human release engineering).

## Out of scope (PR-2)

These are deliberately left for human-owned follow-up PRs:

1. `cef::execute_process(args)` integration in `crates/viscos/src/main.rs`
   (subprocess routing — needs OS-aware dispatch).
2. V8 ↔ Rust bridge (`cef::V8Context`, `cef::V8Value`).
3. Crashpad integration + symbol upload pipeline.
4. 24h soak telemetry aggregation (Faz 1.5 watchdog → Faz 1.6 activation gate).
5. Self-update for `libcef.dll` (Faz 8.5).

## Module map

```
crates/viscos-webview/src/
├── lib.rs            # pub use exports, module docs
├── backend.rs        # WebViewBackend / WebViewWindow traits, select/resolve
├── webview2.rs       # WebView2 real runtime (wry + tao)
└── cef.rs            # CEF feature-gated stub + DLL check + subprocess marker
```
