---
name: Phase 8.0 — Distribution + Auto-update
overview: WebView2 periyodik refresh fine-tune, mouse hover throttling final ayar, pull-based IPC final audit, channel callback cleanup final check, performance profiling (WPR), memory leak audit (dhat), crash reporting (minidump, opt-in), auto-updater (GitHub Releases), code signing, MSI installer (cargo wix), WinGet manifest, user documentation.
isProject: false
todos:
  - id: webview-refresh
    content: WebView2 periyodik refresh (kanal değişimi veya watchdog tetikli)
    status: pending
  - id: throttle-final
    content: Mouse hover throttling final fine-tune (Faz 1.5 sonucuna göre)
    status: pending
  - id: ipc-audit-final
    content: Pull-based IPC final audit
    status: pending
  - id: callback-cleanup-final
    content: Channel callback cleanup final check
    status: pending
  - id: perf-profiling
    content: Performance profiling (Windows Performance Recorder)
    status: pending
  - id: leak-audit
    content: Memory leak audit (dhat)
    status: pending
  - id: crash-reporting
    content: Crash reporting (minidump, opt-in)
    status: pending
  - id: auto-updater
    content: Auto-updater (GitHub Releases, semver)
    status: pending
  - id: code-signing
    content: Code signing (Authenticode, sertifika gerekli)
    status: pending
  - id: msi-installer
    content: MSI installer (cargo wix) + WebView2 fixed version bundle
    status: pending
  - id: winget-manifest
    content: WinGet manifest
    status: pending
  - id: user-docs
    content: User documentation (README, FAQ)
    status: pending
---

# Phase 8.0 — Distribution + Auto-update

> **Süre:** 1–2 hafta
> **Hedef:** v1 release-ready: installer, auto-updater, code signing, performance polish, user docs.
> **Önceki faz:** [`phase-7.0-voice-video.md`](./phase-7.0-voice-video.md)
> **Sonraki faz:** [`phase-8.5-cef-backend.md`](./phase-8.5-cef-backend.md) (koşullu)

---

## 1. Workspace Dependencies

```toml
[workspace.dependencies]
# Auto-updater
self_update = { version = "0.41", features = ["rustls"] }
# Crash reporting
minidumper = "0.8"
# Memory profiling
dhat = "0.3"
# Profiling
cpuprofiler = "0.4"
# Installer
cargo-wix = "0.3"  # CLI tool, dep değil
```

---

## 2. WebView2 Bellek Yönetimi Final

### 2.1 Periyodik Refresh

```rust
// crates/viscos-webview/src/refresh.rs
use std::time::Duration;
use tokio::time::interval;

pub struct WebViewRefresher {
    config: RefreshConfig,
    webview: Arc<WebViewHandle>,
}

#[derive(Debug, Clone)]
pub struct RefreshConfig {
    pub channel_change: bool,    // Kanal değişiminde recreate
    pub watchdog_trigger: bool,  // Watchdog uyarısında recreate
    pub time_based: Option<Duration>, // Opsiyonel: 6 saatte bir
}

impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            channel_change: false,  // Faz 1 verisi: agresif recreate UX'i bozar
            watchdog_trigger: true, // Watchdog zaten recreate tetikliyor
            time_based: Some(Duration::from_secs(6 * 3600)),
        }
    }
}
```

**Karar verisi:** Faz 1'deki 24 saatlik soak test loglarına bak:
- 6 saatte bir refresh → GDI 8000'e ulaşmadan önce reset → %90 kullanıcı fark etmez
- Watchdog tetikli refresh → sadece kritik durumda

**Öneri:** Default time_based = 6 saat. Kullanıcı ayarlardan kapatabilir.

### 2.2 Microsoft Resmi Önerileri (Faz 1'den final)

| Öneri | Uygulama | Durum |
|-------|----------|-------|
| `MemoryUsageTargetLevel.Low` | İnaktif WebView'lerde | ✅ Faz 1'de |
| App-level process sharing | Tek WebView + tek environment | ✅ Faz 1'de |
| Periyodik WebView2 refresh | 6 saatte bir | 🆕 Faz 8 |
| `TrySuspend`/`Resume` | 30dk idle sonra | 🆕 Faz 8 |
| Monitor memory | WPR (debug only) | 🆕 Faz 8 |

```rust
// TrySuspend implementation
async fn try_suspend_on_idle(webview: &WebView, last_activity: Instant) {
    if last_activity.elapsed() > Duration::from_secs(1800) {
        unsafe {
            let core = webview.controller().CoreWebView2().unwrap();
            core.TrySuspend()?;
        }
        tracing::info!("WebView2 suspended (30min idle)");
    }
}
```

