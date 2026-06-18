# Implementation Packet — ADR-0012: Frontend Mimari — Hibrit (WebView + Native Shell)

## Header

- **ADR:** ADR-0012
- **Başlık:** Frontend Mimari — Hibrit (WebView + Native Shell) (Haziran 2026 Trade-off Revizyonu)
- **Durum:** 🟡 Proposed (insan onayı bekliyor)
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0012](../../docs/DECISIONS.md#adr-0012-frontend-mimari--hibrit-webview--native-shell-haziran-2026-trade-off-revizyonu)
- **Önceki plan:** [`viscos_index.md` Bölüm 1 + 6](../../.cursor/plans/viscos_index.md) (hibrit niyeti, somut trade-off yok)
- **Araştırma:** [`docs/CEF-VS-WEBVIEW2.md`](../../docs/CEF-VS-WEBVIEW2.md), [`bridge-resilience-research.md`](../../.cursor/plans/bridge-resilience-research.md)

## Hedef faz worker

**Bu packet üç worker'a bölünür (cross-cutting ADR):**

- **Shell+Webview worker, Faz 1.0, Dalga 1:** Mimari karar (hibrit) + `tao + iced 0.14 + wry/CEF` kurulumu + WebViewBackend trait. `bridge.ts` selector resilience rules.
- **Telemetry worker, Faz 1.5, Dalga 2:** 24 saat shadow mode (yeni login için) + fingerprint parite check (aylık GitHub Action).
- **CEF-rollout worker, Faz 1.6, Dalga 1:** Win11 CEF default, Win10 WebView2 default. **MVP'nin parçası.**
- **DistributionUI worker, Faz 8.5, Dalga 1:** CEF default-out yönetim (UI + Chromium flags + self-update wizard).

**Öncelik sırası:** Faz 1.0 (Faz 1.6'ya temel) → Faz 1.5 (telemetry + shadow mode) → Faz 1.6 (CEF default) → Faz 8.5 (CEF management).

## Uygulama adımları

### Faz 1.0 Dalga 1 (Shell+Webview worker)

1. **`crates/viscos-webview/`** workspace'e ekle — `WebViewBackend` trait:
   ```rust
   pub trait WebViewBackend: Send + Sync {
       fn create_window(&self, config: WindowConfig) -> Result<Box<dyn WebViewWindow>, ViscosError>;
       fn name(&self) -> &'static str;
   }

   pub enum BackendKind { WebView2, Cef }
   pub fn select_default_backend() -> BackendKind {
       // Win11 → CEF, Win10 → WebView2 (Faz 1.6'da aktif olur)
       if cfg!(target_os = "windows") && is_windows_11() {
           BackendKind::Cef
       } else {
           BackendKind::WebView2
       }
   }
   ```

2. **`crates/viscos-shell/`** workspace'e ekle:
   - `Cargo.toml`: `tao 0.35`, `iced 0.14` (reactive rendering), `wry 0.55` (default WebView2 backend), `tokio`.
   - `src/lib.rs`: side panel, tray, hotkey, autocomplete (Faz 6'da).
   - `src/window.rs`: `tao` window + tray icon.

3. **iced 0.14 spike (1 hafta, Faz 1.0 ilk hafta)**:
   - Native side panel + WebView Discord UI aynı pencere içinde.
   - IPC + frame timing ölçümü (native frame drop <%1 mi?).
   - Resize davranışı (COSMIC resize lag `pop-os/libcosmic#753` çözülmüş mü?).
   - **Spike olumsuzsa:** iced 0.14 → 0.13 downgrade veya `egui` değerlendirmesi.

4. **`frontend/src/bridge.ts`** — selector resilience rules (ADR-0012 §2):
   - ✅ DOĞRU: `document.querySelector('[id^="message-content-"]')`, `document.querySelector('[aria-label*="channel"]')`.
   - ❌ YANLIŞ: `document.querySelector('.message__5126c')` (Discord her deploy'da değiştirir).
   - ✅ DOĞRU: `viscos.webpack.findByProps('getCurrentUser')` (Vencord pattern'i).
   - ✅ DOĞRU: `viscos.invoke({ type: 'GetUnreadCount', data: ... })` (pull-based bridge).
   - ❌ YANLIŞ: `MutationObserver` ile heuristic unread count tahmini.

5. **`crates/viscos-webview/BRIDGE-RESILIENCE.md`** dokümanı (Faz 1.0 deliverable) — bu kuralları referans al.

6. **Test:**
   - `tests/webview_backend_trait.rs`: Her backend için compile-time test.
   - `tests/select_default_backend.rs`: Win10 vs Win11 doğru backend seçiyor.
   - `bridge.ts` ESLint rule: hash'li class name selector yasak (custom rule).

### Faz 1.5 Dalga 2 (Telemetry worker)

7. **24 saat shadow mode** (`crates/viscos-auth/src/super_properties.rs` + `crates/viscos-shell/src/first_run.rs`):
   ```rust
   pub struct ShadowMode {
       login_at: SystemTime,
       expires_at: SystemTime,  // +24h
   }

   impl ShadowMode {
       pub fn is_active(&self) -> bool {
           SystemTime::now() < self.expires_at
       }
       pub fn allows_write(&self) -> bool {
           !self.is_active()  // yazma 24 saat sonra aktif
       }
   }
   ```
   - Kullanıcıya modal: "Hesabınız yeni, ilk 24 saat ısınma süresi."
   - Opt-out: `Settings → Advanced → Shadow mode atla`.

8. **Fingerprint parite check (aylık GitHub Action)**:
   - `.github/workflows/fingerprint-parity.yml`: her ayda 1 kez, kendi Viscos instance'ından alınan X-Super-Properties ile aynı tarihli resmi Discord stable client'ınkinden alınan karşılaştırılır.
   - Sapma >%5 → uyarı PR'ı.

9. **Test:**
   - `tests/shadow_mode.rs`: 24h boundary doğru davranıyor.
   - `tests/fingerprint_parite_action.yml` (workflow): mock Discord response ile >%5 sapma algılanıyor.

### Faz 1.6 Dalga 1 (CEF-rollout worker)

10. **`crates/viscos-webview/src/cef.rs`** — `cef-rs` backend implementasyonu:
    - `Cargo.toml`: `cef-rs = { version = "0.x" }` (Faz 1.6 plan'ında versiyon).
    - `WebViewBackend` trait impl.
    - `select_default_backend()` Win11'de `BackendKind::Cef` döner.

11. **`crates/viscos-webview/src/webview2.rs`** — `wry` backend implementasyonu (Win10 default).

12. **Telemetry override** (`select_default_backend` Faz 1.6):
    - `viscos-telemetry` Faz 1.5 backend'inden CEF recommendation alır → kullanıcıya "CEF'e geç" önerisi.

13. **Test:**
    - `tests/cef_default_win11.rs`: Win11 → CEF, Win10 → WebView2.
    - 24h soak Win11 + CEF: 0 restart (GDI leak yok).
    - 24h soak Win10 + WebView2: <5 restart, gap <2s.

### Faz 8.5 Dalga 1 (DistributionUI worker)

14. **CEF default-out wizard** (`crates/viscos-shell/src/settings/cef.rs`):
    - `Settings → Advanced → WebView Backend` UI.
    - "Use CEF (recommended for Win11)" / "Use WebView2 (lighter, opt-out)" toggle.
    - Self-update: CEF binary güncellemesi (Chromium security patches).

15. **Chromium flags UI**:
    - `--disable-gpu` (kullanıcı isteğe bağlı), `--force-device-scale-factor`, vb.
    - Validation: bilinen incompatible flag'ler (`--no-sandbox` prod'da yasak).

16. **Test:**
    - `tests/cef_wizard.rs`: toggle doğru backend'i seçiyor.
    - `tests/chromium_flags.rs`: bilinen flag'ler validate ediliyor.

## Kabul kriterleri

- ✅ `viscos-shell` + `viscos-webview` crate'leri workspace member.
- ✅ `tao 0.35` + `iced 0.14` + `wry 0.55` declare.
- ✅ `WebViewBackend` trait mevcut, 2 implementasyon (WebView2 + CEF).
- ✅ `select_default_backend()` Win11'de CEF, Win10'da WebView2.
- ✅ `frontend/src/bridge.ts` selector resilience rules yayınlandı.
- ✅ `BRIDGE-RESILIENCE.md` dokümanı mevcut.
- ✅ 24 saat shadow mode implementasyonu + modal.
- ✅ Aylık fingerprint parite check GitHub Action.
- ✅ Faz 1.6'da CEF default Win11 için aktif.
- ✅ Faz 8.5'te CEF default-out wizard UI.
- ✅ `davey` (DAVE E2EE, optional dep) `crates/viscos-auth/Cargo.toml`'da `dave` feature'ı altında.

## Test stratejisi

- **Unit:**
  - `tests/webview_backend_trait.rs`: trait compile-time.
  - `tests/shadow_mode.rs`: 24h boundary.
- **Integration:**
  - Win10 + WebView2: 24h soak, <5 restart.
  - Win11 + CEF: 24h soak, 0 restart.
  - DOM churn simülasyonu: bridge.ts selector'lar yeni Discord deploy'unda kırılmıyor (mock DOM).
- **Manuel (Faz 1.0 sonu):**
  - iced 0.14 spike raporu: native frame drop <%1 mi?
  - Bridge resilience kuralları PR review checklist'e eklendi.
  - Discord test hesabı ile: side panel (native) + Discord UI (WebView) aynı pencere.
- **Manuel (Faz 1.5 sonu):**
  - Yeni login → 24 saat shadow mode aktif, mesaj gönderme disabled.
  - Aylık fingerprint parite check çalıştı (manual trigger).
- **Manuel (Faz 1.6 sonu):**
  - Win11 CEF default: GDI objesi <5000 (24h).
  - Win10 WebView2 default: GDI 7000'de restart.
- **Manuel (Faz 8.5 sonu):**
  - CEF wizard UI: toggle çalışıyor, self-update indiriliyor.

## Sınır durumları ve riskler

- **iced 0.14 production kanıtı az:** 1.0 freeze öncesi, Halloy/Snifflet/Neothisia production ama Discord client + WebView overlay senaryosu yok. Mitigation: 1 haftalık spike Faz 1.0 ilk haftasında.
- **24h shadow mode UX friction:** Yeni kullanıcı ilk gün "neden mesaj gönderemiyorum?" diyebilir. Mitigation: modal net metin + opt-out (Settings → Advanced).
- **Bridge.ts selector resilience:** AI-PR review yükü. Her bridge PR'ında "aria-label > class" kontrolü. Mitigation: ESLint custom rule + PR review checklist.
- **`davey` 0.1.x major API drift olabilir:** Optional dependency, hiçbir runtime path'i yok → risk sıfır. `cargo update` haftalık.
- **Fingerprint parite check her ayda 1 GH Action:** 5 dakika, 1 Windows runner dakikası. ADR-0004 CI bütçesine sığar.
- **Discord UI büyük revamp (yılda 1+):** Mart 2025 gibi. Bridge.ts resilience rules yeterli mi kontrol edilir.
- **Discord yeni medya formatı:** WebP → AV2. Hybrid otomatik kaplar (Discord kendisi render ediyor). Native kırılırdı.
- **CEF binary büyüklüğü:** ~240 MB (Faz 1.6'da) → Faz 8.5'te self-update gerekir.

## Review trigger'ları

- Discord UI yeni büyük revamp.
- Discord yeni medya formatı (WebP → AV2).
- Vencord/Equicord Discord yeni versiyonuna geç hazır değilse.
- Discord DAVE "sadece browser" olmaktan çıkıp "native MLS" gerektirirse → `davey` aktif dependency olur.
- Dissent/kind/Acheron production-grade olursa (stickler + voice + animated emoji + permission engine).
- Microsoft WebView2 upstream #5536 çözülürse → WebView2 default'a geri dönüş değerlendirilir.
- CEF self-update sıkıntısı büyürse → Win11 default WebView2'ye çekilebilir.
- Servo 1.0 + Discord mesaj yazma + WebRTC encoded transform → v3 plan güncellenir.

## Cross-references

- **ADR:** ADR-0001 (workspace), ADR-0005 (binary), ADR-0006 (MSRV), ADR-0011 (fingerprint WebGL hash, shadow mode), ADR-0010 (binary bütçesi).
- **Plan:**
  - [`phase-1.0-window-webview.md`](../../.cursor/plans/phase-1.0-window-webview.md) (Faz 1.0)
  - [`phase-1.5-telemetry-and-restart-optimization.md`](../../.cursor/plans/phase-1.5-telemetry-and-restart-optimization.md) (Faz 1.5)
  - [`phase-1.6-cef-default-rollout.md`](../../.cursor/plans/phase-1.6-cef-default-rollout.md) (Faz 1.6)
  - [`phase-8.5-cef-backend.md`](../../.cursor/plans/phase-8.5-cef-backend.md) (Faz 8.5)
- **Araştırma:** [`docs/CEF-VS-WEBVIEW2.md`](../../docs/CEF-VS-WEBVIEW2.md), [`bridge-resilience-research.md`](../../.cursor/plans/bridge-resilience-research.md), [`webview2-hardening.md`](../../.cursor/plans/webview2-hardening.md).
- **Master index:** [`viscos_index.md` Bölüm 1 + 6 + 7](../../.cursor/plans/viscos_index.md).
- **Alternatifler:** Tam native (kind/Acheron), saf Electron, Tauri, Servo, cxx-qt, sıfırdan custom — hepsi elendi (ADR-0012 Consequences).
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Evet — birden fazla noktada:**
1. **Faz 1.0 Dalga 1 başlangıcı:** Mimari karar (hibrit) + `WebViewBackend` trait shape + iced 0.14 spike sonucu.
2. **Faz 1.5 Dalga 2:** 24 saat shadow mode UX kararı (kullanıcı transparanlığı) + fingerprint parite check hassasiyeti.
3. **Faz 1.6 Dalga 1:** CEF default Win11 kararı (ADR-0012 + Faz 1.6 plan zaten onaylı, MVP parçası).
4. **Faz 8.5 Dalga 1:** CEF default-out wizard UX (kullanıcıya geçiş sunumu).

ADR-0012 henüz **Proposed** durumda. Tüm uygulama başlamadan önce insan onayı beklenmeli. Özellikle **5 ek (bridge resilience, shadow mode, davey, iced 0.14 spike, CEF parite check)** bağımsız review gerektirir.
