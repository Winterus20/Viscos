---
name: Phase 1.6 — CEF Default Rollout Dalga 1b (Implementation Status)
overview: Faz 1.6 Dalga 1a + 1b + 1c implementation playbook. WebView2Backend real runtime (Dalga 1a), CEF feature-gated stub (Dalga 1b), CLI/backend detection (Dalga 1c). MVP-1B altyapısı tamamlandı; Faz 1.6 release engineering (gerçek BrowserHost::CreateBrowser, V8 bridge, crashpad, 24h soak) insan PR'larına bırakıldı.
isProject: false
todos:
  - id: cef-feature-stub
    content: CEF feature-gated stub (Cargo feature cef-backend, real DLL check, BrowserHost::CreateBrowser scaffold)
    status: completed
  - id: backend-cli
    content: CLI --backend=webview2|cef|auto override + config fallback
    status: completed
  - id: platform-detect
    content: Win11 (GetVersionExW build ≥22000) + RDP (GetSystemMetrics SM_REMOTESESSION) detection
    status: completed
  - id: ci-matrix
    content: CI matrix workflow (.github/workflows/size-gate.yml) — WebView2 ≤30 MB + CEF ≤320 MB gate
    status: completed
  - id: msi-fixture
    content: WiX fixture (viscos.wxs BACKEND preprocessor + UpgradeCode GUID placeholder)
    status: completed
  - id: cef-binary-build
    content: "Gerçek cef::BrowserHost::CreateBrowser call — PR-2 sonrası release engineering"
    status: pending
  - id: v8-bridge
    content: "V8 context bridge (Faz 4 SharedBuffer ile) — Faz 4 scope"
    status: pending
  - id: crashpad
    content: "CEF subprocess crashpad → minidumper — Faz 8.0 scope"
    status: pending
  - id: soak-24h
    content: "24h soak Win11+CEF (GDI leak yok, restart 0) — manuel insan validation"
    status: pending
---

# Phase 1.6 — CEF Default Rollout Dalga 1b (Implementation Status)

> **Dalga:** 1a + 1b + 1c tamamlandı (PR-2 ile birleşik)
> **Tarih:** 2026-06-19
> **Durum:** Dalga 1a (WebView2 real runtime) ✅, Dalga 1b (CEF feature-gated stub) ✅, Dalga 1c (CLI + backend detection) ✅ — Faz 1.6 release engineering (gerçek `cef::BrowserHost::CreateBrowser`, V8 bridge, crashpad, 24h soak) **release engineering PR'larına bırakıldı**.
> **Önceki doküman:** [`phase-1.6-cef-default-rollout.md`](./phase-1.6-cef-default-rollout.md) (orijinal plan, ADR-0012 §6 ile genişletildi).
> **Sonraki adım:** Faz 2.0 (Auth) — `phase-2.0-discord-api.md` ve Faz 8.0 (Release Engineering) — `phase-8.0-distribution.md`.

---

## 1. Bu Playbook Ne İşe Yarar?

[`phase-1.6-cef-default-rollout.md`](./phase-1.6-cef-default-rollout.md) **plan taslağı**dır — bu doküman ise **implementation durumunu** kayıt altına alır. AI-PR'larının Faz 1.6 kapsamında neleri tamamladığını, nelerin release engineering'e bırakıldığını somutlaştırır.

**Neden ayrı dosya?** Faz 1.6'nın orijinal planı 14 bölüm + trade-off matrisi + insan karar noktaları içerir (büyük dosya). Dalga 1b implementation'ı tamamlandığında, AI tarafından tamamlanan maddeleri **tek bir yerde** görmek ve insan reviewer'ın "ne kaldı?" sorusunu hızlıca cevaplamak için bu playbook oluşturuldu.

---

## 2. Dalga Bazlı Implementation Durumu

### 2.1 Dalga 1a — WebView2Backend Real Runtime (Tamamlandı ✅)

**PR-2 ile merge edildi (`feat/webview-webview2-runtime`).**

