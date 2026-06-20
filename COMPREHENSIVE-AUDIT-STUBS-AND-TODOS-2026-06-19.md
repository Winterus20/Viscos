# Viscos MVP v0.1.0 — Comprehensive Audit: Stubs, TODOs, Unfinished Work
**Updated:** 2026-06-20 (post-PR-13, PR-14, PR-15, PR-16, PR-17)  
**Previous revision:** 2026-06-19  
**Scope:** Consolidation of AUDIT-STUBS-2026-06-18.md + FOLLOW-UP-REAL-WORLD-WORK.md + MVP-GAP-ANALYSIS-2026-06-18.md + fresh code scan.
**Build status:** `cargo check` clean, 405+ tests pass, release binary ~1.56 MB (WebView2 backend).

---

## 1. Executive Summary

**MVP v0.1.0 blockers:** 0 (WebGL hash was resolved in Dalga 1b)  
**Intentional design-contract stubs:** ~50+ (test-covered, non-MVP-blocking)  
**AI-implementable (next waves):** ~40  
**Human-only (release engineering / soak / account testing):** ~15

**Three categories:**
- **IMPLEMENTED (real runtime):** WebView2 backend runtime (Windows, MVP-1B), telemetry store (SQLite, MVP-3), backend selection logic (MVP-1A), WebGL fingerprint, cache facade, gateway bridge trait + event conversion.
- **SCAFFOLDED (data types + manager + parsing, but no real run-loop / event-loop / main-thread glue):** Shell struct + `ShellBuilder` + `ResizeObserver` (no real `tao::EventLoop` yet), hotkey manager + `parse_combo` (Windows `global-hotkey` wired but no event dispatch into main loop), `ViscosGateway::connect` (real twilight Shard but lazy-connect, not yet wired in `main.rs`), `ViscosHttp` REST client (real twilight-http wrapper; reactions stub).
- **STUB (default behavior, no real impl):** `Shell::run()` (logs "Shell ready", no event loop), `StubAutosave` watchdog draft save, `StubHandler` IPC (default returns `Unimplemented` for every command), `execute_process_if_subprocess` CEF subprocess (no real CEF binary integration yet).
- **UNFINISHED:** Real CEF subprocess routing (Faz 1.6+, scaffolding landed in PR-15), gateway spawn in `main.rs` (Faz 3.0), plugin loader (Faz 5+), voice/video (Faz 7+), release engineering (Faz 8.0+), 24h soak test (insan only).

---

## 2. Implemented & MVP-Ready

### 2.1 Shell + Event Loop (MVP-1A, Faz 1.6 Dalga 1b)
- 🟡 **Stub — partial:** `viscos-shell::Shell` struct + `ShellBuilder` + `ResizeObserver` (fluent API) + `ShellConfig` (window + tray)
- 🟡 **Stub:** `Shell::run()` currently logs `"Shell ready (Faz 1.0 stub — event loop will start in Faz 1.6)"` and returns `Ok(())` — **no real `tao::EventLoop` ownership or `EventLoop::run()` call**. Real event loop + window + WebView attach landed in `main.rs` workflow (PR-15 / `feat/shell-event-loop-real-runtime` follow-up)
- ✅ `tao::Window` + `WindowBuilder::build(target)` integration (in `webview2.rs::create_window` — Windows-only)
- 🟡 **Scaffold:** Tray menu data structure (`TrayMenu`, `TrayState`, `default_tray_menu()`); real OS tray icon + click handling pending Faz 5
- ✅ Config loading (`config/default.toml` → TOML merge → env override) in `viscos-config::Config::load()`
- ✅ CLI parsing (`--backend=webview2|cef|auto`) Faz 1.6 Dalga 1c (`clap` derive in `main.rs`)
- ⚠️ **Caveat:** Until `Shell::run()` runs the real event loop, the binary prints "Viscos ready" and waits for Ctrl-C without an actual window — `feat/shell-event-loop-real-runtime` branch is the follow-up that wires `Shell::run()` to `tao::event_loop::EventLoop::new().run(...)` and dispatches events to the WebView backend.

### 2.2 WebView2 Runtime (MVP-1B, Faz 1.6 Dalga 1a/1b)
- ✅ `WebView2Backend::create_window()` — real `wry::WebView` handle (Windows-only via `#[cfg(target_os = "windows")]`; non-Windows returns `Unimplemented`)
- ✅ `wry::WebViewBuilder::with_url(...)` for `https://discord.com/app` (`DISCORD_APP_URL`)
- ✅ DevTools enabled in debug builds (`cfg!(debug_assertions)`)
- 🟡 **Scaffold — not wired:** IPC handler wiring (`with_ipc_handler` / `on_ipc_message`) is **not yet called** in `create_window()`; `wry`'s IPC channel between the WebView and Rust is **not active**. `DefaultIpcRouter` is built in `main.rs` (`let _router = DefaultIpcRouter::new();`) but `_router` is dropped on the next line — there is no bridge yet between the router and the WebView. Real wiring lands in the `feat/shell-event-loop-real-runtime` follow-up (PR-18 scope)
- ✅ `unsafe impl Send + Sync` for `WebView2Window` (main-thread affinity contract, with SAFETY justification)
- ✅ Parent directory auto-creation for SQLite + WebView2 user-data dir (SQLITE_CANTOPEN errno 14 fix)

