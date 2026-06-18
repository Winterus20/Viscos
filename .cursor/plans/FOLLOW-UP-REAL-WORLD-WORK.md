# Viscos MVP v0.1.0 — Kalan Gerçek Dünya İşleri (Post-AI-Implementation)

**Tarih:** 2026-06-18
**Durum:** AI worker'lar 10 fazı (0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 8.5) implement etti. 4 commit GitHub'a push edildi (`fcefd9a..7e94d98`). 334 test geçti, 0 warning, release binary 1.56 MB. Aşağıdaki işler insan dokunuşu + gerçek ortam testi gerektiriyor.

---

## A. Faz 1.5 — Telemetry + Shadow Mode + Fingerprint Parity

**Packet:** `.cursor/packets/packet-0012-frontend-hybrid.md` § Faz 1.5 Dalga 2
**Plan:** `.cursor/plans/phase-1.5-telemetry-and-restart-optimization.md`
**AI durumu:** Shadow mode + 24h altyapı `crates/viscos-auth/src/shadow_mode.rs`'te (4 test). Fingerprint WebGL hash stub (`crates/viscos-auth/src/super_properties.rs` → "stub-pending-backend-decision"). Telemetry backend **YOK** (sadece Cache `TelemetryStats` struct var).

### Yapılacaklar

