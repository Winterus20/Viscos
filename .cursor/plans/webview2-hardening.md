---
name: WebView2 Hardening (Cross-Cutting)
overview: WebView2 upstream leak'lerine karşı üç katmanlı savunma stratejisi. Faz 1, 1.5 ve 8.5'in ortak referansı. wRY/wry seviyesinde çözümsüz upstream bug'lar için watchdog + throttling + CEF escape hatch.
isProject: false
---

# WebView2 Hardening (Cross-Cutting)

> **Bu dosya Faz 1, 1.5, 1.6 ve 8.5'in hepsinde referans alınır.** WebView2'nin upstream leak'leri Microsoft tarafından kabul edilmiş, fix'siz (Haziran 2026 STATE: OPEN). **Üç katmanlı savunma → iki katmanlı yapısal çözüm:**
> 1. **Watchdog** (Faz 1) — sürekli izleme + soft restart + draft autosave
> 2. **~~Throttling~~ → Telemetry** (Faz 1.5) — fare hover GDI leak'i throttle deneyi **kanıtlanmış etkisiz** (Microsoft onayladı); telemetry + restart optimizasyonuna dönüştürüldü
> 3. **CEF escape hatch** (Faz 1.6) — Win11 default CEF, WebView2 opt-in

> **Önemli değişiklik (Haziran 2026):** CEF artık MVP'nin parçası (Faz 1.6), Faz 8.5'teki "koşullu" olmaktan çıktı. Detay: [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md).

---

## 1. Bilinen Tuzaklar (Haziran 2026 İtibarıyla)

### 1.1 wry#1691 / WebView2Feedback#5536 — GDI Object Leak (EN KRİTİK)

**Belirti:** Mouse hover sırasında DirectComposition üzerinden GDI objeleri leak ediyor (~4000 obje / 30s).

**Doğrulama:** Windows 11 build 26200 + WebView2 146'da onaylandı. Plain HTML'de bile oluyor (Discord.js yüklemeye bile gerek yok). Win10'da repro edilemedi.

