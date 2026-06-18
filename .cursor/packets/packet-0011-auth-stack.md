# Implementation Packet — ADR-0011: Auth Stack — `keyring-core` + `secrecy` + Varyant A Encryption

## Header

- **ADR:** ADR-0011
- **Başlık:** Auth Stack — `keyring-core` + `secrecy` + Varyant A Encryption (Haziran 2026)
- **Durum:** 🟡 Proposed (insan onayı bekliyor)
- **Tarih:** 2026-06-18
- **Kaynak ADR:** [`docs/DECISIONS.md` § ADR-0011](../../docs/DECISIONS.md#adr-0011-auth-stack--keyring-core--secrecy--varyant-a-encryption-haziran-2026)
- **Önceki plan:** [`phase-2.0-discord-api.md`](../../.cursor/plans/phase-2.0-discord-api.md) § 2 Cargo.toml, § 4.1 AuthStorage, § 4.3 MFA, § 8 Karar Noktası
- **Araştırma:** [`viscos_auth_research.md`](../../.cursor/plans/viscos_auth_research.md)

## Hedef faz worker

**Auth+API worker, Faz 2.0, Dalga 1.** `viscos-auth` crate'inin kurulumu. Bu packet `phase-2.0-discord-api.md` § 2 (Cargo.toml), § 4.1 (AuthStorage), § 4.3 (MFA) bölümlerini uygular. **ADR henüz Proposed durumda** — uygulamaya başlamadan önce insan onayı alınmalı.

## Uygulama adımları

1. **`Cargo.toml` `[workspace.dependencies]`** — auth dependency'leri:
   ```toml
   keyring-core = { version = "0.7", default-features = false }
   windows-native-keyring-store = { version = "1.1", default-features = false }
   totp-rs = { version = "5.7", default-features = false, features = ["zeroize"] }
   secrecy = { version = "0.10", features = ["serde"] }
   zeroize = { version = "1", features = ["derive"] }
   qrcode = "0.14"
   ```
   - `default-features = false` × 2 → `regex` (~1+ MB) dependency'si yok (binary bütçesi korunur).
   - Varyant B (Argon2id passphrase, age envelope) **YOK** (v2.0 opt-in).

2. **`crates/viscos-auth/`** workspace'e ekle:
   - `Cargo.toml`: yukarıdaki + `viscos-core`, `viscos-error`, `viscos-api` (model tipleri için).
   - `src/lib.rs`: public API (`AuthStorage`, `login_token`, `login_email`, `verify_mfa`, `verify_backup_code`).
   - `src/storage.rs`: `AuthStorage` struct — `keyring-core::Entry` wrapper, `service = "Viscos"`, `user = user_id` (Discord snowflake).
   - `src/login.rs`: `/auth/login` flow (email + password + MFA), `LoginResult::CaptchaRequired { url }` döndürür.
   - `src/mfa.rs`: TOTP (`totp-rs`) + backup codes (Argon2 PHC).
   - `src/super_properties.rs`: X-Super-Properties builder (WebGL hash, build_number, navigator, screen, locale, timezone).
   - `src/disclaimer.rs`: ToS disclaimer canonical metin (4 yerde: ADR + README + modal + Settings).

3. **Multi-account altyapısı (v1'den itibaren)**:
   ```rust
   let entry = keyring_core::Entry::new("Viscos", &user_id.to_string())?;
   entry.set_password(&token)?;
   ```
   - v1 UI single-account; v2.0'da `keyring-core`'un `search` feature'ı açılır + list UI gelir (0 refactor).

4. **MFA backup codes (v1'den itibaren)**:
   ```rust
   pub struct SerializedAccount {
       pub token_hash: String,                    // argon2 PHC
       pub mfa_secret: Secret<String>,            // TOTP secret
       pub mfa_backup_hashes: Vec<String>,        // 8-char codes → argon2 PHC
       pub super_properties: serde_json::Value,
   }
   ```
   - 10 koddan az kaldığında UI'da uyarı.

5. **Captcha stratejisi**:
   - `LoginResult::CaptchaRequired { url: String }` döndürülür.
   - Shell "Tarayıcıda giriş yap, token'ı buraya yapıştır" UI'ı açar.
   - Headless browser (Playwright/headless_chrome) **YOK**.

6. **X-Super-Properties detaylandırma**:
   - `build_number` senkronizasyonu: haftalık GitHub Action (cron) → `discord.com/app` JS bundle'ından parse → PR otomatik açar.
   - WebGL hash kaynağı: `viscos-webview` Faz 1.6 backend kararıyla (Win11 CEF, Win10 WebView2).
   - `navigator.userAgent`, `screen`, `locale`, `timezone_offset` hardcoded.

7. **ToS disclaimer canonical metin** (4 yerde):
   > "Viscos, Discord'un resmi olmayan bir istemcisidir. Kullanıcı kendi hesabıyla giriş yapar; ToS ihlali (otomasyon, scraping, mass DM) bu istemcinin tasarım amacı değildir ve tüm sorumluluk kullanıcıya aittir. Discord multi-layered detection (fingerprint + behavioral heuristics) ile self-bot tespit edip banlayabilir."

8. **Test:**
   - `tests/keyring_round_trip.rs`: token store + retrieve (gerçek DPAPI).
   - `tests/mfa_totp.rs`: TOTP generate + verify (RFC 6238 test vectors).
   - `tests/mfa_backup_codes.rs`: 10 code generate, 9 kullan, UI uyarısı tetikle.
   - `tests/captcha_redirect.rs`: `LoginResult::CaptchaRequired { url }` doğru dönüyor.
   - `tests/super_properties.rs`: build_number, WebGL hash alanları dolu.

9. **Doğrulama**:
   - `cargo test -p viscos-auth` → 10+ test geçer.
   - `cargo build --release` binary 25 MB altında.
   - MSRV 1.89 ile uyumlu (`keyring-core 1.88` + `windows-native-keyring-store 1.88` → OK).

## Kabul kriterleri

- ✅ `viscos-auth` crate workspace member.
- ✅ `keyring-core 0.7` + `windows-native-keyring-store 1.1` (default-features=false, regex yok).
- ✅ `totp-rs 5.7` (zeroize feature) + `secrecy 0.10` + `zeroize 1` declare.
- ✅ `Argon2id` (v2.0) **YOK** (v1'de yok).
- ✅ Multi-account altyapısı (`user = user_id`).
- ✅ MFA TOTP + backup codes (Argon2 PHC).
- ✅ Captcha redirect strategy (`LoginResult::CaptchaRequired`).
- ✅ X-Super-Properties detaylandırılmış (build_number sync + WebGL hash).
- ✅ ToS disclaimer 4 yerde (ADR + README + modal + Settings).
- ✅ Public API her zaman `Result<T, ViscosError>` döner.
- ✅ `Secret<String>` + `ZeroizeOnDrop` tüm token path'lerinde zorunlu.

## Test stratejisi

- **Unit:**
  - `tests/secrecy_audit.rs`: `expose_secret()` call site'ları grep'lenebilir.
  - `tests/zeroize_drop.rs`: Drop sonrası bellek sıfırlanmış (mock allocator ile).
- **Integration:**
  - `keyring` integration (gerçek Windows DPAPI): token store → process restart → retrieve OK.
  - TOTP RFC 6238 vectors.
  - Captcha response mock → `LoginResult::CaptchaRequired`.
- **Manuel:**
  - Discord test hesabı ile email/şifre login → MFA prompt → token keyring'de.
  - QR login (mobile app) → `qrcode` crate ile render → user scan → token keyring'de.
  - `cargo deny check licenses` → keyring MIT, secrecy MIT/Apache, totp-rs MIT, zeroize MIT/Apache.
  - `cargo bloat --release -p viscos` → `keyring-core` + `secrecy` katkısı <500 KB.
  - GitHub Action (haftalık build_number sync) çalışıyor (manual trigger).

## Sınır durumları ve riskler

- **`keyring-core 0.7` 1.0 değil:** 1.0 çıkınca API drift olabilir. Mitigation: `cargo update` haftalık + AI PR review (ADR-0011 Olumsuz).
- **MSRV yükselmesi:** `keyring-core 1.88` + `windows-native-keyring-store 1.88` → ADR-0006 zaten 1.89, OK.
- **Backup codes UX:** Kullanıcı 10 kodu kaybederse hesap kurtarma yok. Mitigation: keyring'de şifreli (DPAPI), UI'da "göster" + "yenile" + "indir (.txt)" aksiyonları.
- **Captcha redirect UX friction:** Kullanıcı "neden tarayıcıya atıyor?" diye şaşırır. Mitigation: modal'da net metin + GIF.
- **Fingerprint WebGL hash'i CEF/WebView2'ye bağımlı:** Faz 1.6 kararı (Win11 CEF, Win10 WebView2) ile uyumlu. Backend değişirse fingerprint üretimi değişmeli.
- **AI-PR review yükü:** Yeni PR'da `keyring-core` + `secrecy` doğru kullanımı kontrol edilmeli (checklist'e eklenir).
- **Headless browser YOK kararı:** Captcha agresifleşirse (örn. her login'de) kullanıcı friction artar. Mitigation: token paste + Discord mobil QR login alternatif.
- **Varyant B yok (passphrase UX öldürür):** v2.0'da opt-in. ADR'ye göre bu trade-off kabul edildi.

## Review trigger'ları

- `keyring-core 1.0` major versiyon çıkarsa (API breaking).
- `keyring-core` bakım duraksaması (son commit >6 ay).
- Discord `/auth/login` rate-limit politikası değişirse.
- Discord captcha zorunluluğu kaldırırsa (headless browser tartışması yeniden açılır).
- Discord MFA mekanizması değişirse (passkey / WebAuthn eklenirse).
- Kullanıcı geri bildirimi: backup codes UX friction.
- v2.0 multi-account UI geliştirilirken `keyring-core search` feature açılır.

## Cross-references

- **ADR:** ADR-0006 (MSRV 1.89 uyumu), ADR-0007 (ViscosError), ADR-0008 (token → `twilight_http` Client), ADR-0010 (encryption anahtarı, Varyant A keyring paylaşımı), ADR-0012 (fingerprint WebGL hash backend bağımlılığı).
- **Plan:** [`phase-2.0-discord-api.md`](../../.cursor/plans/phase-2.0-discord-api.md) § 2, § 4.1, § 4.3, § 8.
- **Araştırma:** [`viscos_auth_research.md`](../../.cursor/plans/viscos_auth_research.md).
- **Alternatifler:** `keyring 2.3` (stale), `windows-dpapi` (chicken-and-egg), `age` passphrase (UX öldürür), TPM/Windows Hello (over-engineering), headless browser (binary + geliştirme riski) — hepsi elendi.
- **ToS:** `viscos_index.md` § 8 (Yasal/Uyumlu Kullanım Notu).
- **Index:** [`docs/ADR-INDEX.md`](../../docs/ADR-INDEX.md).

## İnsan onayı gerekli mi?

**Evet — bu packet Proposed durumda.** ADR-0011 Haziran 2026'da yeni revize edildi ve insan onayı bekliyor. Uygulama başlamadan önce **tüm karar noktaları** (keyring-core geçişi, Varyant A default, captcha redirect, MFA backup codes, multi-account altyapısı, ToS disclaimer) insan tarafından gözden geçirilmeli. Kabul edildikten sonra bu packet implementasyonu başlar. Captcha stratejisi (redirect vs headless) ve Varyant A vs Varyant B kararı özellikle insan onayına muhtaç.