| Madde | Durum | Dosya / Konum |
|-------|-------|---------------|
| `WebView2Backend::create_window()` — gerçek `wry::WebView` handle | ✅ | `crates/viscos-webview/src/lib.rs` |
| `wry::WebViewBuilder::with_url("https://discord.com/app")` | ✅ | `crates/viscos-webview/src/lib.rs` |
| DevTools enabled in debug builds | ✅ | `crates/viscos-webview/src/lib.rs` (cfg!(debug_assertions)) |
| IPC handler scaffold (`with_ipc_handler`) | ✅ | `crates/viscos-webview/src/lib.rs` (Faz 1.6 1b+ routing) |
| `unsafe impl Send + Sync` for `WebView2Window` (main-thread affinity contract) | ✅ | `crates/viscos-webview/src/backend.rs` (safety comment zorunlu) |
| Parent directory auto-creation (SQLITE_CANTOPEN errno 14 fix) | ✅ | `crates/viscos-cache/src/lib.rs` |
| Unit + integration test | ✅ | `crates/viscos-webview/tests/` |

**Test kanıtı:** `cargo test --workspace` 405+ pass; release binary ~1.56 MB.

### 2.2 Dalga 1b — CEF Feature-Gated Stub (Tamamlandı ✅)

**PR-2 ile merge edildi.**

| Madde | Durum | Notlar |
|-------|-------|--------|
| `cef-backend` Cargo feature flag | ✅ | `crates/viscos-webview/Cargo.toml` (default off, opt-in) |
| `CefBackend::new()` feature off → Unimplemented stub | ✅ | `ViscosError::Unimplemented("phase-1.0 stub")` test-verified |
| `CefBackend::new()` feature on → DLL existence check + BrowserHost scaffold | ✅ | `crates/viscos-webview/src/cef.rs` |
| Cargo feature wiring (no-op release) | ✅ | CI `cargo build --release` (cef-backend off) ~1.56 MB |

**Kapsam dışı (release engineering):**
- ⚠️ **Gerçek `cef::BrowserHost::CreateBrowser` çağrısı** — CEF runtime 220-300 MB bundle gerektirir; insan PR'ı Faz 1.6 release engineering kapsamında. TODO marker `crates/viscos-webview/src/cef.rs:77`.
- ⚠️ **CEF subprocess routing (`cef::execute_process`)** — `main.rs`'de tek satır entegrasyon; insan PR.
- ⚠️ **V8 context bridge** — `window.ViscosNative` inject; Faz 4 SharedBuffer ile birlikte (post-PR-5).

### 2.3 Dalga 1c — CLI + Backend Detection (Tamamlandı ✅)

**PR-2 ile merge edildi.**

| Madde | Durum | Notlar |
|-------|-------|--------|
| CLI override (`--backend=webview2|cef|auto`) | ✅ | `crates/viscos-shell/src/main.rs` (clap) |
| Config override (`[webview].backend` in config.toml) | ✅ | `crates/viscos-config/src/lib.rs` |
| `select_default_backend()` orchestration | ✅ | `crates/viscos-shell/src/backend_selection.rs` |
| `is_rdp_session()` detection | ✅ | ADR-0012 §6 uyumlu — `GetSystemMetrics(SM_REMOTESESSION)` |
| `is_windows_11()` detection | ✅ | `GetVersionExW` build ≥22000 |
| CLI wins → config fallback → auto resolution | ✅ | Test: CLI wins, config fallback, auto resolution |
| RDP session → CEF force (ADR-0012 §6) | ✅ | RDP → CEF (default Win11 davranışıyla uyumlu) |

---

## 3. AI Scope (5/5 Tamamlandı ✅)

PR-2 kapsamında Faz 1.6 için AI tarafından teslim edilenler:

- [x] **CI matrix workflow** (`.github/workflows/size-gate.yml`) — WebView2 ≤30 MB + CEF ≤320 MB gate, PR comment ile diff.
- [x] **WiX fixture with BACKEND preprocessor** (`installer/viscos.wxs`) — `<?if $(env.BACKEND) = "Cef" ?>` conditional component inclusion.
- [x] **PowerShell build script with BACKEND param** (`installer/build-installer.ps1`) — `viscos-webview2.msi` veya `viscos-cef.msi` üretir.
- [x] **UpgradeCode GUID placeholder + `[guid]::NewGuid()` notu** (`installer/viscos.wxs`) — insan release engineering öncesi replace edecek.
- [x] **WinGet manifest template** (`installer/winget/manifests/w/Winterus20/Viscos/0.1.0/Winterus20.Viscos.installer.yaml`) — SHA256 placeholder, schema-compliant.

