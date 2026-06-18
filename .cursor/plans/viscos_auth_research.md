# Viscos `viscos-auth` — Alternatif Araştırması, Trade-off'lar ve Öneri

> **Kapsam:** `viscos_index.md` + `phase-2.0-discord-api.md` + `docs/DECISIONS.md` içinde auth / token / keyring ile ilgili bütün referansların 2026 itibarıyla gözden geçirilmesi. Mevcut kararın (Aralık 2025/Phase-2 taslağı) hâlâ geçerli olup olmadığı, hangi alternatiflerin daha iyi olduğu, hangi trade-off'ların yeniden değerlendirilmesi gerektiği.
> **Tarih:** 2026-06-18
> **Durum:** 🟡 Proposed (yeni ADR-0011 adayı)

---

## 0. Plandaki Mevcut Auth Tasarımı (Özet)

Mevcut plan `phase-2.0-discord-api.md` ve `docs/DECISIONS.md` (ADR-0008, ADR-0010'a dağınık referanslar) üzerinden şunu varsayıyor:

| Konu | Mevcut karar | Kaynak |
|---|---|---|
| **Discord API transport** | `twilight-rs 0.17` (model + http + gateway) | ADR-0008 |
| **Token storage** | `keyring 2.3` crate'i, `windows-native` + `apple-native` + `linux-native` features | `phase-2.0` §2, §4.1 |
| **Token format** | `serde_json` serialize edilmiş `StoredToken { token, user_id, username }`, keyring entry = service `"Viscos"`, username `"user_token"` | `phase-2.0` §4.1 |
| **Memory hygiene** | `secrecy` crate ile string sarma | `phase-2.0` §9 risk tablosu |
| **MFA (TOTP)** | `totp-rs 5.0` ile 6-hanelik kullanıcı girişi doğrulama (üretmiyoruz, kullanıcının authenticator'ı üretiyor) | `phase-2.0` §4.3 |
| **QR login** | `qrcode 0.14` + `/auth/qr-login/start` + `/auth/qr-login/{id}` polling | `phase-2.0` §4.2 |
| **Login akışı** | `/auth/login` (email+şifre) + `/auth/mfa/totp` (MFA ticket) — twilight **yok**, manuel `reqwest` | `phase-2.0` §3.4 |
| **Encryption anahtarı kaynağı** | **Açık soru (Bölüm 8 Karar Noktası):** DPAPI / passphrase / hybrid | `phase-2.0` §8 |
| **ToS self-bot riski** | Disclaimer dialog (önerilen) | `phase-2.0` §8 |
| **X-Super-Properties** | Web client fingerprint'i `viscos-auth` üretir (twilight otomatik yapmaz) | `phase-2.0` §3 |

**Açık kalan noktalar (kritik):**
1. **`keyring = "2.3"` güncel değil** — 2026 Mayıs'ta **4.0.1** yayında, 4.0 mimari değişiklikle geldi (`keyring-core` + ayrı store crate'leri). Yeni proje 4.0 ekosistemine girmek zorunda.
2. **Encryption anahtarı sorusu Bölüm 8'de açık duruyor** — DPAPI / passphrase / hybrid seçim hiç yapılmamış. Bu karar olmadan `secrecy` ile sarıp keyring'e koymak, kullanıcının passphrase'i yoksa yalnızca OS-bound protection anlamına gelir (ki zaten keyring default olarak OS-bound). Yani passphrase katmanı "ek savunma" mı yoksa "zorunlu KDF" mu, belli değil.
3. **`secrecy` crate'ten söz ediliyor ama dependency olarak yazılmamış** (yukarıdaki risk tablosu dışında). ADR-0007 `anyhow`/`thiserror` dışında kütüphane listelemiyor.
4. **MFA backup codes** plan'da yok. Discord 2024'ten beri SMS kaldırıldı, sadece TOTP + backup codes; ikincisi için plan yok.
5. **Token rotation / invalidation** stratejisi belirsiz. Discord 401 alındığında temizleme, log out sonrası tam silme, multi-account davranışı plan'da yok.
6. **Multi-account** v2.0'a atılmış — `viscos-auth` API'si şimdiden multi-account'u desteklemeli mi, yoksa keyring'de tek entry mi tutulmalı, karar yok.
7. **Session encryption (SQLite cache)** için anahtar yönetimi ADR-0010'da "Keyring (DPAPI) default; Argon2 parola türetme ileride opt-in (Faz 5+)" yazıyor. Bu auth ile cache'in anahtar türetme katmanını nasıl paylaşacağı belirsiz.

---

## 1. Auth için "Doğru Şey" Ne Demek? (Tehdit Modeli)

Bir üçüncü parti Discord istemcisinin token güvenliği için gerçekçi tehdit modeli:

| Tehdit | Olasılık | Etki | Viscos için anlam |
|---|---|---|---|
| **Laptop çalınması, login ekranı kapalı** | Orta | Yüksek | DPAPI zaten korur (kullanıcı oturumu kilitli) |
| **Laptop açık, kullanıcı yok** | Orta | Çok yüksek | DPAPI korumaz (oturum açık) — disk full-disk encryption'a düşer (BitLocker zaten Win11 default) |
| **Malware, aynı kullanıcı** | Yüksek | Çok yüksek | DPAPI ve keyring korumaz. **`secrecy`/`zeroize` defansı sadece memory dump'a karşı**. Process injection'a karşı etkisiz. |
| **Lokal başka kullanıcı / RDP** | Düşük | Orta | DPAPI `Scope::User` korur |
| **Backup / cloud sync sızıntısı** | Düşük | Yüksek | DPAPI `Scope::User` makine-bound, **yedekleme güvenliği OS'e bağlı**. Argon2 + passphrase → backup'a çıksa bile güvenli. |
| **Supply chain (transitive dep compromise)** | Düşük-orta | Çok yüksek | `cargo audit` + `cargo deny` zaten ADR-0004'te var |
| **Discord ban (self-bot tespit)** | Orta | Hesap kaybı | Token'ı mükemmel saklamak ban'ı engellemez, ToS uyumu + rate-limit disiplini gerekir |

**Sonuç:** **DPAPI/Keychain/keyring tabanlı "OS-bound encryption" modern Windows için %95 yeterli.** Passphrase-based ek katman, ancak kullanıcı bilinçli olarak isterse (privacy-paranoid profil) veya backup senaryosu için anlamlı. Bu, plan'ın Bölüm 8'deki sorusunun cevabı olmalı.

---

## 2. Token Storage — Derinlemesine Karşılaştırma

### 2.1 `keyring` Ekosistemi Durumu (Haziran 2026)

`keyring` 2026'da **mimari değişiklik geçirdi**: 4.0 (Nisan 2026) ile **API kütüphanesi** (`keyring-core`) **store crate'lerinden** ayrıldı. Plan'da `keyring = "2.3"` yazıyor — bu sürüm **stale** ve ekosistem terk etti:

- `keyring 3.6.3` (Temmuz 2025) son "eski API" sürümü. ~495 reverse dep.
- `keyring 4.0.x` (Nisan-Mayıs 2026) yeni ekosistem. `keyring-core 0.7.4` + ayrı `windows-native-keyring-store 1.1.0` + `apple-native-keyring-store 1.0` + `dbus-secret-service-keyring-store 1.0` + `linux-keyutils-keyring-store 1.0`.
- **Walther Chen + Dan Brotsky + open-source-cooperative** tarafından aktif bakım.
- Maintainer'lar **resmen "yeni proje `keyring` değil, `keyring-core` + bir store crate'i kullansın"** diyor (`crates.io` keyring 4.0 README'sinde açık uyarı var).

#### `keyring-core` + `windows-native-keyring-store` pratikliği

```rust
// crates/viscos-auth/src/storage.rs
use keyring_core::{Entry, set_default_store, unset_default_store};
use windows_native_keyring_store::Store;

pub fn install() -> anyhow::Result<()> {
    set_default_store(Store::new()?);
    Ok(())
}

pub fn store_token(user_id: &str, token: &str) -> anyhow::Result<()> {
    let entry = Entry::new("Viscos", user_id)?;   // user_id = username
    entry.set_password(token)?;                    // DPAPI arkasında
    Ok(())
}

pub fn load_token(user_id: &str) -> anyhow::Result<Option<String>> {
    let entry = Entry::new("Viscos", user_id)?;
    match entry.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
```

**`default-features = false` + `search` feature'ı kapatırsak** `regex` dependency'si (1+ MB) **alınmaz** — binary bütçesi için kritik.

#### Trade-off'lar

| Konu | `keyring 2.3` (mevcut plan) | `keyring-core 0.7` + ayrı store (önerilen) |
|---|---|---|
| **Bakım** | Stale (sadece güvenlik patch). Walther Chen dep'yi `open-source-cooperative/keyring-rs`'ye taşıdı. | Aktif, haftalık release. |
| **MSRV** | 1.75 | 1.88 (Viscos 1.89 ile OK, ADR-0006) |
| **Binary etki** | Tek dep, ~55 KB | `keyring-core` (~30 KB) + `windows-native-keyring-store` (~20 KB) = ~50 KB. **Aslında daha küçük** çünkü v4 modüler ve `regex` opsiyonel. |
| **API** | `Entry::new(service, user)` — global singleton store | `set_default_store(Store::new()?)` + aynı API. Multi-store test/mockable. |
| **Mock/test** | Zor (singleton + gerçek OS'e bağlı) | Kolay (`keyring_core::mock::Store` built-in) |
| **Lisans** | MIT/Apache-2.0 | MIT/Apache-2.0 |

**Karar:** `keyring 2.3` **kullanılmamalı**. `keyring-core 0.7` + `windows-native-keyring-store 1.1` + ileride Linux/macOS için `dbus-secret-service-keyring-store` + `apple-native-keyring-store` eklenir (Faz 8 distribution'ında).

### 2.2 `windows-dpapi` Crate'i (Doğrudan DPAPI)

`sheridans/windows-dpapi` 0.2.0 (Mart 2026) — `CryptProtectData`/`CryptUnprotectData` üzerine ince, güvenli wrapper.

```rust
use windows_dpapi::{encrypt_data, decrypt_data, Scope};
let ct = encrypt_data(secret, Scope::User, Some(b"viscos-auth-v1"))?;  // 32 byte user-bound
let pt = decrypt_data(&ct, Scope::User, Some(b"viscos-auth-v1"))?;
```

**Avantaj:** Keyring'in altında zaten DPAPI var; ekstra katman yok. `Scope::User` = aynı kullanıcı, aynı makine.

**Dezavantaj:**
- **Anahtar yönetimi sana düşer** — nerede saklayacaksın? `config.toml`'da mı? "Anahtarı disk'te saklayıp DPAPI ile şifrelemek" → chicken-and-egg.
- **Microsoft Credential Manager UI'ı kaybolur** — kullanıcı "Viscos" entry'sini Windows Denetim Masası'nda göremez, silemez. **Dezavantaj**, çünkü kullanıcı şeffaflığı için Credential Manager UI'ı kıymetli.
- **Multi-process lock yok** — aynı anda iki Viscos instance çalışırsa yarış durumu.

**Karar:** Ana storage olarak **kullanma**. `secrecy` ile sarmalanmış secret materyali kısa süreliğine (in-memory) decrypt etmek için kullanılabilir, ama **kalıcı storage Credential Manager / keyring üzerinden** olmalı.

### 2.3 `secrecy` + `zeroize` (Bellek Hijyeni)

```rust
use secrecy::{Secret, ExposeSecret};
let token: Secret<String> = Secret::new(raw);
let exposed: &str = token.expose_secret();   // audit-grep noktası
// drop → otomatik zeroize
```

- **`secrecy 0.10.3`** (Rust 1.60+, MIT, `forbid(unsafe_code)`, `zeroize` tabanlı). Wrapper tip: `Debug`/`Clone` YOK → log'a kazara sızma yok, audit'te `expose_secret()` görünür.
- **`zeroize 1.x`** — `write_volatile` + `Ordering::SeqCst` fence ile compiler'ın silme atmasını engelliyor. **Spectre/Meltdown'a karşı garanti vermiyor** (dokümantasyon net).

**Trade-off:**
- ✅ Memory dump'a karşı baseline savunma.
- ✅ Type-level audit görünürlüğü (her `expose_secret()` call site review'da aranabilir).
- ❌ Process injection'a karşı **işe yaramaz** (aynı process address space).
- ❌ **Serde ile `Secret<String>` deserialize etmek istersen** serde feature gerekli, `Deserialize` Secret'a default değil.

**Plan'da `secrecy` "risk tablosu"nda geçiyor ama dependency olarak `Cargo.toml`'a eklenmemiş.** Bu bir **bug** — ADR-0007'ye aykırı değil ama plan eksik. **Öneri:** `viscos-auth` ve `viscos-api` her Secret<T> alanını `Secret<String>` veya `Secret<Vec<u8>>` ile sarsın. `zeroize` direk `viscos-auth`'a koyulur (moka/foyer zaten kendi cache'lerinde `Secret` ile sarıyor değil mi? `Secret` kullanmıyorlar, `Vec<u8>` → memcpy riski var. Ama bu konu dışı, ileride değerlendirilir).

### 2.4 `age` Crate'i (Encrypted File Backup / Passphrase KDF)

`age 0.11` (FiloSottile) passphrase-based symmetric encryption, **scrypt** default KDF, ChaCha20-Poly1305 AEAD.

**Kullanım senaryosu:** Kullanıcı "Viscos'u yedekle/geri yükle" derse, **DPAPI-bound** token dosyasını **passphrase ile şifrelenmiş envelope**'a kopyalayabilir — backup güvenliği için. Yedekleme senaryosu dışında **default depolama için uygun değil** (her açılışta passphrase sormak UX'i öldürür).

**Karar:** Backup/export flow'u (Faz 5+ / v2.0 multi-account) için **depolama katmanı olarak değil, ek encryption envelope olarak** kullan. **Default akışta yok.**

### 2.5 `passphrase`-bound Key Wrap (Argon2id + AES-GCM)

OWASP 2024 + RFC 9106 önerisi: **Argon2id** (m ≥ 19 MiB, t = 2, p = 1) → 256-bit KEK → **AES-GCM** ile token'ı wrap et. Salt + nonce + ciphertext dosyada; passphrase kullanıcıdan.

**Trade-off:**
- ✅ Backup-safe (passphrase olmadan decrypt edilemez).
- ✅ Disk çalınsa bile güvenli.
- ❌ **UX ölümcül** — her açılışta passphrase sor. Discord zaten 30 gün-1 yıl token ömrü veriyor, "güvenli" saklama zaten OS-bound; passphrase **sadece iki durumda** gerekli:
  1. Privacy-paranoid kullanıcı (Signal, Bitwarden seviyesi).
  2. Multi-machine roaming (DPAPI makine-bound, başka makinede token lazım olunca passphrase sorulur).

**Karar:** **v1 default'unda yok**, **opt-in** "ek passphrase" olarak v1.5 / v2.0'da eklenebilir. Plan'ın Bölüm 8'deki C seçeneği (hybrid) **default = DPAPI, opt-in = passphrase wrapper** olarak netleştirilmeli.

### 2.6 `Windows Hello` / TPM-Bound KSP (NCrypt)

Microsoft'un Passport / Windows Hello API'si: AES-256-GCM blob, **içerik anahtarı TPM/Hello KSP key ile wrap'lenir**, decrypt **interaktif PIN/biometric** gerektirir. Go'da `keyring` ekosistemi (`ByteNess/keyring`) destekliyor, Rust tarafında **doğrudan crate yok**. `windows-rs` + `NCrypt*` ile yazılabilir ama **3+ hafta extra iş**.

**Trade-off:**
- ✅ En güçlü savunma: process bile çalışsa TPM unlock olmadan decrypt edilemez.
- ❌ Biometric destekleyen cihaz gerektirir (çoğu modern laptop OK).
- ❌ Headless server / RDP oturumunda çalışmaz.
- ❌ **Viscos v1 için over-engineering** — kullanıcı kitlesi Discord istemcisi, banka değil.

**Karar:** v1 **yok**, v3 / Linux-port'ta değerlendirilir (Linux'ta `linux-keyutils-keyring-store` zaten kernel keyring'i kullanıyor, ek TPM stack'ı gereksiz).

---

## 3. MFA (TOTP) — `totp-rs 5.7.1` Yeterli mi?

Mevcut plan: `totp-rs = "5.0"`. **5.7.1 (Mart 2026) en güncel, semver uyumlu.**

| Konu | Durum |
|---|---|
| **RFC 6238 uyumu** | ✅ SHA1/SHA256/SHA512 |
| **`otpauth://` URL parse** | ✅ `otpauth` feature ile |
| **Backup codes** | ❌ `totp-rs` yok. Discord backup codes 8 karakterli alphanumeric; **elle handle etmek 50 satır** (sha256 → base32 → 8 char). |
| **QR render (UI için)** | `qrcode 0.14` + `qrcodegen-image` (totp-rs feature). Viscos QR login için kullanıyor zaten. |
| **MFA üretimi (kullanıcıyı kurmak)** | **Yapmıyoruz** — kullanıcı kendi authenticator'ından giriyor. Doğru. |

**Discord'a gönderim:** `POST /auth/mfa/totp` body `{ "code": "123456", "ticket": "..." }` — `totp-rs` doğrulamak için **kullanılmıyor**, sadece Discord API'ına iletiliyor. **Discord TOTP secret'ı kullanıcı tarafında, Viscos tarafında üretilmiyor.** Doğru yaklaşım.

**Eksik: Backup codes.** Plan'da yok. Discord MFA kurulumunda kullanıcıya 8 backup code veriliyor, hesap kurtarma için. Bunlar Viscos'ta **ayrıca saklanmalı** (yine keyring entry). Bölüm 4.4'e eklenmeli.

---

## 4. QR Login — Pratik Detaylar

Mevcut plan: `qrcode 0.14` + `/auth/qr-login/start` + polling. **Doğru yaklaşım.** Discord resmi istemci aynı protokolü kullanıyor.

**Eksik detaylar:**
1. **QR login token'ı "user token" değil, geçici.** Discord QR flow'u `https://discord.com/ra/{session_id}` URL üretir, mobil app tarayınca WebSocket üzerinden onay gelir, döndürülen token **normal user token** ile aynı — kalıcı. Doğru.
2. **CSRF / replay**: `session_id` tek kullanımlık, polling sırasında expire olabilir (`QrPollResponse::Expired`). Plan'da handle ediliyor. ✅
3. **2FA gerekiyorsa**: QR login sonrası `/auth/login` ile aynı şekilde `LoginResult::MfaRequired { ticket, mfa_type }` dönebilir. Plan'da **QrLoginResponse enum'unda MFA handling belirsiz** — eklenmeli.
4. **Captcha**: `/auth/login` Cloudflare Turnstile veya hCaptcha dönebilir. Discord son 1-2 yılda agresifleşti. **Mevcut planda captcha handling yok** — bu Faz 2'nin **en büyük riski**. Kullanıcıya "bu hesap için tarayıcıdan giriş yap, token'ı yapıştır" yönlendirmesi veya headless browser açılması (Playwright?) gerekir. `plan §8` Karar Noktası olarak işlenmeli.

---

## 5. X-Super-Properties — Fingerprint Yönetimi

Discord Web client'ın **fingerprint**'i:
- Browser UA, screen, locale, release_channel, client_build_number, **WebGL hash**, **AudioContext hash**, **canvas hash**, **timezone offset**, **cookie_enabled**...
- `viscos-auth` bu değerleri **sabit bir profil** olarak üretmeli (her login'de aynı fingerprint).

**Mevcut plan:** `X-Super-Properties` üretimi `viscos-auth`'a konmuş. ✅

**Ama detay eksik:**
1. **Versiyon senkronizasyonu**: Discord `client_build_number` haftalık artıyor. Twilight veya başka bir kaynak bu numarayı **gerçek zamanlı** çekmiyor (Discord proxy'si sıkı rate-limit'li). Viscos'un stratejisi ne?
   - **Seçenek 1 (önerilen):** GitHub `Visos-D/generate-super-properties` veya benzer community project → her build'de script çek, sabit profile yapıştır, `cargo:rerun-if-changed`.
   - **Seçenek 2:** Runtime'da `GET https://discord.com/api/v10/gateway` → `X-Super-Properties` header'ı response'tan parse → ancak Discord bu header'ı public endpoint'te dönmüyor.
   - **Seçenek 3 (pratik):** Discord Web'in **kendi JS bundle**'ından `release_channel` ve `build_number` oku (CDN'den, `https://discord.com/app`). Bu haftalık elle güncelleme gerektirir.

2. **WebGL/Canvas hash'i**: Bu hash **gerçek GPU + tarayıcıya** bağlı. **Native** bir uygulamada bu hash'i üretmek için sahte WebGL/Canvas context yaratmak gerek (CEF/WebView2 kullanıyor olabiliriz!). **Vesktop bunu CEF renderer'ından** alıyor. **Viscos için:** Win11 default CEF (Faz 1.6) → WebView2 wrapper'ın WebGL context'i → oradan hash üret. **Win10 default WebView2** → aynı. **Bu konu `webview2-hardening.md` veya `phase-2.0`'a not düşülmeli.**

3. **"Stable" fingerprint riski**: Discord **fingerprint değişmezse** suspicious; **çok sık değişirse** suspicious. Orta yol: her 24 saatte **build_number güncelle**, diğer alanlar sabit. Bu konu Faz 5 polish'ine.

**Karar:** Mevcut plan **eksik** ama **MFA seviyesinde risk değil** (Discord fingerprint'i token'ı saklama katmanıyla ilgili değil, transport katmanıyla ilgili). Yeni ADR'ye gerek yok, `phase-2.0` Bölüm 3.3'te detaylandırılmalı.

---

## 6. Token Rotation & Invalidation

Mevcut planda **yok**. Pratik senaryolar:

| Senaryo | Mevcut plan davranışı | Olması gereken |
|---|---|---|
| Discord 401 döndü (token iptal) | Twilight `Error` tipine map edilir | `viscos-auth` 401'i handle edip `keyring::delete_credential` çağırmalı, kullanıcıya "tekrar giriş yap" UI'ı göstermeli |
| Kullanıcı "Çıkış Yap" | `delete_token()` plan'da var | ✅ |
| Kullanıcı parolasını Discord'da değiştirdi | Plan'da yok | Token invalidate olur, 401 alırız → yukarıdaki path |
| Token çalınma bildirimi (Discord "delete token" özelliği) | Plan'da yok | "Settings → Connected Apps → Viscos Remove" UI'ı + `POST /oauth2/token/revoke` |
| Multi-account (v2.0) | "v2.0'a atılmış" | **Şimdiden** `keyring` user_id bazlı key'leme (plan'da `username: "user_token"` → bunu user_id yap). v1 single-account'ta bile her user için ayrı entry → ileride 0 refactor. |

**Öneri:** Plan'ın `StoredToken` yapısı şu hale gelmeli:

```rust
const SERVICE_NAME: &str = "Viscos";   // service adı uygulama adı
                                            // user = Discord user_id (snowflake)
// keyring'de her account ayrı entry:
//   "Viscos" / "123456789012345678" = JSON { token, user_id, username, mfa_backup_codes_hashed, ... }
```

---

## 7. Encryption Anahtarı — Bölüm 8'in Cevabı

`phase-2.0` Bölüm 8 açıkça "İNSAN: Token storage encryption anahtarı nereden gelecek?" diye soruyor. Cevap:

### Önerilen: **Varyant A (DPAPI/Keyring) default, Varyant B (Argon2id passphrase) opt-in**

**Gerekçe:**
1. **Threat model (Bölüm 1)**: DPAPI %95 yeterli. Passphrase UX öldürür.
2. **Discord token'ı "parola" değil**: 30 gün-1 yıl ömür, revoke edilebilir. Banka hesabı gibi koruma orantısız.
3. **Industry comparables**: Vesktop (Electron) `safeStorage` (DPAPI arkası), Dissent (Go/Linux) Secret Service. Hiçbiri passphrase zorlamıyor.
4. **Karmaşıklık maliyeti**: Passphrase = Argon2id parametre tuning + UX akışı (recovery flow!) + backup senaryosu. AI-yazılım projesi için yüksek risk.

**Plan'a eklenecek net karar:**

```toml
# crates/viscos-auth/Cargo.toml
[dependencies]
keyring-core = { version = "0.7", default-features = false }  # search feature kapalı → regex yok
windows-native-keyring-store = { version = "1.1", default-features = false }  # search feature kapalı
totp-rs = { version = "5.7", default-features = false, features = ["zeroize"] }
qrcode = "0.14"
secrecy = { version = "0.10", features = ["serde"] }
zeroize = { version = "1", features = ["derive"] }
thiserror = "2"

# Future (Faz 5+ opt-in passphrase):
# argon2 = { version = "0.5", features = ["std"] }
# aes-gcm = "0.10"
```

**v1 storage stratejisi:**
- **Token** → `keyring` (DPAPI arkası, OS-bound). `Secret<String>` ile sarmalanmış in-memory.
- **Backup codes (MFA)** → aynı keyring entry, ayrı alan olarak.
- **X-Super-Properties** → **keyring'de değil**, `config.toml` veya `%APPDATA%/viscos/super_properties.json`'da (kullanıcı başına, makine-bound değil — fingerprint'in kendisi hassas değil).
- **Encryption passphrase** → yok (v1), v2'de opt-in (Faz 5+).

**Multi-machine roaming senaryosu v1'de YOK** — kullanıcı farklı makinede tekrar login olur. v2.0 multi-account ile passphrase'li export/import gelir.

---

## 8. Self-Bot ToS Riski — Auth Planı İçin Net Öneri

Mevcut plan §8 disclaimer dialog'u öneriyor. **Doğru ama eksik:**

1. **İlk açılışta modal disclaimer** ✅ (önerilen).
2. **Settings → About** bölümünde kalıcı not.
3. **README + DECISIONS ADR-0011**'de net: "Viscos **user token + native UI wrapper**, **selfbot değil**. Token'la otomasyon (kendi adına mass DM, scraping) ToS ihlali ve bu istemci için **planlanmamış API yüzeyi yok**."
4. **Twilight kullanımı**: `twilight-model` + `twilight-http` + `twilight-gateway` user-token ile **resmi rate-limit bucket** kullanır, bot-token bucket'ı değil. ✅
5. **Risk azaltma** (ADR-0008'de de var): rate-limit, doğru user-agent, fingerprint temiz.

**Ek:** `viscos-auth` **asla** `/users/@me` dışında **bot-only endpoint'lerini çağırmamalı** (slash command registration, gateway bot intent'leri, vs.). Twilight'ın default `Client::new(token)` user-mode, ama `viscos-api` build aşamasında compile-time flag'le doğru mode seçmeli. **Öneri:** `viscos-api` `X-Discord-Token-Type: User` header'ı eklesin (Discord bunu parse ediyor, **user token kullandığınızı** zaten biliyor; net olmak için).

---

## 9. Storage Stratejisi — Önerilen Şema

```rust
// crates/viscos-auth/src/lib.rs
use keyring_core::{Entry, set_default_store, unset_default_store, Error as KeyringError};
use secrecy::{Secret, ExposeSecret, Zeroize};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use zeroize::ZeroizeOnDrop;

const SERVICE_NAME: &str = "Viscos";

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("keyring error: {0}")]
    Keyring(#[from] KeyringError),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("no account found for user {0}")]
    AccountNotFound(String),
    #[error("token validation failed: {0}")]
    ValidationFailed(String),
}

#[derive(ZeroizeOnDrop)]
pub struct StoredAccount {
    pub user_id: String,        // Discord snowflake
    pub username: String,       // tag (username#discriminator veya yeni unique username)
    pub token: Secret<String>,  // secrecy ile sarılmış
    pub mfa_backup_hashes: Vec<Secret<String>>,  // argon2 PHC strings (v2'de)
}

#[derive(Serialize, Deserialize, ZeroizeOnDrop)]
struct SerializedAccount {
    user_id: String,
    username: String,
    token: String,
    mfa_backup_hashes: Vec<String>,
    created_at: i64,            // Unix timestamp
    last_validated_at: i64,
}

pub struct AuthStorage {
    // store runtime'da set_default_store ile global olarak kurulur;
    // struct boş, sadece helper metotlar için
    _private: (),
}

impl AuthStorage {
    /// Platform-specific store'u kur. v1: Windows native (DPAPI arkası).
    /// v2+ Linux/macOS'ta Store trait'ini dispatch eder.
    pub fn install() -> Result<(), AuthError> {
        #[cfg(target_os = "windows")]
        {
            use windows_native_keyring_store::Store;
            set_default_store(Store::new()?).map_err(AuthError::Keyring)?;
        }
        #[cfg(target_os = "macos")]
        { /* apple-native-keyring-store::Store::new() */ unimplemented!() }
        #[cfg(target_os = "linux")]
        { /* dbus-secret-service-keyring-store::Store::new() */ unimplemented!() }
        Ok(())
    }

    pub fn shutdown() {
        unset_default_store();
    }

    pub fn store_account(&self, account: &StoredAccount) -> Result<(), AuthError> {
        let entry = Entry::new(SERVICE_NAME, &account.user_id)?;
        let ser = SerializedAccount {
            user_id: account.user_id.clone(),
            username: account.username.clone(),
            token: account.token.expose_secret().clone(),
            mfa_backup_hashes: account.mfa_backup_hashes.iter()
                .map(|s| s.expose_secret().clone())
                .collect(),
            created_at: now(),
            last_validated_at: now(),
        };
        let json = serde_json::to_string(&ser)?;
        entry.set_password(&json)?;
        Ok(())
    }

    pub fn load_account(&self, user_id: &str) -> Result<Option<StoredAccount>, AuthError> {
        let entry = Entry::new(SERVICE_NAME, user_id)?;
        match entry.get_password() {
            Ok(json) => {
                let ser: SerializedAccount = serde_json::from_str(&json)?;
                Ok(Some(StoredAccount {
                    user_id: ser.user_id,
                    username: ser.username,
                    token: Secret::new(ser.token),
                    mfa_backup_hashes: ser.mfa_backup_hashes.into_iter()
                        .map(Secret::new)
                        .collect(),
                }))
            }
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_account(&self, user_id: &str) -> Result<(), AuthError> {
        let entry = Entry::new(SERVICE_NAME, user_id)?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(KeyringError::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn list_accounts(&self) -> Result<Vec<String>, AuthError> {
        // keyring-core'un search özelliği — `default-features = false` kapalıysa YOK.
        // v1 multi-account yok → bu metoda gerek yok.
        // v2'de `keyring-core`'u `search` feature ile enable et.
        Ok(vec![])
    }
}
```

**Önemli noktalar:**
- **`keyring-core` `default-features = false`** + **`windows-native-keyring-store` `default-features = false`** → `regex` dependency'si alınmaz (1+ MB kazanç).
- **`service = "Viscos"`, `user = user_id`** — v2 multi-account path'i 0-refactor.
- **`Secret<String>` + `ZeroizeOnDrop`** → in-memory hijyen.
- **`SerializedAccount`'a `last_validated_at`** → 401 alındığında kullanıcıya "X gün önce doğrulandı" gösterebiliriz.

---

## 10. Toparlama: Önerilen Değişiklikler (Plan Patch)

### 10.1 Yeni ADR: ADR-0011 — Auth Stack (Haziran 2026)

`docs/DECISIONS.md`'ye eklenecek. İçerik özeti:

- **Durum:** 🟡 Proposed (insan onayı bekliyor)
- **Karar:** `keyring-core 0.7` + `windows-native-keyring-store 1.1` (default-features = false) + `totp-rs 5.7` + `secrecy 0.10` + `zeroize 1` (derive feature).
- **Değişen:** `keyring 2.3` → `keyring-core 0.7` + ayrı store (4.0 mimari gereği).
- **Encryption anahtarı:** Varyant A (DPAPI/Keyring default), Varyant B (Argon2id passphrase) **v2.0'da opt-in**.
- **Multi-account:** v1'de `keyring` user_id bazlı key'lenir, v2.0'da list/search açılır.
- **MFA backup codes:** v1'de saklanır (ayrı keyring alanı), validation Argon2 PHC.
- **Captcha:** `phase-2.0` Karar Noktası'na eklenmeli (Varyant: tarayıcı redirect vs headless browser).
- **X-Super-Properties:** `phase-2.0` §3.3'te detaylandırılmalı (build_number senkronizasyonu, WebGL hash nereden alınır).
- **ToS disclaimer:** `phase-2.0` §8 + README'de net.
- **Review trigger:** Keyring-core 1.0 çıkarsa; Discord `/auth/login` rate-limit politikasını değiştirirse; CAPTCHA zorunlu hale gelirse.

### 10.2 `phase-2.0-discord-api.md` Patch Listesi

| Bölüm | Patch |
|---|---|
| §2 Cargo.toml | `keyring = "2.3"` → `keyring-core = "0.7"`, `windows-native-keyring-store = "1.1"` (default-features = false), `totp-rs = "5.7"`, `secrecy = "0.10"`, `zeroize = "1"`. |
| §3.3 X-Super-Properties | WebGL/Canvas hash üretimi (CEF/WebView2'den), build_number senkronizasyon stratejisi (haftalık script). |
| §3.4 Endpoint tablosu | `/auth/login` → captcha handling notu; `/auth/qr-login/*` → MFA-ticket path eklenecek. |
| §4.1 AuthStorage | `keyring` 4.0 API'sine uygun `set_default_store` + `Entry::new` rewrite. `service="Viscos"`, `user=user_id`. |
| §4.1 StoredToken | `SerializedAccount`'a expand: `mfa_backup_hashes`, `created_at`, `last_validated_at`. |
| §4.3 MFA | TOTP + **backup codes** bölümü ekle (8-char alphanumeric, Argon2 PHC storage). |
| §5 LoginState enum | `MfaBackupCode` state ekle. |
| §6 Test tablosu | Captcha handling test (mock); MFA backup code test; multi-keyring entry test. |
| §7 Acceptance | "Captcha yoksa token yapıştırma fallback" acceptance. |
| §8 Karar Noktası | Encryption anahtarı: **Varyant A (DPAPI) default, Varyant B passphrase opt-in v2.0'da**; Captcha stratejisi: **redirect-to-browser (önerilen) vs headless**; MFA backup codes saklama. |
| §9 Riskler | Captcha akışı risk satırı; keyring-core 4.0 API drift riski; Discord fingerprint rotation riski. |

### 10.3 `docs/DECISIONS.md` Patch Listesi

- **ADR-0011** (yeni): Yukarıdaki §10.1.
- **ADR-0008** dipnot: "`viscos-auth` twilight'dan bağımsız; user token modunda compile-time enforcement" notu.

---

## 11. Kısa Özet — TL;DR

| Karar | Durum | Öneri |
|---|---|---|
| **keyring** | `2.3` stale | **`keyring-core 0.7` + `windows-native-keyring-store 1.1`** (default-features = false) |
| **DPAPI direkt kullanımı** | Plan'da konuşulmamış | **Hayır**, keyring zaten DPAPI'ı kullanıyor. Doğrudan kullanmak ekstra complexity, UI şeffaflığı kaybı. |
| **Encryption passphrase** | Açık soru | **Varyant A default (DPAPI)**, Varyant B (Argon2id) **v2.0 opt-in**. Plan Bölüm 8 kapatılmalı. |
| **MFA (TOTP)** | `totp-rs 5.0` OK | **`totp-rs 5.7.1`** (semver uyumlu minor). Backup codes eklenmeli. |
| **secrecy / zeroize** | Risk tablosunda var, dependency yok | **Ekle**: `secrecy 0.10` (serde feature) + `zeroize 1` (derive feature). `Secret<String>` ve `ZeroizeOnDrop` her secret materyal için zorunlu. |
| **Multi-account** | v2.0'a atılmış | **v1'den itibaren `user = user_id` key'le**, refactor maliyeti 0. |
| **Captcha handling** | Plan'da YOK | **Ekle** (Bölüm 8 karar noktası): önerilen = "tarayıcıya yönlendir, token'ı yapıştır". |
| **X-Super-Properties** | Plan'da "viscos-auth üretir" | **Detaylandır**: build_number senkronizasyonu, WebGL hash kaynağı (CEF/WebView2). |
| **ToS disclaimer** | Plan'da var | **Modal + Settings + README + ADR'de** hepsinde tek metin. |
| **age (passphrase file)** | Plan'da yok | **Backup/export envelope** olarak v2.0'da ekle. v1'de yok. |
| **TPM / Windows Hello** | Plan'da yok | **v1'de yok**, v3 / Linux-port'ta değerlendir. |

**Net sonuç:** Mevcut plan **%70 doğru yönde**, ama **keyring 2.3 stale** ve **encryption anahtarı kararı açık**. Toplam patch: 1 yeni ADR (~150 satır), `phase-2.0` içinde ~80 satır değişiklik, `Cargo.toml` workspace dependency'lerinde 2 satır değişiklik. **2-3 hafta ekstra AI-PR yükü değil**, doğrudan Faz 2'ye yedirilir.

---

## 12. Açık Sorular (İnsan Onayı)

1. **Captcha stratejisi** — Tarayıcıya yönlendir (önerilen, sade) vs headless browser (Playwright/Firefox) mı?
2. **MFA backup codes** — Saklansın mı, saklanmasın mı? Saklanacaksa Argon2 PHC, plaintext, yoksa tamamen kullanıcının sorumluluğu mu?
3. **Multi-account v1** — Tamamen gizli mi (keyring user_id key'le, v2 açılır), yoksa v1'de UI'da görünsün mü?
4. **X-Super-Properties build_number senkronizasyonu** — Haftalık elle mi, GitHub Action ile otomatik PR mı?
5. **Discord QR login sonrası MFA** — Eğer hesap 2FA istiyorsa, QR başarı sonrası ikinci adım mı yoksa QR flow zaten MFA atlamış mı olur? (Discord davranışı netleştirilmeli.)
6. **`keyring-core 0.7 → 1.0` major bump** olduğunda plan ne yapsın? Pin mi, allow minor+patch only mi?

