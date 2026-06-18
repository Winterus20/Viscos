# WebView2 vs CEF — Hangisini Seçmeli?

> **Kaynak:** [`docs/DECISIONS.md` ADR-0012](../DECISIONS.md) (Haziran 2026).
> **İlgili planlar:** [`phase-1.6-cef-default-rollout.md`](../.cursor/plans/phase-1.6-cef-default-rollout.md), [`phase-8.5-cef-backend.md`](../.cursor/plans/phase-8.5-cef-backend.md).

Bu doküman Viscos kullanıcıları ve katkıcıları için **WebView2 ve CEF backend arasındaki somut trade-off**'u belgeler. Discord'un web client'ını render etmek için Viscos iki ayrı WebView backend'i destekler; her birinin kendi avantajları, riskleri ve ideal kullanım senaryoları vardır.

---

## 1. Hızlı Karar Tablosu

| Senaryo | Öneri | Gerekçe |
|---------|-------|---------|
| Windows 10 + günlük kullanım | **WebView2** | Hafif binary (18 MB), Edge güncellemesi ile otomatik güvenlik fix |
| Windows 11 + 7/24 açık bırakılan | **CEF (default)** | WebView2 GDI leak yapısal çözümsüz, CEF leak'siz |
| Windows 11 + disk alanı kısıtlı | WebView2 + agresif watchdog (5000/8000) | Binary küçük ama sık restart kabul edilmeli |
| **RDP üzerinden** | **CEF (zorla, auto-detect)** | Microsoft WebView2 RDP bug (#5266) düzeltilmedi |
| Sık kapat-aç | WebView2 | Cold start avantajı önemli |
| Multi-platform (Linux/macOS, ilerde) | **CEF** | Cross-platform tutarlılık |
| Vencord/Equicord plugin kullanımı | İkisi de OK | WebView2 daha yaygın test edilmiş |

---

## 2. Detaylı Karşılaştırma

### 2.1 WebView2

**Microsoft Edge WebView2** — Windows 10/11'de OS ile gelen Chromium-tabanlı WebView bileşeni.

#### Avantajlar

- **15–25 MB binary.** Viscos'un kendisi ince, Edge ile aynı Chromium'u paylaşır.
- **OS WebView, Edge güncellemesi ile güvenlik fix otomatik.** Kullanıcı bir şey yapmadan güvenlik yaması alır.
- **Cold start < 1.5s.** Edge zaten açıksa (çoğu Windows kullanıcısı) süre daha da düşer.
- **Düşük idle RAM (~150–200 MB).** Chromium process'i Edge ile paylaşıldığı için ek maliyet yok.
- **Vencord/Equicord plugin uyumu:** Vencord ekibi kendi Electron + WebView2 ortamlarında test ediyor (Vesktop, Legcord kanıtı).
- **Disk alanı minimum:** %APPDATA%/Viscos/webview2-cache ≈ 50–150 MB.

#### Dezavantajlar

- **Win11 GDI object leak (Microsoft Edge WebView2 Feedback [#5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536), STATE: OPEN).** Mouse hover ile ~4000 GDI obje / 30sn leak. Microsoft upstream'te fix yok (Haziran 2026). Viscos Faz 1'de watchdog ile maskeliyor (soft restart), Faz 1.5 telemetry ile ölçüyor, Faz 1.6'da Win11 default CEF'e geçiyor.
- **RDP üzerinden GDI region leak (WV2 [#5266](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5266)).** RDP session'da ek leak katmanı. Yapısal çözüm: CEF default'a geçiş (Faz 1.6 + ADR-0012 §6).
- **Microsoft'a bağımlılık:** Microsoft WebView2'yi deprecate ederse Viscos Win10 + Win11 için alternatif aramak zorunda. Risk düşük (Edge için stratejik), ama sıfır değil.
- **Upstream bug'lar:** `tauri#13133` (channel callback leak), `tauri#13758` (`eval_script` unmanaged lifecycle). Viscos bunları pull-based IPC + watchdog ile tolere ediyor.

#### Önerilen Kullanıcı Profili

- Win10 + sık kapat-aç
- Disk alanı kısıtlı (laptop, küçük SSD)
- Edge'i yoğun kullanan, Chromium'un OS güncellemesiyle gelmesini tercih eden
- RDP kullanmayan

---

### 2.2 CEF (Chromium Embedded Framework)

**CEF** — Chromium'u kendi process'inde çalıştıran embeddable WebView. Viscos `tauri-apps/cef-rs` entegrasyonu ile kullanır.

#### Avantajlar

- **Leak'siz.** Chromium kendi runtime'ını kullanır, Win32 GDI'ya bağımlı değildir. Win11 WebView2 GDI leak'inden tamamen korunmuş.
- **Multi-process mimari:** IPC, renderer, GPU process'leri izole. WebView2'deki unmanaged `eval_script` lifecycle bug'ları yok.
- **RDP güvenli:** WebView2 #5266 bug'ından etkilenmez.
- **Cross-platform tutarlılık:** v2.0'da Linux + macOS aynı engine ile çalışır (CEF tüm platformlarda stabil).
- **7/24 açık bırakılabilir.** Soft restart gerekmez (GDI leak yok).
- **Chromium flags ile agresif optimizasyon:** `--disable-background-networking`, `--disable-extensions`, `--no-first-run`, vb.

#### Dezavantajlar

- **220–300 MB binary.** Chromium runtime gömülü, Edge ile paylaşılmıyor. Faz 8.0 self-update indirilebilir hale getirebilir (henüz kararlaştırılmadı).
- **Cold start 1.5–2.5s.** Chromium'un tamamen başlatılması gerekiyor, WebView2'den yavaş.
- **Idle RAM +50–100 MB.** Ek process + IPC overhead.
- **Disk alanı +150 MB cache.** %APPDATA%/Viscos/cef-cache disk alanı kullanır.
- **Kendi Chromium'unu güncellemek gerekir.** Edge gibi OS güncellemesi ile gelmiyor. Faz 8.5 self-update + Faz 1.5'teki Chromium advisory feed skeleton'ı + ADR-0012 §CefUpdate (haftalık + kritik CVE tetikleyici) ile yönetiliyor.
- **Disk alanı yüksek:** Faz 8.0'da MSI 220–300 MB, kullanıcı disk alanı kısıtlıysa WebView2 MSI tercih edebilir.

#### Önerilen Kullanıcı Profili

- Win11 + 7/24 açık bırakan
- RDP üzerinden çalışan (IT/admin)
- Cross-platform (gelecekte Linux/macOS) isteyen
- Disk alanı yeterli
- Chromium güvenlik güncellemelerini takip etmek isteyen

---

## 3. Bilinen Microsoft WebView2 Bug'ları

| Issue | Başlık | Etki | Viscos Stratejisi |
|-------|--------|------|-------------------|
| [#5536](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536) | Mouse hover GDI leak | Win11'de 4000 GDI/30sn, yapısal çözümsüz | Faz 1 watchdog + Faz 1.5 telemetry + Faz 1.6 Win11 default CEF |
| [#5266](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5266) | RDP GDI region leak | RDP session'da ek leak | Faz 1.6 RDP auto-detect (GetSystemMetrics(SM_REMOTESESSION)) → CEF zorla |
| [#3185](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3185) | SharedBuffer request | Zero-copy blob transfer | Faz 4'te SharedBuffer implementasyonu |
| [#3360](https://github.com/MicrosoftEdge/WebView2Feedback/issues/3360) | SharedBuffer follow-up | API uyumlu hale getirme | Faz 4'te çözüldü |

---

## 4. Backend Seçim Mekanizması (Faz 1.6)

Viscos ilk açılışta backend'i şu sırayla seçer:

```text
1. Config override (config.toml backend.kind = "cef" | "webview2")
   ↓ (yoksa)
2. RDP session auto-detect (GetSystemMetrics(SM_REMOTESESSION) != 0)
   → RDP ise CEF zorla
   ↓ (yoksa)
3. Telemetry override
   - restarts_24h >= 10 → ForceCefDefault
   - restarts_24h >= 5 OR peak_gdi_7d >= 8500 → RecommendCefDefault (sadece bildirim)
   - yoksa → StayWebView2
   ↓ (yoksa)
4. Platform default
   - Win11 (build >= 22000) → CEF
   - Win10 → WebView2
```

**Öncelik:** `config.toml > RDP auto-detect > telemetry > platform default`.

Kullanıcı istediği zaman `Ayarlar > Backend > WebView2/CEF` değiştirebilir (Faz 5 sonrası iced UI; Faz 1.6'da CLI: `viscos-backend set {cef,webview2}`).

---

## 5. Benchmark (v1.0 Hedef)

| Metrik | WebView2 | CEF | Fark |
|--------|----------|-----|------|
| Binary boyutu | 18 MB | 240 MB | +222 MB |
| Cold start | 1.2s | 2.1s | +0.9s |
| Idle RAM | 180 MB | 240 MB | +60 MB |
| 24h soak: GDI peak | 8500 (sonra restart) | 1200 (sabit) | -7300 |
| 24h soak: restart count | 4–6 | 0 | -4–6 |
| RDP davranışı | Leak var (WV2#5266) | Stabil | - |
| Disk alanı | +50–150 MB cache | +150 MB cache + 240 MB binary | - |

**Kaynak:** [`phase-8.5-cef-backend.md` Bölüm 5](../.cursor/plans/phase-8.5-cef-backend.md).

---

## 6. Sık Sorulan Sorular (FAQ)

### 6.1 Neden CEF default 240 MB?

Chromium runtime gömülü (Edge ile paylaşılmıyor). Avantaj: 7/24 leak'siz, RDP güvenli, cross-platform tutarlı. Disk alanı ödünü kabul ediyoruz çünkü Win11 kullanıcılarının %100'ü WebView2 GDI leak'ine açık.

### 6.2 CEF Chromium güncel mi?

Faz 1.5'te `ChromiumAdvisoryFeed` skeleton'ı + Faz 8.5'te haftalık scrape (`https://chromereleases.googleblog.com/feeds/posts/default`) ile kontrol ediliyor. **Kritik CVE çıkarsa acil update tetiklenir** (ADR-0012 §CefUpdate). WebView2 gibi otomatik değil ama güvenlik patch'leri yetişir.

### 6.3 WebView2'ye nasıl dönerim?

Ayarlar > Backend > WebView2 seçin, restart. Win11 kullanıyorsanız GDI leak riskini kabul ediyorsunuz (Faz 1 watchdog + telemetry restart'ları tolere eder, Faz 1.6+ önerilen CEF'tir).

### 6.4 RDP üzerinden çalışmıyor

CEF önerilir (veya zorla — auto-detect açık). WebView2 RDP'de GDI region leak yapar (Microsoft bug #5266, düzeltilmedi).

### 6.5 Disk alanı kısıtlı, hangisini seçmeliyim?

WebView2 MSI'yı indirin (18 MB). RDP kullanmıyorsanız ve Win10'daysanız sıkıntı yok. Win11 + WebView2 kombinasyonu watchdog + telemetry restart'ları ile yaşanabilir ama optimal değil.

### 6.6 Linux/macOS desteği ne zaman?

v2.0 (Faz 8.5 sonrası). CEF her iki platformda da stabil — Linux'ta `cef-rs` desteği, macOS'ta Apple Silicon native. WebView2 cross-platform değil (Linux: WebKitGTK, macOS: WKWebView) → multi-platform hedefte CEF tercih.

---

## 7. Gelecek: Servo (v3 Backlog)

Servo (Rust web engine, Linux Foundation Europe, Igalia geliştirmesi) Haziran 2026'da Discord için **hazır değil** (sadece login + mesaj okuma, mesaj yazamıyor, WebRTC encoded transform yok). v3'te değerlendirilecek. Detay: ADR-0012.

---

## 8. Değişiklik Geçmişi

| Tarih | Değişiklik | Gerekçe |
|-------|-----------|---------|
| 2026-06-18 | İlk yayın | ADR-0012 frontend mimari kararı + Faz 1.6 Win11 default CEF + Faz 1.5 telemetry + Faz 8.5 self-update ile uyumlu |