**Denenen ve başarısız olan tüm WRY/host/CSS/JS seviyesi patch'ler:**
- `NotifyParentWindowPositionChanged` deferred call → etkisiz
- `SetIsVisible` toggle → daha kötü yaptı
- `WH_MOUSE` low-level hook → etkisiz
- `pointer-events: none` (CSS) → etkisiz
- Browser args (disable-gpu, disable-direct-composition, vb.) → etkisiz
- **JS rAF throttle (önceki plan) → etkisiz** (Microsoft issue #5536 doğruladı)
- 0 JS plain `<p>` HTML → **hâlâ leak ediyor**

**Neden başarısız:** WebView2 mouse input'u **DirectComposition üzerinden** alıyor. Win32 message queue bypass ediliyor; message hook'ları, CSS, JS throttle — hiçbiri işe yaramıyor.

**Viscos stratejisi (Haziran 2026 güncellemesi):**
- **Katman 1 (Faz 1):** `viscos-watchdog` crate ile sürekli `GetGuiResources` izleme. **7000/9000 threshold** (Haziran 2026 güncellemesi: önceki 5000/8000), soft restart + pre-restart draft autosave.
- **Katman 2 (Faz 1.5):** ~~JS pointermove rAF throttle~~ → **telemetry + restart optimizasyonu** (throttle kanıtlanmış etkisiz).
- **Katman 3 (Faz 1.6, MVP'nin parçası):** CEF backend **Win11 default** (yapısal çözüm). WebView2 Win10 default veya opt-in.

### 1.2 tauri#13758 — eval_script Unmanaged Lifecycle

**Belirti:** `evaluate_script`/`exec_script` fire-and-forget çalışıyor. Büyük JSON blob'ları IPC buffer şişirip ~2GB WebView limitine yaklaştırır.

**Çözüm:** Pull-based IPC pattern (Bölüm 3). Rust → JS push **yok** (küçük olaylar hariç).

### 1.3 tauri#13133 — Channel Callback Memory Leak

**Belirti:** Channel `onmessage` callback'i `window`'a ekleniyor ve unmount sırasında temizlenmiyor → tüm Vue/component scope'taki referanslar bellekte kalıyor.

**Çözüm:** Frontend wrapper'da `delete onmessage` pattern (Vue/component `onUnmounted` hook'unda otomatik).

### 1.4 WebView2Feedback#5138 — WebResourceResponseReceived Leak

**Belirti:** Subscribe/unsubscribe düzgün yapılsa bile renderer process memory büyüyor.

**Çözüm:** Bu event'i **hiç kullanma**. Pull-based IPC zaten bundan kaçınıyor.

### 1.5 WebView2Feedback#5266 — RDP GDI Region Leak

**Belirti:** RDP üzerinden çalışırken GDI region leak'i. 1+ yıl investigation, MOC of native rendering, "close+open form fixes".

**Çözüm:** RDP kullanıcılarına CEF backend öner (**Faz 1.6'da Win11 default**, korunur). RDP kullanıcıları Win10'da bile CEF'i tercih etmeli.

### 1.6 tauri#14924 — Linux WebKitGTK GBM Error 71 (v2)

**Belirti:** Linux'ta WebKitGTK zaten sorunlu, transparent pencerelerde ghosting, GBM buffer fails. NVIDIA driver'la çakışma.

**Çözüm:** v2'de CEF backend veya Servo Wry backend değerlendir (Faz 8.5 pluggable mimarisi sayesinde).

### 1.7 WebView2Feedback#5601 — Mouse Drag Main-Thread Starvation (REGRESYON)

**Belirti:** FPS uncapped (örn. oyun içi senaryolar) olduğunda mouse drag sırasında WebView2 ana thread'i IPC/WebSocket/Worker mesajlarını işleyemez hale geliyor. Chromium 83 sonrası regresyon; upstream önerilen patch reddedilmiş. Kullanıcılar 133.0.3065.92'de pinlemek zorunda.

**Viscos etkisi:** Discord'ta fare ile sürükle-bırak (kanal sıralama, dosya yükleme) sırasında realtime mesaj gecikmesi olabilir. Faz 1.5 throttling deneyinde bu yüzden FPS cap (60) default olmalı; CEF opt-in bu bug'tan da kaçış sağlar.

**Çözüm:** CEF opt-in (Faz 8.5); kısa vadede `CoreWebView2` ortamında `--disable-frame-rate-limit` bayrağı set etme.

---

## 2. Microsoft Resmi Önerileri (Uygulanacak)

| Öneri | Ne Zaman | Uygulama |
|-------|----------|----------|
| `MemoryUsageTargetLevel.Low` | İnaktif WebView'lerde | `CoreWebView2MemoryUsageTargetLevel::Low` set et |
| App-level process sharing | Her zaman | Tek WebView + tek `CoreWebView2Environment` |
| Periyodik WebView2 refresh | Kanal değişimi veya watchdog tetikli | Dispose + recreate |
| `TrySuspend`/`Resume` | Uzun süre kullanılmayacaksa | Askıya al |
| Monitor memory | WPR recordings | Debug için |

---

## 3. Pull-Based IPC Pattern (KRİTİK)

WebView2 ↔ Rust köprüsünde **asla** Rust → JS push yapma (büyük veri). Bunun yerine:

```
Rust → Event Bus (moka / tokio broadcast)
                ↓
JS tarafı ihtiyaç duyduğunda invoke("get_state") ile pull eder
                ↓
Rust cache'ten döner (RAM moka veya SQLite)
```

**Neden?**
- Push-based: Rust sürekli büyük JSON gönderecek → WebView2 IPC buffer şişer (tauri#13758)
- Pull-based: JS sadece görünür kanalın verisini ister → minimum transfer
- Bonus: Backpressure doğal olarak sağlanır

**Tek exception:** Tray icon badge, mention notification gibi **küçük ve gerçek zamanlı** olaylar push kalabilir.

**Implementation guideline (her fazda):**
- `eval_script` payload > 10KB → CI red flag
- `viscos-ipc` crate'inde `Invoke` (JS → Rust) ve `Emit` (Rust → JS, sadece küçük olaylar) trait'leri
- Channel `onmessage` → unmount hook'unda `delete onmessage` (frontend wrapper otomatik yapar)

### 3.1 Faz 4 Backlog: Büyük Blob'lar İçin WebView2 SharedBuffer

Pull-based pattern **%90 IPC trafiğini** doğru çözüyor (küçük string komutlar, ack'ler, presence event'leri). Ancak **büyük binary response'lar** (avatar, sticker, emoji, attachment thumb) için JSON + base64 round-trip suboptimal:

- 80KB avatar → ~107KB base64 string → `serde_json` parse (her iki tarafta)
- tauri#13758 upstream bug'ının tetiklenme yüzeyi hâlâ var (her `PostWebMessageAsJson` bir `eval_script` instantiate eder)
- Scrolling sırasında 50+ avatar paralel fetch → 4MB transient memory churn

**Faz 4 kararı:** `WebViewBackend::post_shared_buffer` trait method'u implement edilecek. WebView2 backend'i `CoreWebView2SharedBuffer` API'sini kullanacak; CEF backend'i (Faz 8.5) `SharedMemoryRegion` + `message_router` threshold-based davranışı. JS tarafı `ArrayBuffer` olarak zero-copy okur.

**Faz 1'de yapılan:** Trait'te `post_shared_buffer` stub metodu var (default: `unimplemented!()`). Faz 4'e kadar implementasyon yok.

**Faz 4'te yapılacak:**
- `webview2-com` dependency (veya `wry` upstream PR'ı, [`wry#767`](https://github.com/tauri-apps/wry/issues/767) takip)
- `WryWebView2Backend::post_shared_buffer` implementasyonu: `CreateSharedBuffer` → `OpenStream::Write` → `PostSharedBufferToScript`
- `frontend/bridge.ts`'e `getBinary<T>()` API'si, `try/finally releaseBuffer()` zorunlu pattern
- Benchmark hedefi: 100 ardışık 80KB avatar fetch, JSON vs SharedBuffer, **>5× latency düşüşü**

**Upstream kısıt:** [`WebView2Feedback#3360`](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3360) — 32.000 × 1MB SharedBuffer sonrası crash, **Edge 114+ fix'li**. Viscos mininum Edge sürümünü `WEBVIEW2_RELEASE_CHANNEL_PREFERENCE` ile pin etmeli veya runtime version check eklemeli.

**Plugin contract etkisi:** Yok. Vencord/Equicord plugin'leri hâlâ JSON ile konuşur; SharedBuffer sadece Viscos'un internal medya response'ları için.

Detay: [`phase-4.0-cache-media.md` Bölüm 4.4](./phase-4.0-cache-media.md#44-büyük-blob-transfer-webview2-sharedbuffer).

---

## 4. Üç Katmanlı Savunma Detay

### Katman 1: Watchdog (Faz 1 — `viscos-watchdog` crate)

**`GetGuiResources` ile GDI object sayacı (her 30s):**
- Win32 API: `GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS)`
- **7000 GDI → warning log** (Haziran 2026 güncellemesi: önceki 5000)
- **9000 GDI → soft restart (WebView dispose + recreate)** (Haziran 2026 güncellemesi: önceki 8000)
- 12000 GDI → kullanıcıya "Discord'u yeniden başlat" dialog (sadece soft restart başarısızsa)

**Pre-restart draft autosave hook:**
- Restart öncesi `DraftAutosave::snapshot_open_composers()` çağrılır
- Açık mesaj taslakları SQLite'a yazılır (Faz 2'de tam entegrasyon, Faz 1'de in-memory stub)
- Restart sonrası `bridge.ts` otomatik restore eder
- **Draft kaybı: 0** (kabul kriteri)

**IPC buffer tracker (her 60s):**
- `eval_script` payload boyutunu topla (moka counter)
- 50MB → warning
- 100MB → WebView2 refresh

**Heap fragmentation tracker (Faz 4 sonrası):**
- jemalloc stats oku (eğer allocator geçildiyse)
- Stair-step pattern tespiti (Svix case study)
- Aylık %5'ten fazla artış → flag

**Sürekli `tracing` log + opsiyonel WPR dump:**
- Release modda her saat özet log
- Debug modda anlık metric export
- `/diagnostics` endpoint'i (Faz 5 native UI'da gösterilebilir)

**Tray icon badge (Faz 1.5 telemetry):**
- "Viscos (3 restart today)" tooltip — şeffaflık

**Telemetry storage (Faz 1.5 — `viscos-telemetry`):**
- SQLite (`%APPDATA%/Viscos/telemetry.db`)
- Rolling 30 gün retention, 100MB cap
- GDI time-series, restart events, watchdog config history
- **24h restart count query** → Faz 1.6 CEF default kararı

**Unit test:**
- GDI counter mock'lu (Win32 mock veya fake process handle)
- Threshold logic testleri
- Auto-restart trigger koşulları
- Draft autosave snapshot testi

**Soak test (24 saatlik, lokal manuel):**
- Normal kullanım simülasyonu
- Mouse hover %50 süre
- Mesaj scroll %30 süre
- Idle %20 süre
- **Kabul: <5 restart + gap <2s + draft kaybı 0**

### Katman 2: ~~Throttling~~ → Telemetry (Faz 1.5 — kanıtlanmış etkisiz)

**ESKİ PLAN (iptal):**
> WebView2 üzerinde `pointermove` event throttle (requestAnimationFrame ile):
> - İlk test: her 3 frame'de bir event işle (vsync ile sync)
> - Benchmark: 30s mouse hover, GDI objesi sayısı
> - Eğer ≥%50 GDI düşüşü → throttling aktif kalır
> - Eğer <%20 GDI düşüşü → Katman 3'e güven, throttling kaldırılır

**HAZİRAN 2026 BULGUSU — THROTTLE İPTAL:**

Microsoft Edge WebView2 [issue #5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536) ve `tauri-apps/wry#1691` upstream testleri kanıtladı:

| Workaround | Sonuç |
|-----------|-------|
| `SetIsVisible(false/true)` toggle | **Daha kötü** (6000 GDI/30s) |
| WRY parent subclass (`NotifyParentWindowPositionChanged` deferred) | Etkisiz |
| `WH_MOUSE` low-level hook | Etkisiz |
| Subclass WebView2 child windows (`WM_MOUSEMOVE` throttle) | Etkisiz |
| `--disable-gpu-compositing` browser arg | Etkisiz |
| `--disable-features=RemoveRedirectionBitmap` | Etkisiz |
| CSS `:hover` kaldır | Etkisiz |
| CSS `pointer-events: none` | Etkisiz |
| **JS `requestAnimationFrame` throttle** | **Etkisiz** |
| 0 JS plain `<p>` HTML | **Hâlâ leak ediyor** |

**Neden:** WebView2 mouse input'u **DirectComposition üzerinden** alıyor. Win32 message queue'yu bypass ediyor. JS katmanında throttle etmek browser process'e ulaşan mouse event'lerini durdurmuyor.

**YENİ KATMAN 2 — Telemetry + Restart Optimization:**

JS-side throttle **yapılmadı** (etkisiz kanıtlandı), bunun yerine Faz 1.5'te:

- **Telemetry backend** (`viscos-telemetry` crate): GDI time-series, restart events, OS build, WebView2 version. SQLite storage, 30 gün rolling retention.
- **Draft autosave hardening**: SQLite stub (Faz 2'de tam entegrasyon). Pre-restart hook ile mesaj kaybı 0.
- **Tray icon badge**: "X restart today" tooltip — şeffaflık, kullanıcı watchdog davranışını görüyor.
- **Pull-based IPC audit tool** (`viscos-ipc-audit`): Tüm `eval_script` call'larını enumerate, >10KB payload → CI fail. (`tauri#13758` upstream bug yüzeyi azaltma.)
- **Channel callback cleanup pattern**: Frontend wrapper'da `delete onmessage` otomatik. (`tauri#13133` upstream bug workaround.)
- **CefRolloutRecommendation**: Telemetry 24h restart count + 7d peak GDI → "Stay / Recommend / Force" CEF. Faz 1.6'yı tetikler.

**A/B test (iptal):** Pointermove throttle A/B testi anlamsız (kanıtlanmış etkisiz). Yerine 24h soak telemetry testi.

Detay: [`phase-1.5-telemetry-and-restart-optimization.md`](./phase-1.5-telemetry-and-restart-optimization.md).

### Katman 3: CEF Backend (Faz 1.6 — Win11 default, MVP'nin parçası)

**`WebViewBackend` trait abstraction (Faz 1'de tanımlandı):**
```rust
pub trait WebViewBackend: Send + Sync {
    fn create(&self, config: &WebViewConfig) -> Result<Box<dyn WebViewHandle>>;
    fn version(&self) -> &'static str;
    fn known_issues(&self) -> &[&'static str];
}

pub enum BackendKind {
    WebView2,  // Win10 default, Win11 opt-in
    Cef,       // Win11 default (Faz 1.6 — MVP'nin parçası)
}
```

**Haziran 2026 zamanlama değişikliği:**
- Önceki plan: CEF Faz 8.5'te "koşullu opt-in" idi
- Güncel plan: CEF Faz 1.6'da **Win11 default** (MVP'nin parçası)
- Gerekçe: Microsoft bug yapısal olarak çözümsüz, telemetry verisi MVP'den önce yeterli olmayabilir, Win11 kullanıcılarının %100'ü bug'a açık

**Build artifact:**
- İki ayrı MSI: `viscos-webview2.msi` (15–25MB) ve `viscos-cef.msi` (220–300MB)
- İlk çalıştırma: `select_default_backend()` — Win11 → CEF, Win10 → WebView2
- Telemetry force override: ≥10 restart/24h → CEF zorla
- Config.toml'da user override

**Kullanım senaryoları:**
- Win11 default CEF (önerilen): Disk alanı kabul, leak'siz UX
- Win10 default WebView2: Daha hafif binary, leak riski düşük
- RDP kullanıcısı: CEF (WV2#5266'ya tabi değil)
- Disk alanı kısıtlı Win11: WebView2 + agresif watchdog (threshold 5000)

**Faz 8.5 (default-out yönetim):**
- Tam iced backend yönetim UI'ı
- Chromium flags ileri config
- CEF self-update
- Disk/pil/performans trade-off'ları kullanıcı kontrolünde

Detay: [`phase-1.6-cef-default-rollout.md`](./phase-1.6-cef-default-rollout.md), [`phase-8.5-cef-backend.md`](./phase-8.5-cef-backend.md).

**CI:**
- İki backend için ayrı test pipeline
- Aynı test suite her ikisinde de çalışmalı
- Benchmark: RAM, cold start, binary size farkı raporu

---

## 5. Karar Noktaları (Her Faz Sonunda İnsana Sorulacak)

| Faz | Karar | Seçenekler |
|-----|-------|-----------|
| Faz 1 sonu | GDI threshold | **7000/9000** (önerilen) vs 5000/8000 vs config-driven |
| Faz 1 sonu | Auto-restart agresifliği | **Soft + draft autosave** (önerilen) vs dialog (12K'da) vs hard restart |
| Faz 1.5 sonu | Telemetry retention | **30 gün** (önerilen) vs 7 gün vs 90 gün |
| Faz 1.5 sonu | Tray badge format | **"X restart today" tooltip** (önerilen) vs sadece tooltip vs bildirim popup |
| Faz 1.5 sonu | Faz 1.6 tetikleme | **≥5 restart/24h OR peak GDI ≥8500** (önerilen) vs agresif 3 vs gevşek 10 |
| Faz 1.5 sonu | Telemetry opt | **Opt-out (default açık)** vs opt-in |
| Faz 1.6 sonu | Win11 default CEF | **Default CEF** (önerilen) vs wizard vs default WebView2 + öneri |
| Faz 1.6 sonu | Telemetry force | **Asla force** (önerilen) vs 10 restart/24h → force |
| Faz 1.6 sonu | CEF versiyonu | **Stable** (önerilen) vs beta vs LTS |
| Faz 8.5 sonu | Backend UI görünürlüğü | **Görünür** (önerilen) vs advanced settings |
| Faz 8.5 sonu | CEF self-update | **Aylık** (önerilen) vs haftalık vs quarterly |
| Faz 8.5 sonu | Backend geçiş dialog | **Sessiz** (önerilen) vs onay dialog |

---

## 6. Referanslar

- `tauri-apps/wry#1691` — WebView2 GDI object leak (CLOSED, WebView2 upstream bug)
- `WebView2Feedback#5536` — GDI leak upstream report (Haziran 2026 STATE: OPEN)
- `WebView2Feedback#5266` — RDP GDI region leak
- `WebView2Feedback#5138` — WebResourceResponseReceived leak
- `tauri-apps/tauri#13758` — eval_script unmanaged lifecycle
- `tauri-apps/tauri#13133` — channel callback memory leak
- `tauri-apps/tauri#14924` — Linux WebKitGTK GBM Error 71
- Microsoft WebView2 Performance: https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance
- Tauri `cef-rs`: https://github.com/tauri-apps/cef-rs
- `wrymium`: CEF backend for Tauri (Viscos Tauri kullanmadığı için uygun değil; referans amaçlı izleniyor)
- `WebView2Feedback#5601` — Mouse drag main-thread starvation (regresyon, Chromium 83+)
- MS Teams WebView2 post-mortem: https://www.windowslatest.com/2026/06/11/microsoft-claims-ms-teams-is-more-responsive-now-but-it-still-eats-your-pcs-ram-doing-nothing/ (endüstriyel ölçekte aynı sorunlar)
- `tinyhumansai/openhuman` — Çoklu CEF webview production deployment (Discord dahil, CEF 146)
- `phase-1.5-telemetry-and-restart-optimization.md` — Yeniden adlandırıldı (önceki: `phase-1.5-mouse-throttling.md`)
- `phase-1.6-cef-default-rollout.md` — Yeni faz, Win11 CEF default
- `window-webview-watchdog-tradeoffs.md` — Haziran 2026 tradeoff analizi