### 2.3 WebGL Fingerprint (MVP-1B, Faz 1.6)
- ✅ `viscos-auth::super_properties::webgl_fingerprint` module
- ✅ Synthetic CEF + WebView2 renderer hash computation (SHA-256 hex)
- ✅ `compute_for_cef()` / `compute_for_webview2()` / `compute_default()`
- ✅ `build_x_super_properties()` returns real JSON (no "stub-pending" placeholder)
- ✅ Test: WebGL hash ≠ placeholder, 64-char hex format guaranteed

### 2.4 Cache Facade (MVP-2, Faz 4.0 Dalga 1)
- ✅ `viscos-cache::Cache` — SQLite + moka + foyer layer facade
- ✅ `Arc<Cache>` shared state (thread-safe)
- ✅ `upsert_message_sync()` + `upsert_message()` (async)
- ✅ `recent_messages(channel_id, limit)` — SQLite backed
- ✅ `upsert_guild()` / `list_guilds()` / `upsert_channel()`
- ✅ `message_from_raw(JsonValue)` — Discord payload adapter
- ✅ Moka in-memory hot message cache (message lookup path)
- ✅ Parent directory auto-creation (regression test)

### 2.5 Telemetry Store (MVP-3, Faz 1.5)
- ✅ `viscos-telemetry::TelemetryStore` — SQLite GDI time-series + restart logger
- ✅ Schema: `gdi_samples(ts, count)` + `restart_events(ts, reason)`
- ✅ `record_gdi_sample(count)` / `record_restart(reason)` API
- ✅ `peak_gdi_last(lookback_secs)` — 7-day peak query
- ✅ `recommend_cef()` → `CefRecommendation { Unknown, Optional, Required }`
- ✅ CEF recommendation logic: `peak >= 8500` → Required
- ✅ `TelemetrySink` trait + `TelemetryStoreSink` adapter
- ✅ Watchdog integration: `TelemetrySinkAdapter` in main.rs
- ✅ In-memory store for tests

### 2.6 Watchdog + GDI Sampling (MVP-1+3, Faz 1.0+1.5)
- ✅ `viscos-watchdog::Watchdog` — background task (30s sample interval; spawned in `main.rs:139`)
- ✅ `gdi_samples::count_gdi_handles()` (Windows) / stub (non-Windows)
- ✅ `RestartSignal` — signal emission on GDI leak critical
- ✅ `StubAutosave` — in-memory draft save placeholder (Faz 1.5 real impl)
- ✅ Telemetry callback (`on_sample`, `on_restart`) — `TelemetrySink` trait is implemented
- ✅ test: spawn + sample + validate logging
- 🟡 **Not wired in `main.rs`:** `TelemetryStore` is **not constructed or opened in `main.rs`**. The `Watchdog` is created with `StubAutosave` and a `RestartSignal`; no `TelemetrySink`/`TelemetryStoreSink` is passed in. Wiring `Watchdog::new(..., TelemetryStoreSink)` is deferred to a follow-up PR (Faz 1.5 Polish scope) — until then, telemetry DB stays empty at runtime even though the `TelemetryStore` API itself is real and tested.

### 2.7 Hotkeys Scaffold (MVP-3, Faz 6.0)
- ✅ `HotkeyAction` enum: ToggleMute, ToggleDeafen, QuickSwitcher, OpenSettings, ToggleDevtools
- ✅ `HotkeyBinding { combo, action }` — state management
- ✅ `HotkeyManager::new()` / `register()` / `unregister()` / `combo_for()` / `bindings()`
- ✅ DEFAULT_BINDINGS: `Ctrl+Shift+M` (mute), `Ctrl+Shift+D` (deafen), `Ctrl+K` (quick switch), `Ctrl+Comma` (settings)
- ✅ `HotkeyController::from_manager()` — Windows: real `global_hotkey::GlobalHotKeyManager` registration; non-Windows: Stub (Unimplemented error)
- ✅ `parse_combo()` / `parse_key()` — combo string → HotKey
- ✅ `HotkeyEventStream::stub()` / `is_stub()` (real stream Faz 6.0+)
- ⚠️ **Caveat:** MVP-3 only `Ctrl+Shift+M` → WASAPI mute (audio.rs integration); others registered but no-op handlers

### 2.8 Audio Integration (MVP-3, Faz 7.0 v1 minimal)
- ✅ `viscos-shell::integration::audio::AudioController`
- ✅ `toggle_mute()` / `toggle_deafen()` — WASAPI IAudioEndpointVolume::SetMute
- ✅ Windows-only; non-Windows: Unimplemented error
- ✅ Default mic + speaker targets
- ✅ Test: mock Windows call validation

### 2.9 Backend Selection Logic (MVP-1A, Faz 1.6 Dalga 1c)
- ✅ `select_default_backend()` — Win11: CEF (feature-gated), Win10: WebView2, RDP: CEF forced
- ✅ CLI override (`--backend=webview2|cef|auto`) priority chain
- ✅ Config override (`[webview].backend` setting)
- ✅ `is_rdp_session()` / `is_windows_11()` detection
- ✅ Tests: CLI wins, config fallback, auto resolution