---

## 3. Performance Profiling

### 3.1 Windows Performance Recorder (WPR)

**Kullanım:**
```bash
# Profil başlat
wpr -start GeneralProfile -start CPU -start GPU

# 30 saniye uygulamayı kullan
# (kullanıcı normal scroll, click, type)

# Profil durdur
wpr -stop viscos-profile.etl
```

**Analiz:** WPA (Windows Performance Analyzer) ile ETL dosyasını aç, hot path'leri bul.

**Hedef:** 16ms frame budget (60fps). UI thread > 16ms → optimizasyon gerekli.

### 3.2 dhat (Heap Profiling)

```rust
// crates/viscos/src/main.rs (debug feature)
#[cfg(feature = "dhat")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn main() {
    #[cfg(feature = "dhat")]
    let _profiler = dhat::Profiler::new_heap();
    
    // ... normal main
}
```

**Çalıştırma:**
```bash
cargo run --features dhat --release
# 30 dakika normal kullanım
# dhat-heap.json üretilir
# https://github.com/nnethercote/dh_view için
```

**Hedef:** <%5 allocation overhead, <%10 peak memory regression release-to-release.

### 3.3 CPU Profiling (cargo-flamegraph)

```bash
cargo install flamegraph
cargo flamegraph --bin viscos
# 30 saniye scroll + click
# flamegraph.svg üretilir
```

---

## 4. Crash Reporting (Opt-in)

### 4.1 minidumper

```rust
// crates/viscos/src/crash_handler.rs
use minidumper::{Minidumper, Loop};

pub fn install_crash_handler() -> anyhow::Result<()> {
    let minidumper = Minidumper::new(
        std::path::Path::new("crash-dumps/"),
    )?;
    let loop_handle = minidumper.spawn()?;
    
    std::panic::set_hook(Box::new(move |info| {
        // minidump oluştur
        // logla
    }));
    
    Ok(())
}
```

### 4.2 Opt-in Telemetri

```rust
// crates/viscos-telemetry/src/lib.rs
pub struct Telemetry {
    enabled: bool,
    endpoint: String,
}

impl Telemetry {
    pub async fn report_crash(&self, dump: &[u8], metadata: CrashMetadata) {
        if !self.enabled { return; }
        // Sentry, kendi server, vb.
        // Privacy: token, message content ASLA gönderilmez
        // Sadece: stack trace, OS version, app version
    }
}
```

**First-launch dialog:**
```
[ ] Crash raporlarını otomatik gönder (anonim)
    [Anonim crash raporları gönderilmesine izin veriyorum]
    [Hayır, sadece lokal kaydet]
```

---

## 5. Auto-Updater

### 5.1 `crates/viscos-updater/src/lib.rs`

```rust
use self_update::backends::github::ReleaseList;
use self_update::update::ReleaseUpdate;
use self_update::version::bump_is_greater;

pub struct AutoUpdater {
    repo: String,  // "viscos/viscos"
    current_version: String,
}

impl AutoUpdater {
    pub async fn check(&self) -> anyhow::Result<Option<UpdateInfo>> {
        let releases = ReleaseList::configure()
            .repo_owner(&self.repo.split('/').next().unwrap())
            .repo_name(&self.repo.split('/').nth(1).unwrap())
            .build()?
            .fetch()?;
        
        for release in releases {
            if bump_is_greater(&self.current_version, &release.version)? {
                return Ok(Some(UpdateInfo {
                    version: release.version,
                    notes: release.body.unwrap_or_default(),
                    assets: release.assets,
                }));
            }
        }
        Ok(None)
    }
    
    pub async fn apply(&self, update: UpdateInfo) -> anyhow::Result<()> {
        // Binary download
        // Hash verify
        // Replace current exe
        // Restart
        todo!("v1'de self_update::backends::github kullan")
    }
}

pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub assets: Vec<Asset>,
}
```

### 5.2 UX

İlk açılış dialog:
```
Viscos 0.2.0 yayınlandı!
- Yeni özellik: ...
- Bug fix: ...

[Şimdi güncelle]    [Sonra]    [Atla]
```

Manuel check: Settings → About → "Güncelleme kontrol et".

### 5.3 Kanallar (ileride)

- `stable`: releases
- `beta`: pre-releases
- `nightly`: her commit

**v1'de sadece stable.**

---

## 6. Code Signing (Windows Authenticode)

### 6.1 Sertifika Seçenekleri

