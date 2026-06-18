# Viscos ADR Implementation Packet Index

> **Master index for implementation packets.** Each packet is a self-contained brief for a phase worker, derived from a specific ADR in [`docs/DECISIONS.md`](../DECISIONS.md).
> Worker'lar: Bu index'ten başla, kendi fazına ait packet'leri oku, sonra o packet'in "Uygulama adımları" bölümünü takip et.

---

## 1. ADR → Packet Mapping (12 ADR)

| ADR | Başlık | Durum | Packet | Uygulandığı Faz | Tahmini Etki Yüzeyi |
|---|---|---|---|---|---|
| [ADR-0001](../DECISIONS.md#adr-0001-cargo-workspace-bazelbuck2-değil) | Cargo Workspace (Bazel/Buck2 değil) | ✅ Accepted | [packet-0001-cargo-workspace.md](./packet-0001-cargo-workspace.md) | Faz 0.0 | Kök `Cargo.toml` `[workspace]` tablosu |
| [ADR-0002](../DECISIONS.md#adr-0002-async-runtime--tokio-granular-features) | Async Runtime — Tokio (granular features) | ✅ Accepted | [packet-0002-granular-tokio.md](./packet-0002-granular-tokio.md) | Faz 0.0 | `[workspace.dependencies]` + tüm crate'lerde `tokio` declare |
| [ADR-0003](../DECISIONS.md#adr-0003-config-kütüphanesi--config-rs-figment-değil) | Config Kütüphanesi — `config-rs` | ✅ Accepted | [packet-0003-config-library.md](./packet-0003-config-library.md) | Faz 0.0 | `crates/viscos-config/` (TOML + env layered) |
| [ADR-0004](../DECISIONS.md#adr-0004-github-actions--7-job-matrix-tek-runner-2-katmanlı-cache) | GitHub Actions + 7-Job Matrix | ✅ Accepted | [packet-0004-ci-pipeline.md](./packet-0004-ci-pipeline.md) | Faz 0.0 | `.github/workflows/ci.yml` + `.cargo/deny.toml` |
| [ADR-0005](../DECISIONS.md#adr-0005-lto-fat--panic-abort-release-profile) | LTO `fat` + `panic = "abort"` (Release Profile) | ✅ Accepted | [packet-0005-release-profile.md](./packet-0005-release-profile.md) | Faz 0.0 | Kök `Cargo.toml` `[profile.release]` |
| [ADR-0006](../DECISIONS.md#adr-0006-toolchain-189--edition-2024-twilight-rs-uyumu) | Toolchain 1.89 + Edition 2024 (twilight-rs uyumu) | ✅ Accepted | [packet-0006-rust-toolchain.md](./packet-0006-rust-toolchain.md) | Faz 0.0 | `rust-toolchain.toml` + `[workspace.package].rust-version` |
| [ADR-0007](../DECISIONS.md#adr-0007-error-handling--thiserror-lib--anyhow-app) | Error Handling — thiserror (lib) + anyhow (app) | ✅ Accepted | [packet-0007-error-handling.md](./packet-0007-error-handling.md) | Faz 0.0 | `crates/viscos-error/` (thiserror, `#[non_exhaustive]`) |
| [ADR-0008](../DECISIONS.md#adr-0008-discord-rest--gateway--twilight-rs-sub-crate-seçici-entegrasyon) | Discord REST + Gateway — `twilight-rs` | ✅ Accepted | [packet-0008-discord-api-twilight.md](./packet-0008-discord-api-twilight.md) | Faz 2.0 + Faz 3.0 | `crates/viscos-api/` (REST + Gateway wrapper) |
| ADR-0009 | _(Yok — DECISIONS.md'de ADR-0008'den ADR-0010'a atlanır)_ | — | _(yok)_ | — | — |
| [ADR-0010](../DECISIONS.md#adr-0010-cache-stack--sqlite--moka--foyer-varyant-a-2026-q2) | Cache Stack — SQLite + moka + foyer (Varyant A) | ✅ Accepted | [packet-0010-cache-stack.md](./packet-0010-cache-stack.md) | Faz 4.0 | `crates/viscos-cache/` + `crates/viscos-media/` |
| [ADR-0011](../DECISIONS.md#adr-0011-auth-stack--keyring-core--secrecy--varyant-a-encryption-haziran-2026) | Auth Stack — `keyring-core` + `secrecy` + Varyant A Encryption | 🟡 Proposed | [packet-0011-auth-stack.md](./packet-0011-auth-stack.md) | Faz 2.0 | `crates/viscos-auth/` (keyring, MFA, captcha, super-properties) |
| [ADR-0012](../DECISIONS.md#adr-0012-frontend-mimari--hibrit-webview--native-shell-haziran-2026-trade-off-revizyonu) | Frontend Mimari — Hibrit (WebView + Native Shell) | 🟡 Proposed | [packet-0012-frontend-hybrid.md](./packet-0012-frontend-hybrid.md) | Faz 1.0 + 1.5 + 1.6 + 8.5 (cross-cutting) | `crates/viscos-shell/` + `crates/viscos-webview/` + `frontend/src/bridge.ts` |

**Okunan ADR sayısı: 12/12** (ADR-0009 numarası atlanmış, dosyada yok; toplamda 12 aktif ADR — 0001–0008, 0010–0012).

---

## 2. Faz → Packet Mapping (Worker için hızlı rehber)

| Faz | ADR'ler | Worker | Önce packet'leri oku |
|---|---|---|---|
| 0.0 | ADR-0001, 0002, 0003, 0004, 0005, 0006, 0007 | Foundation worker | 0001 → 0002 → 0006 → 0005 → 0007 → 0003 → 0004 |
| 0.5 | _(yok — packet gerektirmez; AI workflow setup)_ | AI-workflow worker | _(yok)_ |
| 1.0 | ADR-0012 (Dalga 1) | Shell+Webview worker | 0012 (Faz 1.0 Dalga 1) |
| 1.5 | ADR-0012 (Dalga 2: shadow mode + parite) | Telemetry worker | 0012 (Faz 1.5 Dalga 2) |
| 1.6 | ADR-0012 (Dalga 3: CEF default) | CEF-rollout worker | 0012 (Faz 1.6 Dalga 1) |
| 2.0 | ADR-0008 (REST), ADR-0011 (auth) | Auth+API worker | 0011 → 0008 (Faz 2.0 Dalga 1) |
| 3.0 | ADR-0008 (Gateway) | Gateway worker | 0008 (Faz 3.0 Dalga 1) |
| 4.0 | ADR-0010 (Dalga 1 + 2 + 3) | Cache+Media worker | 0010 (Dalga 1 → 2 → 3) |
| 5.0 | _(yok — native UI, packet gerektirmez)_ | NativeUI worker | _(yok)_ |
| 6.0 | _(yok — hotkeys, packet gerektirmez)_ | Hotkeys worker | _(yok)_ |
| 7.0 | _(opsiyonel — DAVE E2EE, ADR-0012 §4 davey zaten Optional)_ | _(voice/video, opsiyonel)_ | _(0012 §4 referans)_ |
| 8.0 | _(yok — distribution, packet gerektirmez)_ | Distribution worker | _(yok)_ |
| 8.5 | ADR-0012 (Dalga 4: CEF management UI) | DistributionUI worker | 0012 (Faz 8.5 Dalga 1) |

---

## 3. "Ben bir faz worker'ıyım, hangi packet'leri okumalıyım?" — Hızlı Rehber

### 3.1 Foundation worker (Faz 0.0)

**Sıralı okuma:**

1. **[packet-0001-cargo-workspace.md](./packet-0001-cargo-workspace.md)** — Workspace iskeleti.
2. **[packet-0002-granular-tokio.md](./packet-0002-granular-tokio.md)** — Tokio feature seti.
3. **[packet-0006-rust-toolchain.md](./packet-0006-rust-toolchain.md)** — Rust 1.89 + Edition 2024.
4. **[packet-0005-release-profile.md](./packet-0005-release-profile.md)** — lto + panic = abort.
5. **[packet-0007-error-handling.md](./packet-0007-error-handling.md)** — `viscos-error` crate.
6. **[packet-0003-config-library.md](./packet-0003-config-library.md)** — `viscos-config` crate.
7. **[packet-0004-ci-pipeline.md](./packet-0004-ci-pipeline.md)** — 7-job CI.

**Çıktı:** Çalışan boş Viscos binary'si, yeşil CI, temel altyapı crate'leri.

---

### 3.2 Shell+Webview worker (Faz 1.0)

**Sıralı okuma:**

1. **[packet-0012-frontend-hybrid.md § Faz 1.0 Dalga 1](./packet-0012-frontend-hybrid.md#faz-10-dalga-1-shellwebview-worker)** — Hibrit mimari + WebViewBackend trait + iced 0.14 spike.
2. (Referans) [`phase-1.0-window-webview.md`](../.cursor/plans/phase-1.0-window-webview.md).

**Çıktı:** Çalışan pencere + side panel + WebView + bridge.ts resilience rules.

---

### 3.3 Telemetry worker (Faz 1.5)

**Sıralı okuma:**

1. **[packet-0012-frontend-hybrid.md § Faz 1.5 Dalga 2](./packet-0012-frontend-hybrid.md#faz-15-dalga-2-telemetry-worker)** — 24 saat shadow mode + fingerprint parite check.
2. (Referans) [`phase-1.5-telemetry-and-restart-optimization.md`](../.cursor/plans/phase-1.5-telemetry-and-restart-optimization.md).
3. (Referans) **[packet-0010-cache-stack.md § Dalga 3](./packet-0010-cache-stack.md#dalga-3--adaptive-tier-sizing-faz-15-telemetry-entegrasyonu)** — Adaptive tier sizing entegrasyonu.

**Çıktı:** Telemetry backend, shadow mode implementasyonu, parite check Action.

---

### 3.4 CEF-rollout worker (Faz 1.6)

**Sıralı okuma:**

1. **[packet-0012-frontend-hybrid.md § Faz 1.6 Dalga 1](./packet-0012-frontend-hybrid.md#faz-16-dalga-1-cef-rollout-worker)** — CEF default Win11, WebView2 default Win10.
2. (Referans) [`phase-1.6-cef-default-rollout.md`](../.cursor/plans/phase-1.6-cef-default-rollout.md).
3. (Referans) [`docs/CEF-VS-WEBVIEW2.md`](../docs/CEF-VS-WEBVIEW2.md).

**Çıktı:** Win11'de CEF backend aktif, Win10'da WebView2 backend, `select_default_backend()` telemetry-driven.

---

### 3.5 Auth+API worker (Faz 2.0)

**Sıralı okuma:**

1. **[packet-0011-auth-stack.md](./packet-0011-auth-stack.md)** — `viscos-auth` (keyring, MFA, captcha, super-properties).
2. **[packet-0008-discord-api-twilight.md § Faz 2.0 Dalga 1](./packet-0008-discord-api-twilight.md#faz-20-dalga-1-authapi-worker)** — REST client (`twilight-http`).
3. (Referans) [`phase-2.0-discord-api.md`](../.cursor/plans/phase-2.0-discord-api.md).
4. (Referans) [`viscos_auth_research.md`](../.cursor/plans/viscos_auth_research.md).

**Çıktı:** Token keyring'de, MFA TOTP+backup, captcha redirect, REST çağrıları çalışıyor.

---

### 3.6 Gateway worker (Faz 3.0)

**Sıralı okuma:**

1. **[packet-0008-discord-api-twilight.md § Faz 3.0 Dalga 1](./packet-0008-discord-api-twilight.md#faz-30-dalga-1-gateway-worker)** — `twilight-gateway` wrapper.
2. (Referans) [`phase-3.0-gateway.md`](../.cursor/plans/phase-3.0-gateway.md).

**Çıktı:** WebSocket bağlantısı, sharding, session resume, zstd-stream (twilight hallediyor), reconnect backoff.

---

### 3.7 Cache+Media worker (Faz 4.0)

**Sıralı okuma:**

1. **[packet-0010-cache-stack.md](./packet-0010-cache-stack.md)** — `viscos-cache` (SQLite + moka) + `viscos-media` (foyer + encryption).
2. (Referans) [`phase-4.0-cache-media.md`](../.cursor/plans/phase-4.0-cache-media.md).
3. (Referans) [`cache-stack-research.md`](../.cursor/plans/cache-stack-research.md).

**Çıktı:** Mesaj cache, attachment cache, CDN content-addressable, adaptive tier sizing.

---

### 3.8 DistributionUI worker (Faz 8.5)

**Sıralı okuma:**

1. **[packet-0012-frontend-hybrid.md § Faz 8.5 Dalga 1](./packet-0012-frontend-hybrid.md#faz-85-dalga-1-distributionui-worker)** — CEF default-out wizard, Chromium flags UI, self-update.
2. (Referans) [`phase-8.5-cef-backend.md`](../.cursor/plans/phase-8.5-cef-backend.md).

**Çıktı:** Kullanıcı WebView backend'i seçebiliyor, CEF self-update çalışıyor.

---

## 4. Cross-Reference Ağı (Packet ↔ ADR ↔ Plan)

```
ADR-0001 (Workspace)
  └─ packet-0001 ─┐
                   ├─► [phase-0.0-foundation.md]
ADR-0002 (Tokio)   │
  └─ packet-0002 ──┤
                   │
ADR-0006 (MSRV)    │
  └─ packet-0006 ──┤
                   │
ADR-0005 (Release) │
  └─ packet-0005 ──┤
                   │
ADR-0007 (Error)   │
  └─ packet-0007 ──┤
                   │
ADR-0003 (Config)  │
  └─ packet-0003 ──┤
                   │
ADR-0004 (CI)      │
  └─ packet-0004 ──┘

ADR-0008 (Twilight) ───────────────────┐
  └─ packet-0008 (Faz 2.0 + Faz 3.0)   │
                                       ├─► [phase-2.0-discord-api.md]
ADR-0011 (Auth) ──────────────────────┤
  └─ packet-0011 (Faz 2.0)            ├─► [phase-3.0-gateway.md]
                                       │   [viscos_auth_research.md]
                                       │
ADR-0010 (Cache) ─────────────────────┐
  └─ packet-0010 (Faz 4.0)            ├─► [phase-4.0-cache-media.md]
                                       │   [cache-stack-research.md]
                                       │
ADR-0012 (Frontend) ──────────────────┐
  └─ packet-0012 (Faz 1.0 + 1.5 + 1.6 + 8.5)
                                       ├─► [phase-1.0-window-webview.md]
                                       │   [phase-1.5-telemetry-and-restart-optimization.md]
                                       │   [phase-1.6-cef-default-rollout.md]
                                       │   [phase-8.5-cef-backend.md]
                                       │   [docs/CEF-VS-WEBVIEW2.md]
                                       │   [bridge-resilience-research.md]
                                       │   [webview2-hardening.md]
                                       │   [viscos_index.md § 1, 6, 7]
```

---

## 5. Önemli Bulgular / Uyarılar

1. **ADR-0009 numarası atlanmış.** `docs/DECISIONS.md`'de ADR-0008'den ADR-0010'a atlıyor (muhtemelen reserve edilmiş / iptal edilmiş). Toplam aktif ADR: 12 (0001–0008 + 0010–0012). Mapping tablosunda "yok" olarak işaretlendi.

2. **ADR-0011 ve ADR-0012 Haziran 2026 güncellemesi — Proposed durumda.** Bu iki ADR insan onayı bekliyor; uygulama başlamadan önce mimari karar review gerekir. Diğer 10 ADR Accepted.

3. **ADR-0008 iki worker'a bölündü:** REST (Faz 2.0) + Gateway (Faz 3.0). Aynı packet (packet-0008) iki fazda uygulanır, sıralı.

4. **ADR-0012 dört worker'a bölündü (cross-cutting):** Faz 1.0 (mimari) + Faz 1.5 (telemetry/shadow) + Faz 1.6 (CEF default) + Faz 8.5 (CEF management). Aynı packet (packet-0012) dört fazda uygulanır, sıralı.

5. **Faz 0.0 Dalga sıralaması önemli:**
   - Dalga 1 (sıralı): 0001 (workspace) → 0002 (tokio) → 0006 (toolchain) → 0005 (release profile). Hepsi kök `Cargo.toml` üzerinde.
   - Dalga 2: 0007 (error crate) → 0003 (config crate). Config, Error variant'ına bağımlı.
   - Dalga 3: 0004 (CI). Workspace + crate'ler buildable olduktan sonra.

6. **MSRV 1.89 zorunlu (Haziran 2026 revizyonu).** 1.80 / 1.85'e geri dönüş yok. Twilight-rs 0.17.x MSRV'si ile hizalı.

7. **Binary bütçesi 25 MB (ADR-0005 + 0010 + 0011 katkıları dahil).** `lto = "fat"` + `panic = "abort"` + CEF HARİÇ (~240 MB). CEF kullanıcı opt-in (Faz 8.5).

8. **AI-yazım riski olan ADR'ler sıfırlandı:**
   - ADR-0008 (twilight) — sıfırdan gateway yazma riski kalmadı.
   - ADR-0010 (cache) — tanıdık API, moka/foyer/rusqlite.
   - ADR-0011 (auth) — keyring-core 4.0 mimarisi.
   - ADR-0012 (frontend) — hibrit kanıtlanmış (Dorion/Leto/Vesktop).

9. **Güvenlik/Hijyen baseline (ADR-0007 + 0011):**
   - `thiserror` (lib) + `anyhow` (app) split zorunlu.
   - `secrecy::Secret<String>` + `ZeroizeOnDrop` tüm token path'lerinde.
   - `keyring-core` (DPAPI arkası) token + encryption key.

10. **Telemetri & Adaptivity (Haziran 2026 ekleri):**
    - Faz 1.5 telemetry backend → ADR-0010 adaptive tier sizing + ADR-0012 shadow mode için veri sağlar.
    - `select_default_backend()` Faz 1.6'da telemetry-driven olur.

---

## 6. Engeller

- **Yok.** Tüm 12 ADR dosyada mevcut, context net, packet'ler oluşturuldu. Worker'lar packet'leri okuyup uygulamaya başlayabilir.

**İstisna:** ADR-0011 ve ADR-0012 henüz **Proposed** — worker'lar uygulamaya başlamadan önce insan onayı beklenmeli.

---

## 7. Packet Dosya Yapısı

```
.cursor/packets/
├── packet-0001-cargo-workspace.md      ✅ Accepted — Foundation Dalga 1
├── packet-0002-granular-tokio.md       ✅ Accepted — Foundation Dalga 1
├── packet-0003-config-library.md       ✅ Accepted — Foundation Dalga 2
├── packet-0004-ci-pipeline.md          ✅ Accepted — Foundation Dalga 3
├── packet-0005-release-profile.md      ✅ Accepted — Foundation Dalga 1
├── packet-0006-rust-toolchain.md       ✅ Accepted — Foundation Dalga 1
├── packet-0007-error-handling.md       ✅ Accepted — Foundation Dalga 2
├── packet-0008-discord-api-twilight.md ✅ Accepted — Auth+API + Gateway
├── packet-0010-cache-stack.md          ✅ Accepted — Cache+Media
├── packet-0011-auth-stack.md           🟡 Proposed — Auth+API
├── packet-0012-frontend-hybrid.md      🟡 Proposed — Shell+Webview + Telemetry + CEF + DistributionUI
└── README / INDEX                      (bu dosya)
```

---

## 8. Nasıl Kullanılır (Worker İçin 3 Adım)

1. **Yukarıdaki "Ben bir faz worker'ıyım" bölümünde** kendi fazını bul.
2. **O fazın packet'lerini** (göreceli path) aç ve oku.
3. **Her packet'in "Uygulama adımları" bölümünü** sıralı takip et. "Doğrulama" + "Kabul kriterleri" ile bitir.

**Soru?** Packet'te eksik bilgi → ilgili ADR'ye bak (`docs/DECISIONS.md`). Hâlâ eksik → bu dispatcher'a geri dön (yeni packet gerekir).