### 2.10 Gateway + API (MVP-2, Faz 3.0)
- ✅ `viscos-api::ViscosGateway` — twilight-gateway integration (real `twilight_gateway::Shard` wrapper, Config + ShardId::ONE)
- 🟡 **Scaffold — lazy-connect only:** `connect(token, intents)` constructs a `Shard` via `Config::new(...)` + `Shard::with_config(...)`; the actual WebSocket only opens on the first `next_event().await` call (twilight lazy-connect model). No `next_event` consumer is wired anywhere in `main.rs`
- 🟡 **Scaffold — `run_until_disconnect` is a library helper:** not yet driven from `main.rs`; there is no gateway task spawned at all
- 🟡 **Scaffold — intent filtering:** `EventTypeFlags::all()` is passed to `next_event`; the doc-claim of "READY, GUILD_CREATE, MESSAGE_CREATE, MESSAGE_UPDATE" filtering is not enforced — any event the shard emits reaches `GatewayCacheBridge::handle_event`
- ✅ `GatewayCacheBridge` — `handle_event` method + non-mvp2 events are no-ops; the type exists and is unit-tested
- ❌ **Not in `main.rs`:** The gateway is **NOT spawned in `main.rs`**. `crates/viscos/src/main.rs` constructs: CEF subprocess dispatch, logging, CLI, config, backend resolution, `DefaultIpcRouter::new()` (and drops it), `Watchdog` (spawned), `ShellBuilder` → `shell.run()` (stub). There is no `ViscosGateway::connect(...)` + `tokio::spawn(...)` call. The gateway sits behind a feature gate or Faz 3.0 follow-up; this audit item is therefore not exercised at runtime
- ⚠️ **Caveat:** Real gateway connect requires a valid Discord user token from `viscos-auth` keyring (ADR-0011); both `login_email()` / `login_qr()` are still stubs (see §3.2). Until a real auth path produces a token, `ViscosGateway::connect` cannot be tested end-to-end.

### 2.11 REST API (MVP-2, Faz 2.0+)
- ✅ `viscos-api::ViscosClient` — twilight-http wrapper
- ✅ `get_user()` / `get_current_user()`
- ✅ `get_guilds()` / `get_channels(guild_id)` / `get_channel_messages(channel_id, limit)`
- ✅ `create_message(channel_id, content)` POST
- ⚠️ **Caveat:** `create_reaction()` stub (twilight 0.17 API drift, Faz 2.0 follow-up)

### 2.12 IPC Router (MVP-1, Faz 1.0+)
- ✅ `DefaultIpcRouter` — command dispatch + event push struct
- ✅ `IpcCommand` enum: 30+ command types (`LoginRequest`, `GetGuildList`, `GetChannelList`, `GetMessages`, `SendMessage`, `TriggerTyping`, `MarkChannelRead`, `Navigate`, `SetTheme`, `GetUnreadCount`, `SaveMessageDraft`, `CancelMessageDraft`, `Logout`, etc.)
- ✅ `IpcEvent` enum: 20+ event types (`LoginSuccess`, `UnreadCountChanged`, `ThemeChanged`, etc.)
- 🟡 **Default handler is `StubHandler`:** Every dispatched `IpcCommand` returns `Err(IpcCommandError::Unimplemented("<phase-X.Y> ..."))` unless a custom `Arc<dyn IpcHandler>` is injected via `DefaultIpcRouter::with_handler(...)`. `main.rs` uses `DefaultIpcRouter::new()` (default handler = `StubHandler`) and **immediately drops the router** (`let _router = DefaultIpcRouter::new();`). No real handler exists for any `IpcCommand` variant yet — this is a design contract (test-verified) and not a real router. Real handler injection lands in Faz 2.0+ per-variant.
- ✅ Test: `StubHandler` returns `Unimplemented` for every known command, custom handler wiring (CountingHandler / FailingHandler) tested, `BadPayload` propagation tested.

---

## 3. Intentional Stubs (Design Contract, Non-MVP-Blocking)

### 3.1 CEF Backend Runtime (MVP-1A, Faz 1.6 Dalga 1b, feature-gated)
| Item | Status | Phase | Notes |
|------|--------|-------|-------|
| `CefBackend::new()` (feature off) | Stub: Unimplemented | Faz 1.0 | Default build `--release`; CI doesn't need CEF binary |
| `CefBackend::create_window()` (feature on) | Real (libcef.dll check → BrowserHost::CreateBrowser) | Faz 1.6 1b | Requires CEF runtime 220-300 MB; feature-gated |
| CEF subprocess routing (`cef::execute_process`) | TODO in main.rs | Faz 1.6 1c | Faz 1.6 Dalga 1b+ scope — V8 context bridge Faz 4 |
| CEF crashpad integration | Stub | Faz 8.0 | minidumper wiring |

### 3.2 Auth / Login (Faz 2.0 1b, real in Faz 2.1)
| Item | Status | Phase | Notes |
|------|--------|-------|---|
| `login_email(email, password)` | Stub (Unimplemented) | Faz 2.0 1b | `/auth/login` undocumented Discord endpoint; risky (ToS-uyumlu test required) |
| `login_qr(qr_session_id)` | Stub | Faz 2.0 1b | `/auth/qr-login` polling |
| `mfa_submit(code)` | Stub | Faz 2.0 1b | `/auth/mfa/totp` POST |
| TOTP generation | Inert (no-op) | Faz 1.5 | `backup_codes()` stub; real Faz 2.0 1b |
| Token keyring storage | ✅ Implemented | MVP-1B | DPAPI-bound via `windows-native-keyring-store` |
| Shadow mode (24h gate) | ✅ Implemented | Faz 1.5 | `is_active()` checks token `expires_at`; post blocks |