| Tip | Maliyet | Güven |
|-----|---------|-------|
| **OV (Organization Validation)** | $200-400/yıl | SmartScreen uyarısı yok |
| **EV (Extended Validation)** | $400-800/yıl | Anında SmartScreen trust |
| **Self-signed** | $0 | Kullanıcı "Unknown publisher" uyarısı görür |

**v1 önerisi:** Self-signed veya "unsigned" (kullanıcı SmartScreen "Run anyway" der). v2'de OV sertifikası al.

### 6.2 signtool.exe

```powershell
# Self-signed (test)
$cert = New-SelfSignedCertificate -Subject "CN=Viscos Dev" -Type CodeSigningCert
Set-AuthenticodeSignature -FilePath "viscos.exe" -Certificate $cert

# Production (signtool)
& 'C:\Program Files (x86)\Windows Kits\10\bin\x64\signtool.exe' sign `
    /tr http://timestamp.digicert.com `
    /td sha256 `
    /fd sha256 `
    /f "viscos.pfx" `
    /p $env:CERT_PASSWORD `
    "target\release\viscos.exe"
```

### 6.3 CI Integration

```yaml
# .github/workflows/release.yml
- name: Sign executable
  env:
    CERT_FILE: ${{ secrets.CODE_SIGNING_CERT }}
    CERT_PASSWORD: ${{ secrets.CERT_PASSWORD }}
  run: |
    signtool sign /f $CERT_FILE /p $CERT_PASSWORD /tr http://timestamp.digicert.com target/release/viscos.exe
```

**Önemli:** Sertifika GitHub Secrets'ta, asla commit edilmez.

---

## 7. MSI Installer (cargo wix)

### 7.1 `wix/main.wxs`

```xml
<?xml version="1.0" encoding="utf-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Viscos" Version="0.1.0" 
           Manufacturer="Viscos Contributors" 
           UpgradeCode="PUT-GUID-HERE">
    
    <Package InstallerVersion="500" Compressed="yes" InstallScope="perMachine" />
    
    <MajorUpgrade DowngradeErrorMessage="A newer version is installed." />
    <MediaTemplate EmbedCab="yes" />
    
    <Feature Id="ProductFeature" Title="Viscos" Level="1">
      <ComponentGroupRef Id="ProductComponents" />
      <ComponentRef Id="WebView2Runtime" />
    </Feature>
    
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFilesFolder">
        <Directory Id="INSTALLFOLDER" Name="Viscos" />
      </Directory>
    </Directory>
    
    <ComponentGroup Id="ProductComponents" Directory="INSTALLFOLDER">
      <Component Id="ProductComponent">
        <File Id="ViscosExe" Source="target/release/viscos.exe" KeyPath="yes" />
      </Component>
    </ComponentGroup>
    
    <!-- WebView2 Runtime -->
    <Component Id="WebView2Runtime" Directory="INSTALLFOLDER" Guid="PUT-GUID">
      <File Id="WebView2Installer" Source="assets/MicrosoftEdgeWebview2Setup.exe" />
    </Component>
  </Product>
</Wix>
```

### 7.2 `cargo wix` ile build

```bash
cargo install cargo-wix
cargo wix init
cargo wix --nocapture
# → target/wix/viscos-0.1.0-x86_64.msi
```

### 7.3 WebView2 Fixed Version

MSI içine WebView2 Runtime'ın belirli bir versiyonunu bundle et (kullanıcının sisteminde yoksa kur):

```xml
<CustomAction Id="InstallWebView2" 
              FileKey="WebView2Installer"
              ExeCommand="/silent /install" />
<InstallExecuteSequence>
  <Custom Action="InstallWebView2" After="InstallFiles">NOT Installed</Custom>
</InstallExecuteSequence>
```

---

## 8. WinGet Manifest

`winget/viscos.viscos.yaml` (PR: microsoft/winget-pkgs):

```yaml
PackageIdentifier: viscos.viscos
PackageVersion: 0.1.0
PackageLocale: en-US
Publisher: Viscos Contributors
PackageName: Viscos
License: GPL-3.0
ShortDescription: Lightweight native Discord client
Moniker: viscos
Tags:
  - discord
  - discord-client
  - native
  - rust
Installers:
  - Architecture: x64
    InstallerType: msi
    InstallerUrl: https://github.com/viscos/viscos/releases/download/v0.1.0/viscos-0.1.0-x86_64.msi
    InstallerSha256: <SHA256>
ManifestType: defaultLocale
```

**Submission:** microsoft/winget-pkgs repo'suna PR aç, otomatik review.

---

## 9. User Documentation

### 9.1 `README.md`

```markdown
# Viscos