1. **Yeni crate: `viscos-telemetry`** (Faz 1.5 PRD §3):
   - `crates/viscos-telemetry/Cargo.toml` + `src/lib.rs`
   - SQLite store (WAL mode), 30-day retention, 100MB cap
   - GDI sample collector (her 30s, watchdog'dan besleme)
   - Restart event recorder (watchdog `RestartSignal`'den)
   - API: `telemetry.record_gdi_sample(count: u32)`, `telemetry.record_restart(reason)`, `telemetry.recommend_cef() -> CefRecommendation`
   - CEF recommendation logic: `restarts_24h >= 5` OR `peak_gdi_7d >= 8500` → "CEF önerilir" / "CEF zorunlu"
   - Test: SQLite round-trip, retention cap eviction, recommendation thresholds

2. **Fingerprint WebGL hash** (`crates/viscos-auth/src/super_properties.rs`):
   - Win11 CEF → `cef-rs` WebGL renderer'dan gerçek hash al
   - Win10 WebView2 → `wry` WebGL renderer'dan gerçek hash al
   - Varsa: `taffy` + `gl-rs` veya doğrudan WebView2 `ICoreWebView2Frame` API
   - **Faz 1.6 ile koordineli** — CEF/WebView2 backend seçimi önce yapılmalı

3. **Haftalık GitHub Action** — `.github/workflows/build-number-sync.yml`:
   - cron: `0 0 * * 0` (her Pazar 00:00 UTC)
   - `discord.com/app` JS bundle'ından `client_build_number` parse
   - PR otomatik aç: `chore: bump build_number to XXXXXX`
   - `super_properties::build_number` config'ten çekilir, PR merge olunca otomatik güncellenir

4. **Aylık Fingerprint Parity Action** — `.github/workflows/fingerprint-parity.yml`:
   - cron: `0 0 1 * *` (her ayın 1'i 00:00 UTC)
   - Kendi Viscos instance'ından alınan X-Super-Properties ile aynı tarihli resmi Discord stable client'ınkinden alınan karşılaştırılır
   - Sapma >%5 → uyarı PR'ı

5. **Tray icon badge** (`crates/viscos-shell/src/window.rs` + `viscos-shell/src/integration/tray.rs`):
   - "X restart today" rozeti
   - `tao 0.35`'te `tray` feature'ı kaldırıldı → `tray-icon 0.19` crate'i ekle
   - Faz 1.6'da shell loop'una subscribe et

---

## B. Faz 1.6 — CEF Default Rollout (Win11)

**Packet:** `.cursor/packets/packet-0012-frontend-hybrid.md` § Faz 1.6 Dalga 1
**Plan:** `.cursor/plans/phase-1.6-cef-default-rollout.md`
**AI durumu:** `CefBackend::new()` stub (`crates/viscos-webview/src/cef.rs` → `Err(Unimplemented)`). `select_default_backend()` Win11'de `BackendKind::Cef` dönüyor ama implementasyon yok. `BRIDGE-RESILIENCE.md` yazıldı.

### Yapılacaklar

1. **Gerçek `cef-rs` entegrasyonu**:
   - `Cargo.toml` workspace dep: `cef-rs = { version = "0.x", default-features = false }` (tauri-apps/cef-rs tag `cef-v148.3.0+148.0.9`)
   - `crates/viscos-webview/src/cef.rs` tam implementasyon:
     - `CefBackend::new()` + `CefWindow::new()`
     - `WebViewBackend` trait impl: `create_window()` → `wry 0.55`'in `WebViewBuilder::new().with_url(DISCORD_APP_URL).build()` veya doğrudan `cef-rs` `BrowserView`
     - CEF subprocess main: `cef::execute_process(...)`
   - Binary bütçe: CEF ~30-40 MB ekler. **25 MB gate'i aşılacak** → ADR-0012 § release profile güncelleme: `lto = "fat"` + `codegen-units = 1` (zaten var) + opsiyonel `wasm-opt` link-time.

2. **RDP auto-detection** (`crates/viscos-webview/src/lib.rs::select_default_backend`):
   - `GetSystemMetrics(SM_REMOTESESSION)` → non-zero → `BackendKind::Cef` zorla
   - Veya `GetUserDefaultLangID`/`WTSGetActiveConsoleSessionId` ile

3. **Dual MSI build** (`installer/`):
   - `installer/viscos-webview2.wxs` (Win10 default, ~25 MB)
   - `installer/viscos-cef.wxs` (Win11 default, ~300 MB)
   - GitHub Actions matrix build: 2 ayrı job, 2 ayrı artifact

4. **Minimal CLI flag** (`crates/viscos/src/main.rs`):
   - `viscos --backend=webview2` veya `--backend=cef` veya `--backend=auto`
   - Config.toml'a override yazma

5. **`crashpad` entegrasyonu** (Faz 8.0'da stub):
   - CEF subprocess crash → minidump oluştur
   - minidumper'a forward et (Faz 8.0 stub'ı ile entegre)

6. **24h soak test (manuel):**
   - Win11 + CEF: 0 restart bekleniyor (GDI leak yok)
   - Win10 + WebView2: <5 restart, gap <2s bekleniyor
   - RDP session + CEF: 0 leak doğrula

---

## C. Faz 7.0 — Voice/Video (Opsiyonel, v1'de skip)

**Plan:** `.cursor/plans/phase-7.0-voice-video.md`
**AI durumu:** Hiç dokunulmadı. v1 için sadece **basic audio control** (mic mute/deafen) global hotkey'lerle tetiklensin.

### Yapılacaklar (v1 minimum)

1. **Windows WASAPI mute toggle** (`crates/viscos-shell/src/integration/audio.rs`):
   - `windows 0.58` crate ile `IAudioEndpointVolume::SetMute(bTrue, NULL)`
   - Default mic + default speaker toggle
   - `pub struct AudioController` + `pub fn toggle_mute(&self) -> Result<bool, ViscosError>`
   - Hotkey `Ctrl+Shift+M` (zaten Faz 6.0'da kayıtlı) → bunu tetiklesin

2. **Deafen = mic mute + speaker mute**:
   - `pub fn toggle_deafen(&self) -> Result<(), ViscosError>` → mic + speaker birlikte

### v2.0 (Faz 7.0 tam)

- `davey` crate (DAVE E2EE, Rust MLS) — opsiyonel dependency
- `WebRTC` CEF/WebView2 üzerinden (zaten Discord web app yapıyor, native tarafta sadece mute/deafen override)
- `Opus` codec native (Faz 7.5+)

---

## D. Network-Requires Test'ler (CI'da disable)

**AI durumu:** `crates/viscos-distribution/tests/{updater_check.rs.disabled, cef_update.rs.disabled}` — 2 test reqwest/rustls Windows elevation (os error 740) tetikliyor.

### Yapılacaklar

1. **Self-hosted runner'da çalıştır**:
   - `cargo test -p viscos-distribution --test updater_check -- --ignored`
   - `cargo test -p viscos-distribution --test cef_update -- --ignored`
   - Veya `RUST_TEST_NOCAPTURE=1 cargo test --workspace -- --ignored` (CI nightly job)

2. **Mock HTTP server (mockito/wiremock) ile refactor**:
   - `mockito::Server::new_async()` ile `http://localhost:PORT/...` mock'la
   - `Updater::check()` gerçek GitHub API yerine configurable endpoint
   - Bu sayede CI'da elevation sorunu olmadan test geçer

3. **GitHub Actions nightly workflow** (`.github/workflows/nightly-network-tests.yml`):
   - cron: `0 3 * * *` (her gece 03:00 UTC)
   - `cargo test --workspace -- --ignored`
   - Self-hosted runner VEYA trusted CI provider

---

## E. Release Engineering (Gerçek Dağıtım)

**AI durumu:** Tüm template/stub yerinde. Hiçbiri aktive değil.

### Yapılacaklar

1. **WiX `UpgradeCode` GUID generation**:
   - `installer/viscos.wxs` ve `installer/viscos-webview2.wxs` ve `installer/viscos-cef.wxs` (sonraki faz)
   - `uuidgen` veya PowerShell `[guid]::NewGuid()`
   - **ASLA sonradan değiştirme** (major upgrade tracking bu GUID üzerinden)

2. **Code signing sertifikası**:
   - Self-signed → OV → EV (geçiş yolu)
   - `.pfx` dosyası + parola `VISCOS_CERT_PASSWORD` env var'da
   - GitHub Actions secret: `WINDOWS_CERT_PFX_BASE64` (base64 encoded)
   - `signtool.exe sign /tr http://timestamp.digicert.com /fd sha256 /td sha256 /f viscos.pfx /p $env:VISCOS_CERT_PASSWORD target/release/viscos.exe`

3. **WinGet submission**:
   - `microsoft/winget-pkgs` repo'ya PR
   - SHA256 placeholder'ı doldur: `Get-FileHash viscos.msi -Algorithm SHA256`
   - Microsoft Reviewer onayı (1-2 hafta)

4. **GitHub Release otomasyonu** (`.github/workflows/release.yml`):
   - tag `v*` push → 2 MSI build (WebView2 + CEF) + signtool + SHA256 üret + WinGet manifest auto-update PR

5. **Auto-updater ilk yayın** (Faz 8.0):
   - `viscos-distribution::updater::Updater::check()` gerçek GitHub API call (`self_update::backends::github::ReleaseList`)
   - Version comparison semver ile
   - Download + verify SHA256 + restart-replace

6. **First-run ToS modal** (`crates/viscos-shell/src/native/disclaimer.rs`):
   - 4-canonical ToS string'i (zaten yazıldı)
   - iced modal window, "Kabul Ediyorum" butonu zorunlu
   - Keyring'de flag sakla (kullanıcı kabul etti mi kontrol et)

---

## F. Faz 1.6 + Faz 8.0 Cross-Cutting: Gerçek WebView Yükleme

**AI durumu:** Tüm backend stub (`Err(Unimplemented)` dönüyor). Discord web app hiç yüklenmedi.

### Yapılacaklar

1. **`wry 0.55` WebView2 backend** (`crates/viscos-webview/src/webview2.rs`):
   - `WebViewBuilder::new().with_url("https://discord.com/app").build()`
   - `with_devtools(true)` (geliştirme)
   - `with_initialization_script(BRIDGE_JS)` — bridge.ts'i inject et
   - `post_shared_buffer` implementasyonu (Faz 4.0 + IPC SharedBuffer):
     - `webview2-com` crate ile `ICoreWebView2Environment::CreateSharedBuffer`
     - Veya fallback: `post_message` + base64 (küçük blob'lar için)
   - `frontend/src/bridge.ts` `getBinary<T>()` API'si gerçekten çağrılabilir olmalı

2. **`cef-rs` CEF backend** (`crates/viscos-webview/src/cef.rs`):
   - `cef::BrowserHost::CreateBrowser(sync_url="https://discord.com/app")`
   - CEF subprocess main function: `cef::execute_process(args)`
   - V8 context bridge: `ViscosNative` API'sini CEF window object'ine inject

3. **Frontend `dist/preload.js`** → WebView'in görebileceği `window.viscos` obj:
   - `viscos.invoke({type, data})` → Rust IPC handler'a post
   - `viscos.on(event, handler)` → Rust event'leri (UnreadCountChanged, ThemeChanged, etc.) subscribe

4. **Selenium-style end-to-end test (manuel):**
   - Gerçek Discord hesabı ile ToS-uyumlu login
   - MFA TOTP kodu gir
   - Shadow mode aktifken `create_message` bloklanmalı (24h)
   - 24h sonra yazma aktif
   - Sunucu listesinde gezin → message cache hit (moka)
   - Attachment indir → foyer disk cache hit

---

## G. Faz 8.5 — CEF Management UI (Tam İmplementasyon)

**AI durumu:** `CefManager`, `ChromiumFlags`, `CefUpdater` API'leri var ama sadece config okuma. UI entegrasyonu **YOK**.

### Yapılacaklar

1. **`iced` UI panel** (`crates/viscos-shell/src/native/cef_settings.rs`):
   - Settings → Advanced → WebView Backend: radio (WebView2, CEF, Auto)
   - Chromium flags: text input (comma-separated), canlı preview
   - CEF cache strategy: dropdown (aggressive, balanced, conservative)
   - "CEF güncellemelerini kontrol et" buton → `CefUpdater::check()`
   - "Şimdi güncelle" buton → `CefUpdater::apply()`

2. **CEF self-update feed entegrasyonu**:
   - `https://cef-builds.spotifycdn.com/index.json` parse
   - Version compare + CVE severity check (NVD feed optional)
   - Auto-update on critical CVE (config opt-in)

---

## H. Voice/Video (Faz 7.0) — v1'de Skip

Yukarıda Bölüm C'de basic audio control (mute/deafen) zaten listelendi. Tam voice/video Faz 7.0 v2.0'a ertelendi.

---

## I. Disk Şifreleme Key Yönetimi (Multi-device Sync)

**AI durumu:** `MediaKey::from_keyring()` Windows DPAPI'ye bağlı. **Cihaz-özgü** — başka bilgisayarda cache okunamaz.

### Yapılacaklar (v2.0)

- Opsiyonel Argon2id passphrase → age envelope encryption (ADR-0011 Varyant B)
- Modal: "Cache passphrase belirle (opsiyonel, multi-device sync için)"
- Keyring'de hem DPAPI-bound hem passphrase-bound key sakla
- `keyring::Entry::new("Viscos", "cache_key_variant")` ile variant seç

---

## J. Frontend TypeCheck Cleanup

**AI durumu:** NativeUI+Hotkeys worker `frontend/tests/bridge.test.ts` + `frontend/tests/eslint-rule.test.ts`'de **pre-existing typecheck hataları** raporladı. Bunlar önceki worker'lardan kaldı.

### Yapılacaklar

1. `cd frontend && npm run typecheck` çalıştır
2. Hataları tek tek düzelt (veya `// @ts-expect-error` ekle)
3. `frontend/package.json` scripts'e `"typecheck": "tsc --noEmit"` ekle (zaten var)
4. `frontend/package.json` scripts'e `"prebuild": "npm run typecheck"` ekle (CI gate)

---

## K. AI-Validation CI İlk Çalıştırma

**AI durumu:** `.github/workflows/ai-task-validate.yml` yazıldı ama hiç çalıştırılmadı.

### Yapılacaklar

1. **İlk PR'ı bu repo'da aç** (kendi kendine test için):
   - Branch: `test/ci-validation`
   - Trivial değişiklik: `docs/AI-WORKFLOW.md` typo fix
   - PR aç → `AI-Generated: yes` label ekle (manuel veya `.github/labeler.yml` rule)
   - `ai-task-validate.yml` çalışsın
   - Eğer fail ederse: dosya boyutu, unwrap scan, eval_script payload size limit'lerini ayarla

2. **Branch protection** (`Settings → Branches → main → Require status checks`):
   - `ci / fmt`, `ci / clippy`, `ci / test`, `ci / build`, `ci / size-gate`, `ai-validation / validate` required
   - Dismiss stale PR approvals on new push

---

## L. Discord Test Hesabı Kuralları (ToS Compliance)

**Zorunlu** (Tüm manuel testler için):

- Sadece **kendi kişisel hesabınız** ile test
- **ToS-uyumlu davranış**: otomasyon, mass DM, scraping YAPMA
- Sadece login, MFA, message okuma/yazma, attachment indirme, voice mute toggle
- Multi-account testing: 2 ayrı test hesabı ile (gerçek kişisel, ToS-riskli davranış test etmeyin)
- **Yasak**: rate-limit'i zorla, self-bot davranışı, CAPTCHA bypass denemeleri

---

## Öncelik Sırası (İnsan Review İçin)

| # | Faz | Süre | Kritik mi? |
|---|---|---|---|
| 1 | Faz 1.6 (gerçek CEF) | 2-3 hafta | ✅ MVP blocker |
| 2 | Faz 8.0 release engineering (signing, WinGet) | 1-2 hafta | ✅ Dağıtım blocker |
| 3 | Faz 1.5 telemetry | 1 hafta | ⏳ Tavsiye |
| 4 | Faz 8.5 CEF UI | 1 hafta | ⏳ Geliştirici deneyimi |
| 5 | Faz 1.6 RDP detection | 3 gün | ⏳ Edge case |
| 6 | Faz 7.0 basic audio | 2-3 gün | ⏳ UX polish |
| 7 | Faz 1.5 fingerprint parity | 1 hafta | ⏳ Anti-bot |
| 8 | J. Frontend typecheck cleanup | 1 gün | ⏳ Hygiene |
| 9 | K. CI validation first run | 1 gün | ⏳ Hygiene |
| 10 | D. Network test refactor (mock) | 2 gün | ⏳ Test kalitesi |
| 11 | I. Passphrase encryption (v2.0) | 1 hafta | ❌ Sonra |

---

## Bu dosya nereye yazıldı

`.cursor/plans/FOLLOW-UP-REAL-WORLD-WORK.md` — AI worker'lar implementasyonu bitirdi, bu dosya insan tarafından takip edilecek post-implementation checklist.

Son güncelleme: 2026-06-18, Viscos MVP v0.1.0 (`7e94d98`)