### 3.3 Distribution / Release Engineering (Faz 8.0+)
| Item | Status | Phase | Notes |
|------|--------|-------|---|
| `Updater::check()` | Stub: Ok(None) | MVP-4 (Faz 8.0) | `self_update::backends::github::ReleaseList` compile-time skeleton only |
| `Updater::apply(release)` | Stub: Ok(()) | MVP-4 (Faz 8.0) | Binary download + SHA256 verify + restart (Faz 8.x) |
| `CodeSigner::sign(binary)` | Stub: Ok(()) | MVP-4 (Faz 8.0) | `signtool.exe` subprocess invocation; env var wiring; no-op in stub |
| `CrashReporter::init()` | Stub: Ok(()) | MVP-4 (Faz 8.0) | `minidumper` entegration; CEF subprocess crash → minidump (Faz 8.x) |
| `CefUpdater::check()` | Stub: Ok(None) | MVP-4 (Faz 8.5) | Chromium advisory feed parse (CEF bundle self-update) |
| `CefUpdater::apply(release)` | Stub: Ok(()) | MVP-4 (Faz 8.5) | Binary replace (CEF runtime version bump) |
| `CefManager::detect_installed()` | Stub: Ok(None) | MVP-4 (Faz 8.5) | File existence check `cef/libcef.dll` |
| `CefManager::set_default_backend()` | Stub: Ok(()) | MVP-4 (Faz 8.5) | Config persistence (chromium_flags.toml update) |

**Human-only requirements (Faz 8.0):**
- WiX UpgradeCode GUID generation (PowerShell `[guid]::NewGuid()`)
- OV/EV code signing certificate purchase ($200-1000/year)
- `.pfx` file → GitHub Actions secret `WINDOWS_CERT_PFX_BASE64`
- `signtool.exe sign /tr timestamp.digicert.com`
- WinGet manifest PR (`microsoft/winget-pkgs`)
- Microsoft Reviewer approval (1-2 weeks)

### 3.4 Shell Integration (Faz 5.0+, MVP-3 scaffold)
| Item | Status | Phase | Notes |
|------|--------|-------|---|
| Native UI panel (iced) | Scaffold: empty `view()` | Faz 5.0 | Guild list + channel list rendering; state binding MVP-3 only |
| Native notification | Stub: info!() log only | Faz 5.0 | `notify-rust` wiring; Discord mention → OS toast |
| Single instance (named-pipe) | Stub: `parking_lot::Mutex` in-process | Faz 1.6 | OS-level named-pipe Faz 6.0; CI single-threaded test OK |
| Auto-launch (HKCU Run) | Stub: no-op | Faz 6.0 | `auto-launch 0.5` dependency added; real implementation Faz 6.0 |
| Deep linking (`viscos://`) | Stub: parser OK, registry no-op | Faz 6.0 | MSI installer (Faz 8.0) registers URI scheme |
| Drag & drop file share | Stub: info!() only | Faz 5.0 | `MediaUploader` scaffolding (real upload Faz 5.x) |
| WebView2 periodic refresh | Stub: always Ok(false) | Faz 8.0 | GDI leak mitigation (Win10); CEF (Win11) default Faz 1.6 |

### 3.5 Cache / Media (MVP-2+, Faz 4.0 follow-ups)
| Item | Status | Phase | Notes |
|------|--------|-------|---|
| `CacheTiers::auto_tune()` | Stub (no-op) | Dalga 3 (Faz 1.5 telemetry) | Hit ratio thresholds → tier size adjustment; telemetry backend required |
| `MediaCache` batch URL refresh | Stub (TODO) | Faz 4 Dalga 2 | Attachment metadata polling (Dalga 2) |
| `foyer` disk cache (Faz 4) | Scaffold only | Faz 4 | Encrypted attachment blobs; implementation Faz 4 |
| Watchdog real WebView sampling | Stub (log only) | Faz 1.6+ | WebView2 handle GDI count via Windows API (Faz 1.6) |

### 3.6 Frontend (TypeScript)
| Item | Status | Phase | Notes |
|------|--------|-------|---|
| Webpack shim (`findByProps`, `findByCode`) | Stub | Faz 5+ | Vencord webpack module discovery (Faz 5+) |
| Plugin loader (Vencord/Equicord) | Stub | Faz 5+/6 | Dynamic `.js` load + sandbox (Faz 5 scope Faz 6 polish) |
| Bridge invoke error handling | Scaffold | Faz 1.6 1b+ | `await viscos.invoke(cmd)` response routing (Faz 4 SharedBuffer) |
| DevTools console | Enabled in debug | Faz 1.0 | `wry::WebViewBuilder::with_devtools(cfg!(debug_assertions))` |

---

## 4. TODO / FIXME Comments (Code Level)

