---
name: Viscos — Master Index
overview: Viscos Discord client projesinin master index dosyası. Vizyon, mimari özet, faz sırası ve cross-cutting kararlar burada. Faz detayları ilgili sub-plan dosyalarında.
isProject: true
---

# Viscos: Yüksek Performanslı Hibrit Discord İstemcisi

> **Master index.** Faz detayları için ilgili `phase-X.Y-*.md` dosyasına bak.
> Cross-cutting konular: WebView2 hardening → `webview2-hardening.md`, AI workflow → aşağıda Bölüm 4.

## 1. Vizyon (Tek Sayfa)

**Viscos**, Windows 10/11 için optimize edilmiş, Rust ile yazılmış hibrit bir Discord istemcisidir. Resmi Discord istemcisinin (Electron) ağır yanlarını ortadan kaldırırken Discord'un zengin web UI'ını (animated emoji, video, voice, screen share) korur.

**Mimari:**
- **Native Rust shell** (`tao` + `iced`) — pencere, tray, side panel, klavye, autocomplete
- **WebView2** (varsayılan Win10) — Discord'un React web client'ını çalıştırır
- **CEF backend** (Faz 1.6 — Win11 default, MVP'nin parçası) — Microsoft WebView2 GDI leak'ten yapısal kaçış
- **IPC köprüsü** — pull-based, Rust → JS push yok (küçük olaylar hariç)

> **Frontend mimari kararının tam gerekçesi:** [`docs/DECISIONS.md` ADR-0012](../DECISIONS.md) (Haziran 2026). Hibrit (native shell + WebView2/CEF + Discord web) seçildi çünkü: tam native UI (kind/Acheron) yıllar alır + AI-risk yüksek + DAVE E2EE browser'da bedava + animated WebP/Lottie Discord zaten yapıyor + bridge.ts selector resilience kuralları Discord DOM churn'inden koruyor.

**Hedef metrikler (resmi Discord ile):**
| Metrik | Viscos | Resmi Discord |
|--------|--------|---------------|
| RAM (idle) | 150–300 MB | 500–1500 MB |
| Cold start | < 1.5 s | 3–6 s |
| Binary | 15–25 MB | 150 MB+ |
| CPU idle | < 1 % | 2–5 % |
| GDI objesi | < 5000 (watchdog 8000'de) | — |

**İş akışı:** Kod %100 AI (Cursor agent) tarafından yazılır, insan mimari karar + review + acceptance test yapar. Detay: Bölüm 4.

---

## 2. Faz Yol Haritası

| Faz | Dosya | Süre | Açıklama | Öncelik |
|-----|-------|------|----------|---------|
| 0.0 | `phase-0.0-foundation.md` | 1–2 hf | Cargo workspace, CI, config | Kritik |
| 0.5 | `phase-0.5-ai-workflow-setup.md` | 3–5 g | `.cursorrules`, task templates, PR template | Kritik |
| 1.0 | `phase-1.0-window-webview.md` | 2–3 hf | tao+wry+iced 0.14, **GDI watchdog (7000/9000 + draft autosave)** | Kritik |
| 1.5 | `phase-1.5-telemetry-and-restart-optimization.md` | 1 hf | Telemetry backend, IPC audit, channel cleanup | Yüksek |
| 1.6 | `phase-1.6-cef-default-rollout.md` | 1–2 hf | **Win11 default CEF + Win10 default WebView2** (koşullu, MVP'nin parçası) | Kritik |
| 2.0 | `phase-2.0-discord-api.md` | 2–3 hf | REST + auth + keyring-core + MFA TOTP+backup; **ADR-0011** | Kritik |
| 3.0 | `phase-3.0-gateway.md` | 2–3 hf | WebSocket + zstd + resume | Kritik |
| 4.0 | `phase-4.0-cache-media.md` | 2 hf | SQLite + moka + foyer | Yüksek |
| 5.0 | `phase-5.0-native-ui.md` | 3–4 hf | iced side panel + Vencord POC | Yüksek |
| 6.0 | `phase-6.0-hotkeys.md` | 1 hf | Hotkeys, deep linking, single-instance | Orta |
| 7.0 | `phase-7.0-voice-video.md` | 3+ hf | DAVE E2EE (opsiyonel, **v1'de atlanır**) | Düşük |
| 8.0 | `phase-8.0-distribution.md` | 1–2 hf | Auto-update, signing, MSI, WinGet | Yüksek |
| 8.5 | `phase-8.5-cef-backend.md` | 2 hf | CEF default-out yönetim (UI + Chromium flags + self-update) | Koşullu |

**WebView2 hardening** tüm bu fazlarda geçerli → `webview2-hardening.md`.

**Haziran 2026 değişiklik notu (Faz 1.5, Faz 1.6, Faz 8.5):**
- **Faz 1.5 yeniden adlandırıldı:** `phase-1.5-mouse-throttling.md` → `phase-1.5-telemetry-and-restart-optimization.md`. Mouse hover throttling **kanıtlanmış etkisiz** (Microsoft issue #5536, DirectComposition bypass). Yerine telemetry + restart optimizasyonu.
- **Faz 1.6 eklendi (MVP'nin parçası):** CEF backend Win11 default olarak MVP'ye alındı. Önce Faz 8.5'te "koşullu opt-in" idi; Microsoft upstream bug yapısal çözümsüz olduğu için öne çekildi.
- **Faz 8.5 yeniden tanımlandı:** "Koşullu CEF opt-in" → "Default-out yönetim (UI + flags + self-update)". CEF artık Win11 için default, Faz 8.5 yönetim katmanını ekler.

Detay: [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md).

**Toplam v1 (Faz 7 hariç):** ~ 16–22 hafta (4–5.5 ay), 1 kişilik ekip + AI agent.

---

## 3. Proje Yapısı (Workspace)

```
viscos/
├── Cargo.toml                  # Workspace
├── README.md
├── LICENSE                     # GPL-3.0
├── .cursorrules                # AI agent kuralları (Faz 0.5)
├── .cursor/
│   ├── plans/                  # Bu dosyalar
│   │   ├── viscos_index.md     # Master
│   │   ├── webview2-hardening.md
│   │   └── phase-*.md
│   └── tasks/                  # AI task template'leri (Faz 0.5)
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   ├── release.yml
│   │   └── ai-task-validate.yml
│   └── PULL_REQUEST_TEMPLATE.md
├── crates/
│   ├── viscos-core/            # Domain types, events (no I/O)
│   ├── viscos-api/             # Discord REST + Gateway
│   ├── viscos-cache/           # SQLite + moka + foyer
│   ├── viscos-media/           # Resim, video, ses cache
│   ├── viscos-shell/           # iced GUI, pencere yönetimi
│   ├── viscos-webview/         # WebViewBackend trait (wry/CEF)
│   ├── viscos-watchdog/        # GDI/IPC/heap izleme
│   ├── viscos-ipc/             # Native ↔ Web köprüsü (pull-based)
│   ├── viscos-auth/            # Token, keyring-core, MFA TOTP+backup, secrecy, zeroize (ADR-0011)
│   ├── viscos-config/          # config-rs wrapper (12-factor layered)
│   └── viscos/                 # Ana binary
├── frontend/                   # WebView2 içi TS wrapper
│   ├── src/bridge.ts
│   ├── src/patches/
│   └── src/styles/
└── docs/
    ├── ARCHITECTURE.md
    ├── PERFORMANCE.md
    ├── DEVELOPMENT.md
    ├── AI-WORKFLOW.md
    └── DECISIONS.md            # ADR
```

**Crate bağımlılık kuralları:**
- `core` → bağımlılığı yok (sadece std + serde)
- `api`, `cache`, `auth`, `config` → `core`'a bağımlı
- `shell`, `webview`, `watchdog`, `ipc` → `core` + gerekli diğer crate'ler
- `viscos` (binary) → hepsini bağlar

---

## 4. AI-Yazar, İnsan-Onay İş Akışı (Cross-Cutting)

> Kodun %100'ü AI (Cursor agent) tarafından üretilir. İnsan yalnızca **mimari karar**, **kod review**, **acceptance test** ve **proje yönü** belirler.

### 4.1 Felsefe
**"AI yazar, insan karar verir, AI yazar."** İnsan "ne" ve "neden" sorusunu sorar, AI "nasıl"ı yapar.

### 4.2 İnsanın Rolü
1. Mimari kararlar (backend, modül, dependency)
2. Önceliklendirme (hangi feature önce)
3. Trade-off değerlendirmesi (RAM vs disk, hız vs okunabilirlik)
4. Kod review (her PR'da mimari tutarlılık, edge case)
5. Acceptance test (lokal build, gerçek hesap)
6. Release kararları
7. Etik sınırlar (ToS uyumu, gizlilik)

### 4.3 AI'ın Rolü
1. Kod yazma (feature, bugfix, refactor)
2. Test yazma (unit, integration)
3. Dokümantasyon (rustdoc, ADR taslağı)
4. Code review (ilk tur: lint, format, basit bug)
5. Refactoring, boilerplate, benchmark yazımı

### 4.4 Tipik Feature Akışı
```
1. İNSAN: Issue açar + label'lar
2. AI: Branch açar, kodu okur, implement eder, test yazar
3. AI VALIDATION (CI): cargo test, clippy, fmt, coverage
4. İNSAN: PR review (mimari, UX, edge case, acceptance)
5. AI: Review comment'lerine göre düzeltme
6. İNSAN: Onay + merge
```

### 4.5 Karar Matrisi (Kim Ne Verir?)

| Karar Türü | İnsan | AI |
|------------|-------|-----|
| Mimari (backend, modül) | ✅ | ❌ |
| Dependency seçimi (versiyon) | ✅ onay | ✅ öneri |
| Public API shape | ✅ onay | ✅ öneri |
| Implementasyon detayı | ❌ | ✅ |
| Test stratejisi | ❌ | ✅ |
| Bug fix | ❌ | ✅ (insan review) |
| Trade-off kararı | ✅ | ❌ |
| Önceliklendirme | ✅ | ❌ |
| Release zamanlaması | ✅ | ❌ |
| Etik sınır (ToS) | ✅ | ❌ |
| Mimari refactor | ✅ onay | ✅ öneri |

### 4.6 AI'ın Hard Limitleri
- ❌ Breaking API değişikliği (insan onayı olmadan)
- ❌ Yeni dependency (insan onayı olmadan)
- ❌ Mimari karar (sadece öneri)
- ❌ ToS ihlali şüphesi
- ❌ Release publish (sadece PR hazırlar)
- ❌ Code signing (private key'e erişemez)
- ❌ Production secret (env var veya hardcode)
- ❌ Büyük refactor > 500 satır (parçalamalı)

### 4.7 AI'ın Otomatik Red Flag'leri (CI fail)
| Durum | Tespit | Aksiyon |
|-------|--------|---------|
| `eval_script` payload > 10KB | payload size check | Pull-based pattern ihlali → fail |
| `unsafe { }` (dokümante edilmemiş) | `cargo geiger` | İnsan review zorla |
| Public API breaking change | `cargo semver-checks` | Major bump gerekli |
| Yeni dependency | `Cargo.toml` diff | İnsan onayı zorunlu |
| DB schema change | `migrations/` diff | Migration + rollback testi |
| `unwrap()` in production | clippy | Test'te OK, runtime'da yasak |
| `println!` / `dbg!` | clippy | tracing kullan |
| Dosya > 400 satır | file size check | Refactor önerisi |
| `todo!()` / `unimplemented!()` | grep | Issue açılmadan merge yok |
| GDI watchdog bypass | grep | İnsan review zorunlu |
| **Bilinen güvenlik açığı (transitive dep)** | `cargo audit` (haftalık + PR) | Justifiye `ignore` + issue linki yoksa fail |
| **Lisans uyumsuzluğu** | `cargo deny check licenses` | GPL/AGPL/LGPL default deny; allow list dışı dep PR fail |
| **Güvenilmeyen kaynak** | `cargo deny check sources` | `crates.io` dışı registry / git dep PR fail (izin verilenler hariç) |
| **Yasaklı crate** | `cargo deny check bans` | Banned crate eklenirse fail |
| **Binary bütçesi aşımı** | CI size job (25 MB) | Hedef metrikleri korumak için fail |
| **Clippy pedantic/nursery** | `cargo clippy -- -D warnings` | AI'a stil rehberi; insan review `#[allow]` kontrol eder |

Detaylı workflow dosyaları: `phase-0.5-ai-workflow-setup.md`.

---

## 5. Mimari Akış (Tek Diyagram)

```
┌─────────────────────────────────────────────────────────────┐
│                       Viscos Process                         │
│                                                              │
│  ┌─────────────────────┐        ┌────────────────────────┐  │
│  │  viscos-shell       │        │  viscos-webview        │  │
│  │  (iced 0.14, native)│ ◄───►  │  (wry/CEF, WebView)    │  │
│  │                     │   IPC  │                        │  │
│  │  - Sunucu listesi   │ (pull) │  - Discord web client  │  │
│  │  - Kanal listesi    │        │  - Mesaj render        │  │
│  │  - Üye listesi      │        │  - Voice/Video (Faz 7) │  │
│  │  - Tray + Hotkey    │        │  - Ekran paylaşımı     │  │
│  │  - Bildirimler      │        │  - Animated emoji      │  │
│  └──────────┬──────────┘        └────────────┬───────────┘  │
│             │                                │               │
│             └────────────┬───────────────────┘               │
│                          │                                    │
│             ┌────────────▼────────────┐                      │
│             │     viscos-ipc          │                      │
│             │  (pull-based, serde)    │                      │
│             └────────────┬────────────┘                      │
│                          │                                    │
│  ┌───────────────────────▼────────────────────────────────┐ │
│  │              viscos-core (state, events)                 │ │
│  └────┬──────────┬──────────┬──────────┬──────────────────┘ │
│       │          │          │          │                      │
│  ┌────▼────┐ ┌───▼────┐ ┌──▼────┐ ┌───▼────┐               │
│  │  api    │ │ cache  │ │ media │ │ auth   │               │
│  │ REST+   │ │ SQLite │ │ RAM+  │ │token,  │               │
│  │ Gateway │ │ +moka  │ │ disk  │ │keyring │               │
│  └─────────┘ └────────┘ └───────┘ └────────┘               │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  viscos-watchdog (Faz 1'den, sürekli çalışır)         │   │
│  │  - GDI object counter (her 30s, threshold 7000/9000)   │   │
│  │  - IPC buffer size monitor                            │   │
│  │  - Heap fragmentation tracker (jemalloc stats)        │   │
│  │  - Draft autosave pre-restart hook (mesaj kaybı 0)    │   │
│  │  - Auto-restart tetikleyici (soft, WebView recreate)  │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  viscos-telemetry (Faz 1.5'ten, SQLite, opt-out)      │   │
│  │  - GDI time-series, restart events                    │   │
│  │  - 30 gün rolling retention, 100MB cap                │   │
│  │  - CefRolloutRecommendation (Faz 1.6 tetikleyici)     │   │
│  │  - Tray icon "X restart today" badge                  │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  WebView Backend (Faz 1.6 — MVP'nin parçası):               │
│    - Win11 default → CEF (Chromium, leak'siz)               │
│    - Win10 default → WebView2 (wry, hafif)                  │
│    - select_default_backend() telemetry override'ı destekler│
└──────────────────────────────────────────────────────────────┘
```

---

## 6. Teknik Kararlar (Özet)

| Karar | Seçim | Gerekçe |
|-------|-------|---------|
| Dil | Rust 1.89+ Edition 2024 | Performans + güvenlik; 1.89 = twilight-rs MSRV ile hizalı (ADR-0006) |
| GUI Framework | **iced 0.14** | Son deneysel sürüm (1.0 freeze öncesi), reactive rendering default, COSMIC resize lag çözüldü, wgpu GPU, native |
| Pencere | `tao` 0.35 | OS-native event loop, tray + menu + hotkey built-in, Leto Discord client kanıtı |
| WebView (default Win10) | `wry` 0.55 → WebView2 | Tauri'nin altındaki kanıtlanmış lib, 15–25 MB binary |
| WebView (default Win11, Faz 1.6) | **`cef-rs` (CEF)** | Microsoft WebView2 GDI leak (issue #5536) yapısal çözümsüz, CEF leak'siz |
| WebView (opt-out) | `WebView2` (kullanıcı isterse) | Disk alanı kısıtlı Win11 kullanıcıları için, agresif watchdog eşliğinde |
| Backend abstraction | `WebViewBackend` trait | Pluggable (gelecekte Servo), `select_default_backend()` telemetry-driven |
| Discord API | **twilight-rs 0.17** (twilight-model + twilight-http + twilight-gateway) | Sıfırdan reqwest + tokio-tungstenite + manual zstd yazmak AI için riskli + Discord protocol drift yükü; twilight Discord-breaking change'lere 24 saat içinde cevap veriyor; ADR-0008 |
| **Auth token storage** | **`keyring-core 0.7` + `windows-native-keyring-store 1.1`** (Haziran 2026) | `keyring 2.3` stale, 4.0 mimarisine geçildi; DPAPI arkası; `default-features=false` ile `regex` dependency yok (~1+ MB tasarruf); **ADR-0011** |
| **Bellek hijyeni** | **`secrecy 0.10` + `zeroize 1`** (Haziran 2026) | `Secret<String>` + `ZeroizeOnDrop` tüm token path'lerinde zorunlu; memory dump baseline savunma; `expose_secret()` audit-grep; **ADR-0011** |
| **Encryption stratejisi** | **Varyant A (DPAPI/Keyring) default, Varyant B (Argon2id passphrase) v2.0 opt-in** (Haziran 2026) | Threat model %95 yeterli, passphrase UX öldürür; **ADR-0011** |
| Cache RAM | moka | TinyLFU, async (ADR-0010 korundu, alternatifler elendi) |
| Cache Disk | foyer | Hybrid memory+disk (**0.10→0.22 minor bump**, ADR-0010) |
| DB | SQLite WAL + rusqlite | Standard, hızlı (**0.32→0.38 minor bump**, ADR-0010) |
| Encryption | AES-GCM (chacha20poly1305 çıkarıldı, ADR-0010) | Win10/11 AES-NI hardware acceleration, +60 KB tasarruf |
| Compression | zstd | Discord'un kullandığı |
| Allocator (v1) | system | Yeterli, jemalloc Faz 4'te benchmark |
| Allocator (koşullu) | jemalloc | Svix case study |
| **Watchdog threshold** | **7000/9000 GDI** (Haziran 2026) | Win11 leak pattern'ine göre: erken uyarı, restart gap <2s |
| **Pre-restart draft autosave** | **Evet** (Faz 1'den) | Mesaj kaybı 0, watchdog restart öncesi hook |
| **Telemetry backend** | **SQLite, opt-out** (Faz 1.5) | Restart trendleri, CEF default karar veriye dayalı |
| IPC pattern | **Pull-based** | WebView2 buffer şişmesini önler |
| Büyük blob transfer (Faz 4+) | **WebView2 SharedBuffer / CEF SharedMemoryRegion** | Zero-copy, tauri#13758 upstream bug'ından yapısal kaçış |
| **Async runtime** | **tokio (granular features)** | `full` yerine rt-multi-thread + macros + sync + time + fs + net + io-util → %30-40 daha hızlı compile |
| **Config** | **config-rs (config 0.14)** | Aktif bakım, 12-factor standard; figment May 2024'ten beri stale |
| Logging | tracing + tracing-subscriber | Async span/event/context; WebView2 IPC correlation için kritik |
| Logging (Faz 1+) | tracing-appender | 24 saat soak test'inde log kaybı olmasın |
| Error (library) | thiserror + `#[non_exhaustive]` | Typed, matchable, evrimsel güvenli |
| Error (application) | anyhow | Glue code + main; library boundary'de sızıntı yok |
| Lint (CI) | clippy pedantic + nursery, deny warnings | AI agent'a stil rehberi |
| CI | GitHub Actions (windows-latest) + 2 katmanlı cache | Swatinem (registry) + sccache (compiler artifact) |
| CI jobs | fmt, clippy, nextest, build, audit, deny, geiger + 25 MB size gate | Binary bütçesi + lisans uyumu + güvenlik |
| License | GPL-3.0 | Dissent gibi |
| Mimari | Hibrit | Tüm özellikleri sıfırdan yazmak yıllar alır |
| Tauri | HAYIR | Leto kanıtladı: tao+wry yeterli |
| Kod yazarı | AI (Cursor agent) | Hız + tutarlılık |
| Karar veren | İnsan | Mimari, review, acceptance |

**Değişiklik gerekçeleri (Faz 0.0 araştırması, Haziran 2026):**
- `figment` → `config-rs`: Stale (May 2024'ten beri güncelleme yok). 4 yıllık AI-agent projesi boyunca upstream güncellemesi alamamak technical debt olur. `figment2` topluluk fork'u olarak aktif; `config-rs` ise endüstri standardı.
- `tokio/full` → granular features: `full` gereksiz driver'ları (process, signal, io-std) çeker, compile time %30-40 artırır, binary büyütür.
- `rust-version = "1.80"` → `"1.85"`: Edition 2024 stabil + modern cargo resolver.
- `lto = "thin"` → `lto = "fat"` + `panic = "abort"`: 15-25 MB binary hedefi için kritik; Discord client GUI app → panic sonrası process yeniden başlatılır (watchdog Faz 1).
- `cargo test` → `cargo nextest`: Paralel + izole, ~2-3x hızlı; flaky network test'leri için `--retries 2`.
- CI'a eklendi: `cargo-audit` (haftalık RustSec), `cargo-deny` (lisans + source + ban), `cargo-geiger` (unsafe raporu), 25 MB size gate.

**Değişiklik gerekçeleri (Haziran 2026 trade-off analizi, Faz 1 + 1.5 + 1.6):**
- **`iced = "0.13"` → `"0.14"`**: Son deneysel sürüm (1.0 freeze öncesi). Reactive rendering default, COSMIC resize lag çözüldü (`pop-os/libcosmic#753`).
- **GDI threshold 5000/8000 → 7000/9000**: Win11 leak pattern'ine göre (Haziran 2026, ~4000 GDI/30sn). Restart gap <2s hedefi.
- **Faz 1.5 yeniden adlandırıldı**: Mouse throttle **kanıtlanmış etkisiz** (Microsoft issue #5536, DirectComposition bypass). Yerine telemetry + restart optimizasyonu. Kullanıcı şeffaflığı (tray badge) + auto-restart agresifleştirme.
- **Faz 1.6 eklendi (MVP'nin parçası)**: CEF backend Win11 default olarak öne alındı. Microsoft upstream bug yapısal çözümsüz, MVP'de Win11 kullanıcıları leak'ten korunmalı.
- **Pre-restart draft autosave hook**: Restart öncesi mesaj taslakları SQLite'a yazılır, kullanıcı mesaj kaybı 0.
- **Frontend mimari** | **Hibrit (WebView2/CEF + native iced shell)** (Haziran 2026 — ADR-0012) | Tam native UI (kind/Acheron) yıllar alır + AI-risk + ToS gri bölge; hibrit kanıtlanmış (Dorion/Leto/Vesktop), DAVE E2EE browser'da bedava, animated WebP/Lottie Discord zaten yapıyor, bridge.ts selector resilience kuralları + anti-bot heuristic parite koruması ile |
- **Cache stack minor bump + cleanup + 2 yeni strateji (ADR-0010)**: Detaylı araştırma [`cache-stack-research.md`](./cache-stack-research.md). (1) `rusqlite 0.32 → 0.38` (Aralık 2025, 2 yıllık patch birikimi, API uyumlu). (2) `foyer 0.10 → 0.22` (Ocak 2026, 12 minor versiyon olgunlaşma, hybrid engine production-grade, RisingWave+Chroma+SlateDB kanıtı). (3) `chacha20poly1305` çıkarıldı (Win10/11 AES-NI yaygın, +60 KB binary tasarruf, kod sadeleşmesi). (4) **Stretto PoC stretch goal olarak Faz 4 sonuna eklendi** (cachebench OLTP trace'inde %33–47 pp hit ratio potansiyeli; scan-heavy trace'de %57 pp kayıp riski — gerçek workload benchmark sonucu **>15 pp** ise v2 backlog'a al). (5) **Redb, SQLx, sled, Limbo, CacheLib Rust binding, BlobCache, Possum elendi** (gerekçeler: SQL yok / async perf regression / alpha churn / çok erken / C++ build / Rust port yok / multi-process gereksiz). **(6) Discord CDN Content-Addressable cache key stratejisi (Haziran 2026 eki)** — `attachment_id` (snowflake u64) → foyer KV, 24h signed URL limit'i cache ömrünü sınırlamaz. Background refresh worker (23h < expires_at < 24h aralığı, 50 URL per call, rate-limit korumalı). **(7) Adaptive Tier Sizing (Haziran 2026 eki)** — Faz 1.5 telemetry-driven tier tuning, v1 default'ları statik, v1.5'te hit ratio thresholds ile otomatik ayarlama.

- **Auth stack değişikliği (Haziran 2026 — ADR-0011, detay [`viscos_auth_research.md`](./viscos_auth_research.md))**: (1) `keyring 2.3` → `keyring-core 0.7` + `windows-native-keyring-store 1.1` (4.0 mimarisi, stale dependency'den kaçış, `default-features=false` ile regex yok). (2) `secrecy 0.10` + `zeroize 1` dependency'lere eklendi (memory dump baseline savunma, ADR-0011 zorunlu kıldı). (3) Varyant A (DPAPI/Keyring) encryption default, Varyant B (Argon2id passphrase) v2.0'da opt-in. (4) Multi-account v1'den itibaren altyapı (`user = user_id` Discord snowflake). (5) MFA backup codes (Argon2 PHC, keyring entry'sinde). (6) Captcha stratejisi: tarayıcıya yönlendir, token yapıştır (headless browser YOK, +30+ MB binary tasarrufu). (7) X-Super-Properties detaylandı: haftalık GitHub Action build_number sync, WebGL hash CEF/WebView2'den. (8) ToS disclaimer canonical metin (4 yerde tutarlı: ADR + README + modal + Settings). |

Detaylı ADR'ler: `docs/DECISIONS.md` ve [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md). Eski tradeoff matrisi: arşivde `viscos_discord_client_89f43510.plan.md` Bölüm 2.4. **Haziran 2026 eki:** ADR-0012 (Frontend Mimari), [`docs/CEF-VS-WEBVIEW2.md`](../CEF-VS-WEBVIEW2.md) (backend karşılaştırma dokümanı), [`bridge-resilience-research.md`](./bridge-resilience-research.md) (Discord DOM churn stratejisi).

---

## 7. Rakipler ve Alınan Dersler (Özet)

Aşağıdaki 5 proje Viscos'un mimari kararlarını şekillendirdi. Diğer rakipler (Legcord, Ventauri, LemonCord, Equirust, Acheron, CPPCord, Dissent, Puklic) bilgilendirici ama tekrar eden pattern'ler, ayrıntıya girilmedi.

| Proje | Mimari | Ders | Viscos'a Etkisi |
|-------|--------|------|-----------------|
| **Dorion** | Tauri + WebView | Tauri shell yeterli; side panel native olmak zorunda değil | Dorion modeli kanıtlanmış; Viscos onu iced side panel + WebView2 ile genişletir |
| **Leto** | tao+wry (Tauri'siz) | "Tauri vs tao+wry" performans farkı yok | Tauri olmadan `wry` kararı doğru |
| **Vesktop** | Electron + Vencord | 11 namespace'li `VesktopNative` bridge API'si plugin ekosistemi sağlar | Vencord/Equicord uyumu için preload bridge tasarımı kritik (Faz 5/6) |
| **kind / Acheron** | C++/Qt tam native | Animated emoji, sticker, DAVE E2EE'yi sıfırdan yazmak yıllar alır | Hibrit yaklaşımın en güçlü gerekçesi |
| **CEF / Servo** | CEF: Tauri'nin `cef-rs`'i; Servo: 0.1.0 | CEF Win11 GDI leak'ten kaçış; Servo 2026'da Discord için erken | Faz 8.5'te CEF opt-in; v3'te Servo değerlendir |
| **Tauri upstream (wry#767, tauri#13133, tauri#13758)** | `eval_script` unmanaged lifecycle + channel callback leak | Pull-based + watchdog **tolere eder**; SharedBuffer (Faz 4) **yapısal çözüm** | [`webview2-hardening.md` Bölüm 3](./webview2-hardening.md#3-pull-based-ipc-pattern-kritik) |
| **Microsoft WebView2 SharedBuffer (#3185, #3360)** | `CoreWebView2SharedBuffer` + `PostSharedBufferToScript` | Edge ≥ 114 stable, zero-copy, max 2GB | Faz 4'te medya response'ları SharedBuffer; [`phase-4.0-cache-media.md` Bölüm 4.4](./phase-4.0-cache-media.md#44-büyük-blob-transfer-webview2-sharedbuffer) |

**Viscos'un farkı (kimse yok hepsinde):**
1. Side panel iced native
2. Tauri'siz tao+wry
3. Vencord/Equicord uyumu hedefi
4. 15–25 MB binary (CEF hariç)
5. Pluggable WebView backend trait
6. **GDI watchdog** + throttling + CEF üç katmanlı savunma
7. AI-yazar / insan-onay workflow

---

## 8. Yasal/Uyumlu Kullanım Notu

Bu proje bir **üçüncü parti Discord istemcisidir**. Discord, kullanıcı hesaplarını otomatikleştiren istemcileri (self-bot) tespit edip banlayabilir. Bu yazılım **GPL-3.0** ile lisanslanır; kullanıcılar tüm risklerin kendilerine ait olduğunu kabul eder.

**Öneriler:**
- Kullanıcı kendi hesabıyla giriş yapar
- Resmi Discord API ToS'una uyulur
- Mass DM, spam, scraping amaçlı değildir
- Discord'un IP'sine veya hizmetine saldırı içermez

---

## 9. Açık Kaynak Topluluk

- **Lisans:** GPL-3.0
- **README:** Kurulum, build, kullanım
- **CONTRIBUTING.md:** PR kuralları, kod stili, **AI-PR kuralları**
- **Issue templates:** Bug report, feature request, **AI-task request**
- **PR'da `AI-Generated: yes` label zorunlu**, insan co-author zorunlu

---

## 10. Ölçüm ve Başarı Kriterleri

### v1.0 (MVP)
- Windows 10/11'de çalışıyor
- **Win11 default CEF, Win10 default WebView2** (Faz 1.6)
- Login (email/şifre, QR, token — captcha redirect fallback ile)
- MFA: TOTP + backup codes (Argon2 PHC, keyring'de)
- `Secret<String>` + `ZeroizeOnDrop` tüm token path'lerinde
- ToS disclaimer modal (ADR-0011 canonical metin)
- Sunucu + kanal listesi (native, animasyonlu)
- Mesaj gönderme/alma (WebView2 veya CEF)
- Mesaj cache (SQLite)
- Tray + bildirimler + global hotkeys
- GDI watchdog aktif (7000/9000 + draft autosave), 24 saat soak test:
  - **<5 restart + gap <2s + draft kaybı 0** (Win10)
  - 0 restart (Win11 + CEF)
- 200–300 MB RAM, < 2 s cold start
- Telemetry backend aktif (restart trend raporu)
- GitHub Releases'de iki MSI: `viscos-webview2.msi`, `viscos-cef.msi`
- **AI-PR'larının %100'ü insan review'den geçti**

### v1.5
- Mouse hover throttling (Faz 1.5 sonucuna göre)
- Custom CSS tema
- Vencord plugin sistemi
- Voice (DAVE E2EE, koşullu)
- Screen share

### v2.0
- Linux (WebKitGTK → muhtemelen CEF/Servo)
- macOS (WKWebView)
- Multi-account
- Plugin marketplace
- Faz 8.5 CEF backend kararı (Win11 leak verisine göre)

### v3.0
- Servo Wry backend (olgunlaşırsa)
- Native voice/video (artık gerekli olmayabilir)

### AI Workflow Metrikleri
| Metrik | Hedef |
|--------|-------|
| AI-PR insan review süresi | < 30 dk ortalama |
| AI-PR merge oranı | > %70 |
| AI kod bug rate | < production ortalamasının 1.5× |
| AI-PR redo oranı | < %20 |
| İnsan coding süresi | < 5 saat/hafta |
| Coverage | > %80 her crate |
| Clippy warning | 0 (CI fail) |
| Memory regression | < %5 release-to-release |

---

## 11. Hızlı Linkler

- **ADR (Mimari Karar Kayıtları):** [`docs/DECISIONS.md`](../DECISIONS.md) — **ADR-0010 (Haziran 2026): Cache Stack**, **ADR-0011 (Haziran 2026): Auth Stack** (keyring-core + secrecy + zeroize), **ADR-0012 (Haziran 2026): Frontend Mimari — WebView + Native Shell hibrit**
- **WebView2 hardening:** [`webview2-hardening.md`](./webview2-hardening.md)
- **CEF vs WebView2 karşılaştırma:** [`docs/CEF-VS-WEBVIEW2.md`](../CEF-VS-WEBVIEW2.md) (Haziran 2026, ADR-0012 + Faz 8.5 referansı)
- **Bridge resilience araştırması:** [`bridge-resilience-research.md`](./bridge-resilience-research.md) (Haziran 2026, ADR-0012 §2 — Discord DOM churn + selector best-practices)
- **Frontend trade-off analizi:** [`viscos_index.md` Bölüm 6 + ADR-0012](#6-teknik-kararlar-özet) (tam trade-off matrisi: hibrit vs kind/Acheron vs Tauri vs Servo)
- **Haziran 2026 trade-off analizi:** [`window-webview-watchdog-tradeoffs.md`](./window-webview-watchdog-tradeoffs.md)
- **Cache stack araştırması (Haziran 2026):** [`cache-stack-research.md`](./cache-stack-research.md)
- **Faz 0.0 Foundation:** [`phase-0.0-foundation.md`](./phase-0.0-foundation.md)
- **Faz 0.5 AI Workflow:** [`phase-0.5-ai-workflow-setup.md`](./phase-0.5-ai-workflow-setup.md)
- **Faz 1.0 Window + WebView + Watchdog:** [`phase-1.0-window-webview.md`](./phase-1.0-window-webview.md)
- **Faz 1.5 Telemetry & Restart Optimization:** [`phase-1.5-telemetry-and-restart-optimization.md`](./phase-1.5-telemetry-and-restart-optimization.md) (yeniden adlandırıldı)
- **Faz 1.6 CEF Default Rollout (Win11):** [`phase-1.6-cef-default-rollout.md`](./phase-1.6-cef-default-rollout.md) (yeni)
- **Faz 2.0 Discord API:** [`phase-2.0-discord-api.md`](./phase-2.0-discord-api.md) — **ADR-0011 (Haziran 2026) Auth Stack** uygulandı (`keyring-core 0.7` + `windows-native-keyring-store 1.1` + `secrecy 0.10` + `zeroize 1`, MFA TOTP+backup, captcha redirect, Varyant A encryption)
- **Auth araştırması:** [`viscos_auth_research.md`](./viscos_auth_research.md) (Haziran 2026)
- **Faz 3.0 Gateway:** [`phase-3.0-gateway.md`](./phase-3.0-gateway.md)
- **Faz 4.0 Cache + Medya:** [`phase-4.0-cache-media.md`](./phase-4.0-cache-media.md)
- **Faz 5.0 Native UI + Vencord:** [`phase-5.0-native-ui.md`](./phase-5.0-native-ui.md)
- **Faz 6.0 Hotkeys:** [`phase-6.0-hotkeys.md`](./phase-6.0-hotkeys.md)
- **Faz 7.0 Voice/Video:** [`phase-7.0-voice-video.md`](./phase-7.0-voice-video.md)
- **Faz 8.0 Distribution:** [`phase-8.0-distribution.md`](./phase-8.0-distribution.md)
- **Faz 8.5 CEF Backend Yönetim (Default-Out):** [`phase-8.5-cef-backend.md`](./phase-8.5-cef-backend.md) (yeniden tanımlandı)