**Tüm bu maddeler** [`COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md`](../COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md) §3.3 Distribution ile uyumlu.

---

## 4. Human Scope (Release Engineering'e Bırakıldı)

Aşağıdaki maddeler **insan PR'ı** gerektirir (OV/EV code signing sertifikası + GitHub PAT + Microsoft Reviewer approval). AI tarafı bu maddeler için sadece **scaffold** bırakır:

- [ ] **Gerçek `cef::BrowserHost::CreateBrowser`** — CEF runtime bundle (220-300 MB) gerekli; insan release engineering workflow Faz 8.0'da tetiklenir.
- [ ] **V8 context bridge** (`window.ViscosNative`) — Faz 4 SharedBuffer implementasyonu ile birlikte.
- [ ] **Crashpad integration** — `minidumper::Minidumper::new(&config.dump_dir)`; CEF subprocess crash → minidump forward. Faz 8.0 scope.
- [ ] **24h soak test (Win11 + CEF)** — Leak yok (GDI peak <5000), restart 0. Manuel insan validation; mock telemetry **yetersiz** (gerçek hesap + gerçek mouse hover leak pattern).
- [ ] **OV/EV code signing sertifikası** ($200-1000/year) + `.pfx` → GitHub Actions secret (`WINDOWS_CERT_PFX_BASE64`).
- [ ] **WiX UpgradeCode GUID** — `[guid]::NewGuid()` (PowerShell) ile stable GUID, ASLA değişmemeli (major upgrade tracking).
- [ ] **WinGet manifest PR** (`microsoft/winget-pkgs`) + Microsoft Reviewer onayı (1-2 hafta).

**Gerekçe:** Mimari + trade-off kararları + token/secret handling **insan review** gerektirir (master index Bölüm 4 Hard Limit).

---

## 5. Definition of Done

### 5.1 Faz 1.6 Dalga 1b AI Scope (PR-2 ile tamamlandı)

- [x] `cef-backend` Cargo feature off → release binary ~1.56 MB (WebView2 only)
- [x] `cef-backend` Cargo feature on → DLL existence check + BrowserHost scaffold compiles
- [x] Win11 detection: `GetVersionExW` build ≥22000 → CEF default
- [x] Win10 detection: build <22000 → WebView2 default
- [x] RDP detection: `GetSystemMetrics(SM_REMOTESESSION) != 0` → CEF force
- [x] CLI `--backend=webview2|cef|auto` priority chain
- [x] Config `[webview].backend` fallback
- [x] `select_default_backend()` orchestration (telemetry optional input)
- [x] CI size gate (WebView2 ≤30 MB, CEF ≤320 MB)
- [x] WiX fixture BACKEND preprocessor
- [x] WinGet manifest template (SHA256 placeholder)

### 5.2 Faz 1.6 Human Scope (Release Engineering Deferred)

- [ ] Gerçek `cef::BrowserHost::CreateBrowser` (post PR-2, Faz 1.6 release)
- [ ] V8 context bridge (Faz 4)
- [ ] Crashpad (Faz 8.0)
- [ ] 24h soak Win11+CEF (manuel)
- [ ] Code signing sertifikası (Faz 8.0)
- [ ] WinGet PR + Microsoft review (Faz 8.0)

**Toplam:** 5/5 AI items ✅; 6/6 human items ⏸ release engineering deferred.

---

## 6. Out of Scope (PR-2 ve Dalga 1b kapsamı dışı)

Aşağıdaki maddeler **bilinçli olarak bu playbook'ta YOK**:

- **Multi-CEF-version build** — Tek CEF tag (`cef-v148.3.0+148.0.9`); Faz 8.5'te version matrix eklenirse değerlendirilir.
- **Linux CEF production** — Viscos v1 Windows-only; WebKitGTK Faz 2.0+ backlog.
- **CEF proxy config UI** — `chromium_flags.toml` manuel edit; Faz 5 iced UI sonrası otomatik.
- **CEF self-update** — Faz 8.5 `CefUpdater` scope; Chromium advisory feed parse.
- **Real Discord account testing** — ToS grey-zone + risk; insan validation Faz 2.0'da.

---

## 7. Test Stratejisi (PR-2 Dalga 1b)