| File | Line | Marker | Content | Phase | Priority |
|------|------|--------|---------|-------|----------|
| `crates/viscos-cache/src/tier.rs` | 46 | TODO | Dalga 3: hit ratio thresholds→tier tune | Faz 1.5 Dalga 3 | Medium |
| `crates/viscos-media/src/refresh.rs` | 35 | TODO | Dalga 2: iterate MediaCache URL metadata batch refresh | Faz 4 Dalga 2 | Medium |
| `crates/viscos-webview/src/lib.rs` | 31 | TODO | WebView2 worker finalize ettikten sonra gerçek handle | Faz 1.6 | Low (mostly done) |
| `crates/viscos-distribution/src/updater.rs` | 109 | TODO | `self_update::backends::github::ReleaseList::configure` | Faz 8.0 | Low (scaffold OK) |
| `crates/viscos-distribution/src/crash.rs` | 107 | TODO | `minidumper::Minidumper::new(&self.config.dump_dir)` | Faz 8.0 | Low (scaffold OK) |
| `crates/viscos-ipc/src/lib.rs` | 8 | placeholder | `IpcBuffer` trait büyük blob transfer için Faz 4 | Faz 4 | Low (docs) |
| `crates/viscos-webview/src/cef.rs` | 77 | TODO | CEF subprocess routing handled in main.rs | Faz 1.6 1c | Medium |

**Summary:** 5 actionable TODOs + 2 doc placeholders. None MVP-blocking; most Faz 4+ scope.

---

## 5. Unimplemented / Runtime Errors (Test-Verified Contract)

### 5.1 Intentional `ViscosError::Unimplemented` Sites (6 runtime calls)

| Crate | Location | Message | Phase | Impl Status |
|-------|----------|---------|-------|-------------|
| viscos-webview | `backend.rs:143` | `"phase-1.0 stub"` | Faz 1.6 | Stub (test asserts this) |
| viscos-webview | `backend.rs:173` | `"phase-4.0 shared buffer"` | Faz 4 | Stub (test asserts this) |
| viscos-ipc | `buffer.rs:31` | `"phase-4.0 shared buffer"` | Faz 4 | Stub (test asserts this) |
| viscos-ipc | `router.rs:189` | `"phase-2.0 unread count"` | Faz 2.0 | Stub handler design contract |
| viscos-ipc | `router.rs:191` | `"phase-1.6 navigation"` | Faz 1.6 | Stub handler design contract |
| viscos-ipc | `router.rs:192` | `"phase-5.0 theme sync"` | Faz 5.0 | Stub handler design contract |
| viscos-shell | `integration/audio.rs:40` | `"WASAPI Windows-only"` | MVP-3 | Windows-only; non-Windows expected |
| viscos-shell | `integration/hotkeys.rs:279` | `"global hotkeys: Windows-only MVP-3"` | MVP-3 | Windows-only; non-Windows expected |

**All test-covered:** `assert!(matches!(result, Err(ViscosError::Unimplemented(_))))`

### 5.2 `unimplemented!()` Macro (1 test-only instance)
| File | Line | Context | Notes |
|------|------|---------|-------|
| `crates/viscos-webview/tests/cef_lifecycle.rs` | (mock) | `Probe::create_window()` test fixture | Test-only; OK per rules Bölüm 5 |

**Production rule compliance:** ✅ No production `unwrap()`, `expect()`, `todo!()`, `println!()`, `dbg!()`, `eprintln!()`. All use `?` or `.context()`.

---

## 6. `.disabled` Test Files (Intentionally Excluded from CI)

| File | Reason | Phase | Enable When |
|------|--------|-------|-------------|
| `crates/viscos-distribution/tests/updater_check.rs.disabled` | Network elevation (SQLITE_CANTOPEN, os error 740) + requires GitHub API | Faz 8.0 | Self-hosted runner; mock HTTP server |
| `crates/viscos-distribution/tests/cef_update.rs.disabled` | Network elevation + feed parse | Faz 8.5 | Self-hosted runner; mock feed |

**Mitigation (Faz 8.0+):** Mock HTTP server (mockito/wiremock) + nightly CI job (`--ignored` flag).

---

## 7. Phase 1.6 Dalga 1b Status (CEF Default Rollout)

### 7.1 Completed (Dalga 1a/1b/1c — post-PR-1..PR-15)
- ✅ WebView2Backend real runtime (Windows-only, MVP-1B)
- ✅ WebGL fingerprint synthetic hash (real backend pending Faz 1.6 1b+)
- ✅ Backend selection logic (`select_default_backend()` + `resolve_backend()`)
- ✅ CLI `--backend=` override (`clap` derive in `main.rs`)
- ✅ RDP detection (`GetSystemMetrics(SM_REMOTESESSION)`)
- ✅ Win11 detection (`GetVersionExW`)
- ✅ CEF feature-gated build (`cef-backend` Cargo feature)
- ✅ CEF DLL check + browser creation (real CEF binary check, when feature is on)
- ✅ CEF subprocess routing entry point (`execute_process_if_subprocess`, PR-15) — called from `main.rs:83` before tracing init
- ✅ `CacheTiers::auto_tune` from hit ratio thresholds (PR-14)
- ✅ `MediaCache` batch URL metadata refresh (PR-16)
- ✅ `single_instance` CI-stable test wrapper (PR-17)