> Lightweight native Discord client for Windows. Built with Rust + iced + WebView2.

## Features
- ~200-300 MB RAM (vs Discord 500-1500 MB)
- < 2s cold start
- 15-25 MB binary
- Native side panel (iced)
- Vencord plugin support
- Auto-updater

## Installation

### WinGet (recommended)
\`\`\`powershell
winget install viscos.viscos
\`\`\`

### MSI
Download from [Releases](https://github.com/viscos/viscos/releases).

## Disclaimer
Viscos is a third-party Discord client using user tokens. Discord may ban accounts
that use automated clients. Use at your own risk.

## Development
See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).

## License
GPL-3.0
```

### 9.2 `docs/FAQ.md`

- "Viscos self-bot mi?" → Hayır, user token. Discord ToS riski var.
- "Vesktop'tan farkı?" → Tauri'siz, native side panel, daha küçük binary.
- "Vencord uyumlu mu?" → Evet, plugin yüklenebilir.
- "Linux desteği?" → v2'de planlanıyor.
- "Ses/görüntü?" → v1'de WebView2 üzerinden (Discord native), native v1.5+.

---

## 10. Test Stratejisi (Faz 8.0)

| Test | Tip | Kabul |
|------|-----|-------|
| WPR profiling | Manuel (lokal) | 60fps hedefi |
| dhat heap | Manuel (lokal) | <%5 alloc overhead |
| Crash minidump | Integration | Minidump dosyası üretiliyor |
| Auto-updater | Integration (mock) | Yeni version tespit + download |
| MSI install/uninstall | Integration (Windows) | Add/Remove Programs'ta temiz |
| WinGet install | Manuel | `winget install viscos.viscos` çalışıyor |
| 24 saatlik final soak | Lokal | Crash yok, RAM < 350MB, leak <%5 |

---

## 11. Kabul Kriterleri (v1 Release)

- [ ] `cargo build --release` → binary < 25MB
- [ ] MSI installer oluşturuluyor ve temiz kuruluyor
- [ ] Code signing (self-signed minimum) çalışıyor
- [ ] Auto-updater ilk run'da update kontrol ediyor
- [ ] Crash rapor opt-in dialog
- [ ] WPR profili temiz (60fps)
- [ ] dhat raporu temiz
- [ ] 24 saatlik final soak: crash yok, RAM < 350MB
- [ ] WinGet manifest submit edildi
- [ ] README + FAQ yayında
- [ ] v0.1.0 tag'i GitHub'da
- [ ] **Tüm AI workflow metrikleri tutuldu** (master Bölüm 4):
  - [ ] Coverage > %80
  - [ ] Clippy 0 warning
  - [ ] Memory regression < %5

---

## 12. Karar Noktası (Faz 8.0 Sonu)

> 🔵 **İNSAN:** Faz 8.5 (CEF backend) aktif edilsin mi?
> - **HAYIR (önerilen v1):** Win11 leak watchdog ile yönetilebilir, kullanıcı şikayeti gelince Faz 9 olarak ekle
> - **EVET:** Win11 kullanıcılarına CEF release, +200MB binary

> 🔵 **İNSAN:** Code signing stratejisi?
> - Self-signed (v1, ücretsiz, uyarı var)
> - OV sertifika (yıllık $200-400, SmartScreen OK)
> - EV sertifika (yıllık $400-800, anında trust)

> 🔵 **İNSAN:** Auto-updater agresifliği?
> - Sessiz: arka planda indir, kapat'ta kur
> - Sor: her release'te dialog
> - Disabled: kullanıcı manuel

> 🔵 **İNSAN:** Crash reporting?
> - Opt-in (önerilen, GDPR uyumlu)
> - Opt-out (default açık)
> - Off (lokal kayıt)

---

## 13. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| WebView2 update kırılması | Çalışmama | Edge stable channel'a pin, fallback |
| Code signing sertifika expired | SmartScreen | Otomatik renewal alarm, 30 gün önceden |
| Auto-updater başarısız | Eski sürüm | Atomic replace, rollback |
| MSI install hatası | Kullanıcı yükleyemez | Detaylı log, support link |
| Crash report PII leak | GDPR ihlali | Metadata only, opt-in |

---

## 14. Çıkış → Faz 8.5 (koşullu) veya v1.0

Eğer Faz 8.5 koşullu değilse:
- v1.0 release
- WinGet + GitHub Releases
- Marketing: r/Discord, Hacker News, kendi Discord sunucusu

Faz 8.5 → CEF backend opsiyonel.