| Test | Tip | Kabul |
|------|-----|-------|
| BackendKind detection | Unit | Win10/11 doğru |
| RDP detection | Unit (Windows) | RDP session → CEF force |
| CEF DLL check (feature on) | Unit | libcef.dll yok → Unimplemented |
| CLI parsing (`--backend=auto`) | Unit | Default argüman auto |
| Config override (`[webview].backend`) | Unit | CLI wins, config fallback |
| select_default_backend orchestration | Unit | 3 katman doğru |
| WebView2 real create_window | Integration (Windows) | wry::WebView handle valid |
| Size gate (WebView2) | CI | ≤30 MB |
| Size gate (CEF matrix) | CI | ≤320 MB |

---

## 8. Bilinen Riskler ve Kabul Edilen Trade-off'lar

| Risk | Etki | Azaltma |
|------|------|---------|
| CEF 220-300 MB binary | Disk alanı | Feature-gated; default WebView2 ~1.56 MB |
| WebView2 GDI leak (Win11) | Restart spam | Faz 1.6 default CEF (Win11) + watchdog (Win10) |
| RDP session GDI leak (Win11) | Restart spam | ADR-0012 §6 — RDP → CEF force |
| `cef::BrowserHost::CreateBrowser` scaffold | Browser açılmaz (feature on) | Release engineering Faz 8.0'da gerçek call |
| V8 bridge yok | JS → Rust state transfer suboptimal | Pull-based IPC (ADR-0012 §3) workaround |
| Real account test eksik | Edge case'ler yakalanmamış | Faz 2.0 1b sonrası insan validation |

---

## 9. Sonraki Adımlar

### 9.1 Hemen (Bu PR-6 sonrası)

- **Faz 2.0 1b (Auth):** `login_email()` / `mfa_submit()` — PR-7+ kapsamında (insan PR, ToS-uyumlu hesap test gerekli).
- **Faz 1.5 Polish:** Telemetry-driven CEF recommendation tetikleyici doğrulama (mock telemetry).

### 9.2 Release Engineering (Faz 8.0, İnsan PR'ları)

- WiX UpgradeCode GUID generation (`[guid]::NewGuid()`).
- OV/EV code signing sertifikası satın al.
- `.pfx` → GitHub Actions secret (`WINDOWS_CERT_PFX_BASE64`).
- Test build sign + verify.
- WinGet manifest PR (`microsoft/winget-pkgs`).
- Microsoft Reviewer onayı (1-2 hafta).
- 24h soak Win11+CEF + Win10+WebView2.

### 9.3 Faz 4 (Post-MVP)

- V8 context bridge (`window.ViscosNative`) + SharedBuffer transfer.
- `davey` optional dependency aktif (Faz 7 DAVE E2EE için API surface sabitleme).

---

## 10. Çapraz Referanslar

- **ADR-0010 (Cache):** Content-addressable CDN cache key + adaptive tier sizing — `crates/viscos-cache/src/lib.rs` (PR-3 ile).
- **ADR-0011 (Auth):** Keyring-core + DPAPI encryption — `crates/viscos-auth/src/lib.rs` (mevcut main'de, PR-4 cross-ref).
- **ADR-0012 (Frontend):** Hibrit mimari + RDP detection + bridge.ts resilience — `crates/viscos-shell/src/backend_selection.rs` (PR-2 ile).
- **Master index:** [`viscos_index.md`](./viscos_index.md) Bölüm 6 — hedef metrikler (binary bütçesi, RAM, cold start).
- **Audit:** [`COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md`](../COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md) — Dalga 1b status + Faz 1.6 §7.
- **CI workflows:** [`.github/workflows/ci.yml`](../workflows/ci.yml) + [`.github/workflows/size-gate.yml`](../workflows/size-gate.yml) + [`.github/workflows/release.yml`](../workflows/release.yml) (son ikisi PR-6 ile).

---

**Playbook prepared:** 2026-06-19 (PR-6 Dalga 1b closing)
**Compliance:** `.cursorrules` Bölüm 1-15 (Rust 1.89 ed.2024, ADR-0006–0012, master index)
**Next action:** PR-6 merge → Faz 2.0 1b (Auth, PR-7+) + Faz 8.0 (Release Engineering, human PR'lar)