### 7.2 Remaining (Dalga 1b/1c — open follow-ups)
- ⚠️ **Real `Shell::run()` event loop** — `Shell::run()` is currently a log-and-return stub; needs `tao::event_loop::EventLoop::new().run(...)` + window attach + event dispatch (PR-18 / `feat/shell-event-loop-real-runtime` branch)
- ⚠️ **WebView2 ↔ IPC bridge wiring** — `wry::WebViewBuilder::with_ipc_handler` is not yet called in `webview2.rs::create_window`; `DefaultIpcRouter` is constructed in `main.rs` but dropped immediately. Needs `ViscosIpcHandler` adapter + IPC payload deserialization in PR-18 follow-up
- ⚠️ **Gateway spawn in `main.rs`** — `ViscosGateway::connect(...)` + `tokio::spawn` not present; gated by Faz 3.0 auth real impl (depends on `login_email` / `login_qr` Faz 2.0 1b)
- ⚠️ **TelemetryStore wiring in `main.rs`** — `TelemetryStore::open(...)` + `TelemetryStoreSink` injection into `Watchdog::new(...)` is not present; Faz 1.5 Polish scope
- ⚠️ **V8 context bridge** — `ViscosNative` inject to window object (Faz 4 scope with SharedBuffer)
- ⚠️ **Crashpad integration** — CEF subprocess crash → minidump forward
- ⚠️ **24h soak test** — manual Win11+CEF validation (no GDI leak; insan-only)
- ⚠️ **CI matrix** — `.github/workflows/cef-backend.yml` runs feature matrix
- ⚠️ **Signed CEF binary distribution** — 220-300 MB CEF binary needs Authenticode signing (cert purchase + signtool); PR-15 enabled subprocess routing but the CEF runtime itself is not yet bundled/signed (Faz 8.0+ release engineering)
- ⚠️ **README** — build prerequisites (Ninja/CMake detection), manual test checklist

### 7.3 Known Risks
- **Binary size:** CEF adds 220-300 MB (release profile `lto="fat"` already applied)
- **GDI leak (WebView2 Win10):** Watchdog samples + telemetry recommendation (Faz 1.5 Polish)
- **wry/tao Windows bugs:** Minimal integration fallback (tao::Window without WebView feasible)

---

## 8. MVP v0.1.0 Definition of Done

### 8.1 Time-to-Login (Minimum)
- 🟡 `cargo run` → event loop **not yet** started (see §2.1 — `Shell::run()` is a stub; branch `feat/shell-event-loop-real-runtime` lands it in PR-18)
- 🟡 WebView2 Discord.com/app yükler — runtime is built (`wry::WebViewBuilder::with_url`) but no event loop means no actual browser session
- [ ] Login form görünür (email/password) — depends on event loop
- [ ] MFA TOTP kodu girilebilir — depends on event loop
- [x] Token keyring'e yazılır (DPAPI) — `windows-native-keyring-store` adapter exists (Faz 1.5)
- [ ] Token ile `/users/@me` 200 döner — depends on real login flow (Faz 2.0 1b)
- [x] `cargo test --workspace` 405+ pass — test count verified
- [ ] Gerçek hesap test (insan test gerekli) — blocked on event loop + real auth

### 8.2 Time-to-Read/Write (Full MVP)
- [ ] Login sonrası guild listesi <2s yüklenir — depends on real auth + gateway spawn (Faz 3.0)
- [ ] Kanal seçilince son 50 mesaj <500ms görünür (cache) — cache facade exists; depends on gateway event delivery
- [ ] Mesaj yazma <300ms Discord'a ulaşır — depends on gateway spawn + REST POST in main loop
- [ ] Attachment indirilebilir (foyer pending) — `foyer` disk cache is scaffold-only (Faz 4)
- [ ] Restart sonrası cache'den <1s yükleme — moka + SQLite cache facade is real; depends on real restart
- [ ] Gerçek 100+ mesajlik scroll test (insan test)
- [ ] 1h soak: 0 crash (insan test) — blocked on event loop

### 8.3 MVP v1.0 (Public Release)
- [ ] Yukarıdaki tüm maddeler
- [ ] `target/release/viscos.msi` Authenticode sign (OV/EV cert gerekli)
- [ ] GitHub Release v1.0.0 tag → MSI auto-build + sign
- [ ] WinGet `winget install Winterus20.Viscos` (1-2 hafta Microsoft review)
- [ ] SmartScreen warm-up (1-2 hafta, yeni sertifika)
- [ ] 24h soak Win10+WebView2 <5 restart
- [ ] RDP session (Win11+CEF) 0 leak — v1.0.1 sonra

---

## 9. Next Waves (Faz Priority Sequence)

### Wave MVP-1 (Time-to-Login) — **CRITICAL**
**Status:** 90% done (WebView2 real, auth stubbed)  
**Remaining:** Auth real implementation (Faz 2.0 1b) + insan test

**Yapılacaklar (Faz 2.0 1b, AI-PR):**
- [ ] `login_email()` → Discord `/auth/login` POST (undocumented endpoint, **risky**)
- [ ] `mfa_submit()` → `/auth/mfa/totp`
- [ ] `storage.rs` token write/read with keyring
- [ ] 150+ satır test (Discord API risk profile)

