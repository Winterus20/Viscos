---
name: Phase 8.5 — Backend Management UI (Default-Out Artık)
overview: Faz 1.6'da Win11 default CEF olarak MVP'ye alındı. Faz 8.5 artık "opsiyonel escape hatch" değil, **default-out yönetim**: CEF artık Win11 için default, kullanıcı isterse WebView2'ye opt-out yapabilir. Tam backend yönetim UI'ı (iced) + ileri seviye ayar (Chromium flags, cache strategy).
isProject: false
todos:
  - id: cef-ui
    content: Backend yönetim UI'ı (iced): WebView2 ↔ CEF geçiş wizard'ı
    status: pending
  - id: chromium-flags
    content: İleri seviye: kullanıcı Chromium flags set edebilsin
    status: pending
  - id: cache-strategy
    content: CEF cache stratejisi (boyut, retention, clear on exit)
    status: pending
  - id: backend-benchmark
    content: Backend benchmark: RAM, cold start, binary size (release notes)
    status: pending
  - id: cef-self-update
    content: CEF self-update (Faz 8.0 distribution ile entegre)
    status: pending
  - id: cef-docs-update
    content: docs/CEF-VS-WEBVIEW2.md güncelle (v1.5 sonrası feedback)
    status: pending
  - id: cve-feed
    content: Chromium security advisory feed (haftalık scrape, kritik CVE → acil update tetikleyici) — ADR-0012 §CefUpdate + Faz 1.5 skeleton devralınır
    status: pending
  - id: rdp-default-cef
    content: RDP session auto-detect (Faz 1.6'dan devralınır, default CEF policy) — ADR-0012 §6
    status: pending
  - id: bridge-resilience-doc
    content: Frontend bridge resilience kuralları Faz 1.0/1.6 deliverable'ları referansı (BRIDGE-RESILIENCE.md) — ADR-0012 §2
    status: pending
---

# Phase 8.5 — Backend Management UI (Default-Out Artık)

> **Süre:** 2 hafta
> **Hedef:** Faz 1.6 sonrası **default-out yönetim**: CEF Win11 için default, kullanıcı isterse WebView2'ye geçebilir. Backend yönetim UI'ı (iced) + ileri seviye Chromium flags + cache stratejisi.
> **Kritik referans:** [`webview2-hardening.md`](./webview2-hardening.md), [`phase-1.6-cef-default-rollout.md`](./phase-1.6-cef-default-rollout.md)
> **Önceki faz:** [`phase-8.0-distribution.md`](./phase-8.0-distribution.md)
> **Önemli değişiklik:** Faz 1.6'da MVP'ye alındı (önce "koşullu Faz 8.5" idi). Artık bu faz **yönetim UI + ileri özellikler**.

---

## 1. Kapsam Değişikliği

**Önceki plan (v1):**
- Bu faz "koşullu" idi: sadece GDI leak kullanıcı şikayeti varsa aktif
- Opt-in CEF, default WebView2

**Haziran 2026 güncellemesi:**
- WebView2 GDI leak (Microsoft issue #5536, Haziran 2026 STATE: OPEN) **yapısal olarak çözümsüz**
- Faz 1.5 telemetry verisi karar vermek için **yeterli** olmayabilir — MVP'de Win11 kullanıcıları kötü UX yaşar
- Faz 1.6'da CEF Win11 default olarak MVP'ye alındı (koşulsuz)
- Bu faz artık **"backend yönetim katmanı"** — opt-out wizard + ileri config + self-update

**Koşul:** Bu faz sadece Faz 1.6 başarıyla tamamlandıysa (CEF Win11 default stabil) ilerler.

---

## 2. Backend Yönetim UI'ı (iced)

Faz 5 native UI ile entegre. Ayarlar > Backend bölümü:

```rust
// crates/viscos-shell/src/ui/backend_settings.rs

use iced::{Column, Text, Button, Radio, Element, Space};
use viscos_webview::BackendKind;
use viscos_config::Config;

#[derive(Debug, Clone)]
pub enum BackendSettingsMessage {
    Select(BackendKind),
    ApplyAndRestart,
    OpenDocsLink,
}

pub struct BackendSettings {
    current: BackendKind,
    selected: BackendKind,
    config: Config,
}

impl BackendSettings {
    pub fn view(&self) -> Element<BackendSettingsMessage> {
        let platform_default = platform_recommended_default();

        Column::new()
            .push(Text::new("Discord Render Backend").size(24))
            .push(Text::new(
                "Viscos iki render motoru destekler: WebView2 (hafif, OS WebView) \
                 ve CEF (Chromium, leak'siz)."
            ))
            .push(Space::with_height(10))
            .push(self.radio_row(
                BackendKind::WebView2,
                "WebView2",
                &format!(
                    "15-25 MB binary, OS WebView. Önerilen: Win10, kısa oturumlar. \
                     Win11'de GDI leak riski (Microsoft upstream bug, Haziran 2026).",
                ),
            ))
            .push(self.radio_row(
                BackendKind::Cef,
                "CEF (Chromium)",
                "220-300 MB binary, kendi Chromium runtime. Önerilen: Win11, \
                 7/24 açık bırakılan kullanım. Leak'siz.",
            ))
            .push(Space::with_height(20))
            .push(Text::new(format!(
                "Platform önerisi: {:?}. Mevcut: {:?}",
                platform_default, self.current
            )))
            .push(Space::with_height(20))
            .push(
                Button::new(Text::new("Uygula ve Yeniden Başlat"))
                    .on_press(BackendSettingsMessage::ApplyAndRestart)
            )
            .push(
                Button::new(Text::new("Detaylı karşılaştırma (docs)"))
                    .on_press(BackendSettingsMessage::OpenDocsLink)
            )
            .into()
    }

    fn radio_row(
        &self,
        kind: BackendKind,
        label: &str,
        description: &str,
    ) -> Element<BackendSettingsMessage> {
        Column::new()
            .push(
                Radio::new(
                    kind,
                    label,
                    Some(self.selected),
                    BackendSettingsMessage::Select,
                )
            )
            .push(Text::new(description).size(12))
            .into()
    }
}
```

**UX:**
- Platform önerisi üstte, kullanıcı isterse override eder
- Backend değişikliği → restart zorunlu (trade-off net gösterilir)
- "Detaylı karşılaştırma" docs link'i (CEF-VS-WEBVIEW2.md)

---

## 3. İleri Seviye: Chromium Flags

Power user için `config.toml`'da Chromium flag override:

```toml
[backend.cef]
chromium_flags = [
    "--disable-features=Translate",
    "--disable-background-networking",
    "--disable-extensions",
    "--no-first-run",
    # Kullanıcı ekleyebilir, ama bilinen unstable flag'ler deny-list'lenir
]

# Bilinen sorunlu flag'ler (CI deny)
[backend.cef.deny_flags]
flags = [
    "--single-process",      # CEF multi-process zorunlu
    "--disable-gpu",         # GPU olmadan CEF düzgün çalışmaz
]
```

**Güvenlik:** `--disable-web-security` gibi güvenlik-bypass flag'leri deny-list'te, kullanıcı ekleyemez (CI fail).

---

## 4. CEF Cache Stratejisi

```toml
[backend.cef.cache]
# Default: 250 MB (Chromium standart)
max_size_mb = 250

# Clear on exit: privacy odaklı kullanıcılar için
clear_on_exit = false

# Retention: en az N gün, eski dosyaları sil
min_retention_days = 7
```

**IPC command:**
```rust
IpcCommand::ClearCefCache  // Settings'ten veya hotkey ile
```

`CefWebViewHandle::clear_cache()` implementasyonu Faz 4'te (cache crate entegrasyonu).

---

## 5. Backend Benchmark & Release Notes

Release öncesi benchmark, sonuçlar `docs/CEF-VS-WEBVIEW2.md`'ye eklenir:

| Metrik | WebView2 | CEF | Fark |
|--------|----------|-----|------|
| Binary boyutu | 18 MB | 240 MB | +222 MB |
| Cold start | 1.2s | 2.1s | +0.9s |
| Idle RAM | 180 MB | 240 MB | +60 MB |
| 24h soak: GDI peak | 8500 (sonra restart) | 1200 (sabit) | -7300 |
| 24h soak: restart count | 4-6 | 0 | -4-6 |
| RDP davranışı | Leak var (WV2#5266) | Stabil | - |

**Release notes format:**
```markdown
# Viscos v1.0.0 — Backend Performance

## WebView2
- Binary: 18 MB
- Cold start: 1.2s
- 24h soak (Win10): stabil, 0 restart
- 24h soak (Win11): 4-6 restart, kabul edilebilir UX
- RDP: leak riski (bilinen Microsoft bug)

## CEF
- Binary: 240 MB
- Cold start: 2.1s
- 24h soak (Win11): 0 restart, leak yok
- RDP: stabil
```

---

## 6. CEF Self-Update (Faz 8.0 ile entegre)

CEF kendi Chromium'unu güncellemek zorunda — Edge gibi otomatik değil. Faz 8.0 self-update mekanizması CEF binary'sini de güncelleyecek:

```rust
// crates/viscos-update/src/cef.rs

pub struct CefUpdate {
    current_version: CefVersion,
    latest_version: CefVersion,
    download_url: String,
    sha256: String,
    trigger: CefUpdateTrigger,                  // YENİ: ADR-0012 §CefUpdate
}

pub enum CefUpdateTrigger {
    ScheduledMonthly,                           // Faz 8.5 plan'ında "ayda bir"
    ScheduledWeekly,                            // YENİ: routine weekly check
    CriticalCveDetected(Vec<CveAlert>),         // YENİ: ADR-0012 §CefUpdate
}

impl CefUpdate {
    pub async fn check() -> anyhow::Result<Option<Self>> {
        // 1. Faz 1.5'teki skeleton'ı devral — ChromiumAdvisoryFeed
        //    https://chromereleases.googleblog.com/feeds/posts/default scrape
        //    Son 7 gündeki Stable channel + Security tag'li post'ları al
        // 2. CVE-YYYY-NNNNN parse + severity (Critical/High/Medium/Low)
        // 3. critical_cves = Critical severity olanlar → CefUpdateTrigger::CriticalCveDetected
        // 4. Routine update = Faz 1.5 telemetry'den "ayda 1 stable" + "haftada 1 security"
        
        let current = read_cef_version()?;
        let latest = fetch_latest_cef_release("tauri-apps", "cef-rs").await?;
        if latest > current {
            let critical_cves = ChromiumAdvisoryFeed::critical_since(current.release_date).await?;
            let trigger = if !critical_cves.is_empty() {
                CefUpdateTrigger::CriticalCveDetected(critical_cves)
            } else if last_update_was_long_ago() {
                CefUpdateTrigger::ScheduledMonthly
            } else {
                CefUpdateTrigger::ScheduledWeekly
            };
            Ok(Some(Self {
                current_version: current,
                latest_version: latest,
                download_url: latest.binary_url,
                sha256: latest.sha256,
                trigger,
            }))
        } else {
            Ok(None)
        }
    }
    
    pub async fn apply(&self) -> anyhow::Result<()> {
        // CEF DLL'lerini yeni versiyonla değiştir
        let bytes = download_with_sha256(&self.download_url, &self.sha256).await?;
        cef::install_update(&bytes)?;
        
        // Kullanıcıya bildirim (trigger'a göre):
        match &self.trigger {
            CefUpdateTrigger::CriticalCveDetected(cves) => {
                tray.show_notification(
                    "Viscos: Kritik Chromium güvenlik güncellemesi",
                    &format!("{} kritik CVE yaması uygulandı: {}", cves.len(), 
                        cves.iter().map(|c| c.id.as_str()).collect::<Vec<_>>().join(", ")),
                )?;
            }
            CefUpdateTrigger::ScheduledMonthly => {
                tray.show_notification(
                    "Viscos: Aylık CEF güncellemesi",
                    "Chromium runtime güncellendi.",
                )?;
            }
            _ => {}
        }
        Ok(())
    }
}
```

**Güncelleme stratejisi (Haziran 2026 — ADR-0012 §CefUpdate):**
- **Routine haftalık kontrol:** Faz 1.5'teki `ChromiumAdvisoryFeed` her Pazartesi scrape eder, yeni stable + security post'ları varsa kullanıcıya bildiririm.
- **Kritik CVE tetikleyici:** Son 7 günde `Severity: Critical` CVE çıkarsa → CEF stable'da fix olmasa bile **acil update** tetiklenir, kullanıcıya bildirim.
- **Monthly baseline:** Faz 8.5 plan'ındaki "ayda bir" korunur, routine major version takibi.
- Kullanıcı bildirim: "CEF Chromium güncellemesi mevcut (N CVE)" + "Uygula ve Yeniden Başlat" butonu.
- Restart gerekli, kullanıcı schedule edebilir (Ayarlar → Backend → Güncelleme zamanı).

**Trade-off:** Faz 8.5 plan'ında "ayda bir" önerilmişti. ADR-0012 haftalık + kritik CVE ile bunu **sıkılaştırır**. `cef-rs` upstream release cadence 3-4 hafta ama Chromium security CVE window'u dar. Kullanıcı transparency için 3 tier trigger (haftalık routine, kritik CVE, aylık major).

---

## 7. CEF vs WebView2 Doküman Güncelleme

`docs/CEF-VS-WEBVIEW2.md` Faz 1.6 + Faz 8.5 verileriyle güncellenir:

```markdown
# WebView2 vs CEF — v1.5 sonrası kullanıcı feedback'i

## Sık sorulan sorular

### Neden CEF default 240 MB?
Chromium runtime gömülü (Edge ile paylaşılmıyor). Avantaj: 7/24 leak'siz, RDP güvenli.

### CEF Chromium güncel mi?
Self-update ile ayda bir kontrol edilir. WebView2 gibi otomatik değil ama güvenlik patch'leri yetişir.

### WebView2'ye nasıl dönerim?
Ayarlar > Backend > WebView2 seçin, restart. Win11 kullanıyorsanız GDI leak riskini kabul ediyorsunuz.

### RDP üzerinden çalışmıyor
CEF önerilir. WebView2 RDP'de GDI region leak yapar (Microsoft bug, düzeltilmedi).
```

---

## 8. Test Stratejisi (Faz 8.5)

| Test | Tip | Kabul |
|------|-----|-------|
| Backend UI (iced) | Integration | Seçim/restart doğru |
| Backend CLI `viscos-backend set` | Integration | config.toml yazılıyor |
| Chromium flags | Unit | Deny-list doğru |
| CEF cache clear | Integration | `IpcCommand::ClearCefCache` çalışıyor |
| CEF self-update | Integration (mock) | Update flow tamamlanıyor |
| Benchmark release notes | Manuel | Tablo doğru |

---

## 9. Kabul Kriterleri (Faz 8.5)

- [ ] Ayarlar > Backend UI çalışıyor (iced)
- [ ] WebView2 ↔ CEF geçiş wizard'ı tamamlanıyor
- [ ] Chromium flags config desteği + deny-list çalışıyor
- [ ] CEF cache stratejisi (clear_on_exit, max_size_mb) çalışıyor
- [ ] CEF self-update mock ile test ediliyor
- [ ] `docs/CEF-VS-WEBVIEW2.md` v1.5 feedback'iyle güncel
- [ ] Benchmark raporu release notes'a ekleniyor
- [ ] CI dual pipeline hâlâ çalışıyor

---

## 10. Karar Noktası (Faz 8.5 Sonu)

> 🔵 **İNSAN:** Backend UI varsayılan görünür mü?
> - Görünür (önerilen): Kullanıcı transparency, kontrol
> - Gelişmiş ayarlarda: Sıradan kullanıcı görmez, power user erişir

> 🔵 **İNSAN:** CEF self-update agresifliği?
> - Ayda bir (önerilen): Chromium güvenlik yaması
> - Haftalık: Chromium minor sürümleri, riskli
> - Quarterly: LTS yaklaşımı, major gap riski

> 🔵 **İNSAN:** Backend geçişi onay dialog'lu mu?
> - Sessiz (önerilen): Hızlı UX
> - Onay dialog: Kullanıcı bilgilendirilir

---

## 11. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| CEF self-update başarısız | Eski sürümde kalır | sha256 verify, rollback path |
| Chromium flags yanlış set | CEF crash | Deny-list, CI smoke test |
| Cache çok büyük | Disk | max_size_mb cap, retention policy |
| Backend UI karmaşıklığı | UX | Sade + "advanced" toggle |

---

## 12. Çıkış → v1.6+ veya v2.0

Bu faz tamamlandığında:
- Kullanıcı tam backend kontrolüne sahip
- CEF güncel kalır (self-update)
- Disk/disk alanı/pil dengesi iyi yönetilir

**v2.0:** Linux + macOS (ikisi de CEF), multi-account, native voice.
**v3.0:** Servo/Verso backend (olgunlaşırsa), GDI concerns tamamen tarih olur.