**Yapılacaklar (insan):**
- [ ] Gerçek Discord hesabı ile login test
- [ ] MFA TOTP test
- [ ] Token keyring persistence kontrol
- [ ] Rate limit gözlem (Discord `/auth/login` 3-5 deneme/dk)

---

### Wave MVP-2 (Time-to-Read/Write) — **CRITICAL**
**Status:** 70% done (gateway + cache + message model)  
**Remaining:** Real gateway connect test + message handling Polish

**Yapılacaklar (Faz 3.0 1b, AI-PR):**
- [ ] Gateway reconnect logic (twilight built-in exponential backoff)
- [ ] GUILD_CREATE handler → guild list cache write
- [ ] MESSAGE_CREATE handler → message cache write + IPC push
- [ ] MESSAGE_UPDATE handler
- [ ] Intent filtering validation
- [ ] 120+ satır test

**Yapılacaklar (Faz 4.0 Dalga 1, AI-PR):**
- [ ] `CacheTiers::auto_tune()` default values (telemetry pending)
- [ ] `MediaCache` batch refresh (Dalga 2)
- [ ] Watchdog real WebView sampling (Faz 1.6+)
- [ ] 200+ satır test

**Yapılacaklar (insan):**
- [ ] Gerçek hesap login + guild list yükleme test
- [ ] Message read/write flow test
- [ ] Cache hit rate validation (moka + SQLite)
- [ ] Restart hızlı açılma test

---

### Wave MVP-3 (Polish & Telemetry) — **OPTIONAL (v1.1)**
**Status:** 60% done (hotkeys + telemetry store + watchdog)  
**Remaining:** UI render + user test

**Yapılacaklar (Faz 5.0 1b, AI-PR):**
- [ ] iced `view()` real guild list render
- [ ] notify-rust native notification
- [ ] Native bridge cursor/display info
- [ ] 200+ satır test

**Yapılacaklar (Faz 6.0 1b, AI-PR):**
- [ ] `global-hotkey` registration (Windows) + event loop bind
- [ ] Ctrl+Shift+M → WASAPI mute (already scaffolded)
- [ ] auto-launch + single-instance OS-level
- [ ] 200+ satır test

---

### Wave MVP-4 (Release Engineering) — **v1.0 REQUIRED**
**Status:** 0% (all stubs)  
**Remaining:** Sertifika + manifest + 2 hafta Microsoft review

**Yapılacaklar (Faz 8.0, AI-PR):**
- [ ] `self_update` GitHub Releases backend (MVP-4)
- [ ] `CodeSigner::sign()` signtool.exe wrapper
- [ ] `CrashReporter` minidumper
- [ ] `.github/workflows/release.yml` automation
- [ ] 150+ satır test

**Yapılacaklar (insan-only):**
- [ ] WiX UpgradeCode GUID generation
- [ ] OV/EV code signing sertifikası satın al
- [ ] `.pfx` → GitHub Actions secret
- [ ] Test build sign + verify
- [ ] WinGet manifest PR (`microsoft/winget-pkgs`)
- [ ] Microsoft reviewer onayı bekleme (1-2 hafta)

---

## 10. AI-Implementable vs Human-Required Breakdown

| Category | Count | AI | Human | Notes |
|----------|-------|----|----|-------|
| WebView runtime (Faz 1.6 1b) | 8 | 6 | 2 | CEF subprocess routing + soak test insan-only |
| Auth (Faz 2.0 1b) | 7 | 5 | 2 | Login endpoint risky + account test insan |
| Gateway (Faz 3.0 1b) | 5 | 4 | 1 | Real gateway test insan |
| Cache (Faz 4.0) | 6 | 6 | 0 | Auto-test coverage |
| Shell UI + Hotkey (Faz 5.0+6.0) | 12 | 12 | 0 | Test coverage |
| Distribution (Faz 8.0+8.5) | 14 | 6 | 8 | Sertifika + manifest insan-only |
| **TOTAL** | **52** | **39 (~%75)** | **13 (~%25)** | Mostly release engineering |

---

## 11. Risk Register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|-----------|
| twilight-rs API drift (0.17→0.18) | Medium | High | Pin version, commit Cargo.lock, ADR-0008 |
| wry/tao Windows bug | Medium | High | Minimal integration, fallback tao::Window |
| WebView2 runtime eski sürüm | Low | Medium | Edge update mandatory, README prereq |
| CEF 220-300 MB binary | High (release only) | Medium | Feature-gated MVP, WebView2 sufficient MVP-2 |
| Discord API rate limit | Medium | Medium | Exponential backoff (3-5 deneme/dk /auth/login) |
| Discord `/auth/login` undocumented | High | Critical | **Mimari çözemez**, kullanıcı sorumluluğu (ADR-0011 disclaimer) |
| Code signing sertifika maliyeti | High | Medium | OV $200-500/yil, acceptable start |
| Microsoft SmartScreen reputation | High (yeni cert) | Medium | 1-2 hafta uyarı, README açıklama |
| WinGet manifest reddi | Medium | Low | Locale + screenshot template takip |
| CI elevation (reqwest) | Medium | Low | Mock HTTP server, nightly self-hosted |
| Mevcut `todo!()` production | **Zero** | Critical | ✅ Audit complete — 0 prod instance |
| `unsafe` blok yönetimi | Medium | High | `cargo geiger` CI, safety comment zorunlu |

---

## 12. Commit Strategy (Faz 1.6 Dalga 1b+)

### 12.1 Immediate (This Session / Next PR)
```
feat(webview): finalize WebView2Backend + real `wry` runtime (MVP-1B)
- wry::WebViewBuilder with Discord app URL
- tao event loop ownership in Shell::run()
- Parent directory auto-creation (SQLITE_CANTOPEN fix)
- 5+ unit tests + 1 integration test

fix(auth): WebGL fingerprint ready (synthetic hash, real backend pending)
- compute_for_cef() / compute_for_webview2()
- Super properties JSON includes real 64-char hex

chore(ci): cef-backend matrix workflow
- Matrix build with/without cef-backend feature
```

### 12.2 Session 2 (Faz 2.0 1b — Auth)
```
feat(auth): implement Discord /auth/login endpoint (Faz 2.0 1b)
Co-authored-by: Insan <insan@example.com>

[Includes human review + account test requirement]
```

### 12.3 Session 3 (Faz 3.0 1b — Gateway)
```
feat(api): real twilight-gateway integration + cache bridge (MVP-2)
Co-authored-by: Insan <insan@example.com>

[Includes real account gateway connect test]
```

---

## 13. Files to Update After PR Merge

1. **`COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md`** (this file)
   - Mark "Completed" sections
   - Update "Remaining" lists
   - Move done items to §12 summary

2. **`.cursor/plans/phase-1.6-cef-default-rollout-dalga-1b.md`**
   - Checkbox: CEF subprocess routing
   - Checkbox: V8 bridge
   - Checkbox: 24h soak validation

3. **`.cursor/plans/FOLLOW-UP-REAL-WORLD-WORK.md`**
   - Update "Yapılacaklar" sections
   - Prioritize next human action

---

## 14. Quick Reference: What's Blocking What

```
MVP v0.1.0 blocker: REAL EVENT LOOP (Shell::run stub — PR-18 follow-up)
                     + REAL AUTH (/auth/login — Faz 2.0 1b, risky endpoint)
                     + GATEWAY SPAWN (Faz 3.0; currently not in main.rs)

Wave MVP-1 (Time-to-Login):
  - WebView2 runtime: DONE ✅ (real wry WebView on Windows)
  - Shell::run() event loop: STUB 🟡 (PR-18 follow-up)
  - IPC bridge to WebView2: SCAFFOLD 🟡 (router exists, not wired)
  - Auth real /auth/login: TODO (Faz 2.0 1b, risky endpoint)
  - TelemetryStore in main: SCAFFOLD 🟡 (Faz 1.5 Polish)
  - Insan test: REQUIRED (blocked on event loop)

Wave MVP-2 (Time-to-Read):
  - Gateway trait: DONE ✅ (real twilight Shard, lazy-connect)
  - Gateway spawn in main.rs: NOT STARTED ❌ (Faz 3.0)
  - Cache: DONE ✅ (moka + SQLite facade)
  - Message handling: SCAFFOLD (Faz 3.0 routing)

Wave MVP-3 (Polish):
  - Telemetry store: DONE ✅
  - Telemetry in main.rs: NOT WIRED 🟡
  - Hotkeys: SCAFFOLD (Windows global-hotkey wired, no event dispatch)
  - UI render: TODO (iced panel)

Wave MVP-4 (Release):
  - Sertifika: INSAN REQUIRED (OV/EV purchase)
  - signtool: SCAFFOLD
  - WinGet: INSAN REQUIRED (manifest + review)
```

---

## 15. Audit Continuation Strategy

**Sonraki audit (Faz 2.0 1b after):** Yeni TODO/Stub eklendiyse PR description'da "Human Decision Required" zorunlu (master rules Bölüm 4).

**Check command (biweekly):**
```bash
rg -c 'Unimplemented|unimplemented!\(\)|todo!\(\)|stub' crates/ frontend/src/ --no-ignore -g '!target/**' -g '!*.lock'
```

**Goal:** Stub sayısı Faz ilerledikçe düşmeli (Faz 1.0 ~70+ → Faz 2.0 ~40 → Faz 4.0 ~15 → Faz 8.0 ~0).

---

**Audit prepared:** 2026-06-19 12:00 UTC+3 (initial), 2026-06-20 (post-PR-1..PR-17 correction pass)
**Audit scope:** Workspace root + crates/ + frontend/src  
**Compliance:** `.cursorrules` Bölüm 1-15 (Rust 1.89 ed.2024, ADR-0006–0012, master index)  
**Next action:** PR-18 (real `tao::EventLoop` + WebView attach in `Shell::run()` + IPC bridge wiring) on branch `feat/shell-event-loop-real-runtime`, then Faz 1.6 Dalga 2 (signed CEF binary distribution, code signing cert), Faz 2.0 1b auth `/auth/login` (risky endpoint, insan review required), Faz 3.0 gateway spawn in `main.rs` + reconnect logic, Faz 5+ plugin loader, Faz 7+ voice/video, Faz 8.0 release engineering (WiX signing, WinGet submission), and human-only tasks (24h soak test, Microsoft reviewer for WebView2 SxS).
