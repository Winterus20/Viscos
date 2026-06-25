//! Authenticode code signing (Faz 8.0 release engineering).
//!
//! Faz 8.0 kapsamı: **`signtool.exe` invocation + test PFX fixture**.
//! Production sertifikası Faz 8.x release engineering'inde (insan PR —
//! sertifika maliyeti, OV/EV kararı, `.cursorrules` §4).
//!
//! Production stratejisi (Faz 8.0 karar noktası, ADR-0006 ile uyumlu):
//! - v1: self-signed (ücretsiz, "Unknown publisher" uyarısı).
//! - v2: OV sertifika ($200-400/yıl, SmartScreen OK).
//! - v3: EV sertifika ($400-800/yıl, anında trust).
//!
//! Cross-reference:
//! - [`phase-8.0-distribution.md` §6](../../.cursor/plans/phase-8.0-distribution.md#6-code-signing-windows-authenticode)
//!
//! # Security
//!
//! Bu modülün güvenlik modeli beş temel direğe dayanır:
//!
//! 1. **Sertifika erişimi.** `.pfx` dosyası **asla** repoya commit edilmez
//!    (`.cursorrules` §15 "Sırları / API anahtarlarını / private key'i commit
//!    etme"). `SignerConfig::cert_path` sadece dosya yolunu tutar; production'da
//!    `certs/release.pfx` olarak release engineering makinesinde lokal tutulur.
//!    v1 self-signed için geliştirici makinesinde `certs/viscos-dev.pfx` kullanılır.
//!
//! 2. **Şifre taşınması.** `SignerConfig::cert_password_env` **şifrenin
//!    kendisini değil**, şifreyi tutan env var adını saklar. Production şifre
//!    CI secret olarak inject edilir (`VISCOS_CERT_PASSWORD`); lokal
//!    geliştirmede `.env` (gitignored) veya shell session'dan okunur.
//!    Şifre `zeroize::Zeroizing<String>` wrapper'ı ile birlikte kullanılır;
//!    drop anında bellekten silinir (ADR-0011 hizalı).
//!
//! 3. **İmzalama ortamı.** `signtool.exe` invocation yalnızca **CI / release
//!    engineering makinesinde** çalışır; geliştirici makinesi ve runtime'da
//!    **imzalama YOK**. `.cursorrules` §15 "Production binary'yi CI dışında
//!    elle imzalama" YASAK. Geliştirici makinesinde `sign()` çağrısı test
//!    fixture'ı üzerinde koşturulabilir (self-signed PFX), ancak bu çağrı
//!    runtime'da tetiklenmez.
//!
//! 4. **Timestamp authority.** `SignerConfig::timestamp_url` RFC 3161 TSA
//!    endpoint'i (default DigiCert). Sertifika expire olsa bile imza geçerli
//!    kalır (counter-signature). TSA URL'i değiştirilirse yeni release
//!    zincirinin SHA-256 thumbprint'i değişir — release notes'a yansıtılmalı.
//!
//! 5. **Hash algoritması.** Default SHA-256; SHA-384/512 opsiyonel. MD5/SHA-1
//!    ASLA kabul edilmez (Windows SmartScreen reject eder).

use std::path::{Path, PathBuf};

use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use viscos_error::{Result, ViscosError};
use zeroize::Zeroizing;

/// Code signer konfigürasyonu.
///
/// `cert_path` + `cert_password_env` çifti sertifikayı environment'tan
/// güvenli şekilde çeker; şifre hiçbir zaman struct içinde literal olarak
/// saklanmaz (Security §2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerConfig {
    /// `signtool.exe` mutlak yolu. `None` ise Windows SDK default konumları
    /// taranır (`C:\Program Files (x86)\Windows Kits\10\bin\<sdk_ver>\x64\signtool.exe`),
    /// bulunamazsa PATH'e fallback.
    #[serde(default)]
    pub signtool_path: Option<PathBuf>,
    /// `.pfx` sertifika path'i. Lokal veya CI runner'da bulunur; **asla commit edilmez**.
    pub cert_path: PathBuf,
    /// Sertifika parolası (env var adı — şifrenin kendisi değil). Production'da
    /// CI secret'tan inject edilir, `.env` ile lokal geliştirmede okunur.
    pub cert_password_env: String,
    /// RFC 3161 timestamp authority URL'i (default: DigiCert).
    pub timestamp_url: String,
    /// Hash algoritması (`sha256` | `sha384` | `sha512`).
    pub hash_algorithm: String,
    /// Subject alternative name / description (`/d` flag).
    #[serde(default = "default_description")]
    pub description: String,
    /// Publisher URL (`/du` flag).
    #[serde(default = "default_publisher_url")]
    pub publisher_url: String,
}

fn default_description() -> String {
    "Viscos Discord Client".to_string()
}

fn default_publisher_url() -> String {
    "https://viscos.app".to_string()
}

impl SignerConfig {
    /// Default `signtool` timestamp URL'i (DigiCert).
    ///
    /// RFC 3161 TSA endpoint. DigiCert global root'a sahip olduğu için SmartScreen
    /// dahil tüm Windows trust chain'lerinde doğrulanır.
    #[must_use]
    pub const fn default_timestamp_url() -> &'static str {
        "http://timestamp.digicert.com"
    }

    /// v1 önerisi: self-signed, SHA-256.
    ///
    /// Self-signed sertifika SmartScreen'de "Unknown publisher" uyarısı
    /// gösterir; Faz 8.x'te OV/EV sertifikası alındığında bu default
    /// değiştirilecektir (breaking config — ADR gerektirir).
    #[must_use]
    pub fn self_signed_default() -> Self {
        Self {
            signtool_path: None,
            cert_path: PathBuf::from("certs/viscos-dev.pfx"),
            cert_password_env: "VISCOS_CERT_PASSWORD".to_string(),
            timestamp_url: Self::default_timestamp_url().to_string(),
            hash_algorithm: "sha256".to_string(),
            description: "Viscos Discord Client".to_string(),
            publisher_url: "https://viscos.app".to_string(),
        }
    }
}

/// Code signing hatası.
#[derive(Error, Debug)]
pub enum SignerError {
    /// `signtool.exe` PATH'te ve SDK default konumlarında bulunamadı.
    #[error("signtool executable not found (set SignerConfig::signtool_path)")]
    SigntoolMissing,
    /// Sertifika dosyası belirtilen path'te yok.
    #[error("certificate file missing: {0}")]
    CertMissing(PathBuf),
    /// `cert_password_env` env var'ı tanımsız veya boş.
    #[error("certificate password env var missing or empty: {0}")]
    PasswordEnvMissing(String),
    /// `signtool.exe` çalıştırıldı ama exit code != 0.
    #[error("signtool invocation failed (exit {exit_code}): {stderr}")]
    SigntoolFailed {
        /// signtool exit kodu.
        exit_code: i32,
        /// signtool stderr çıktısı.
        stderr: String,
    },
    /// signtool child process başlatılamadı (OS-level).
    #[error("signtool spawn failed: {0}")]
    Spawn(#[from] std::io::Error),
    /// Hedef binary okunamadı (SHA-256 hesabı için).
    #[error("target binary unreadable: {0}")]
    TargetUnreadable(PathBuf),
}

impl From<SignerError> for ViscosError {
    fn from(err: SignerError) -> Self {
        ViscosError::Io(std::io::Error::other(format!("signer: {err}")))
    }
}

/// İmzalanmış artifact metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedArtifact {
    /// İmzalanmış binary'nin path'i.
    pub path: PathBuf,
    /// Binary'nin SHA-256'sı (signtool çağrısı sonrası, signtool çıktısındaki
    /// `SignedHash` veya dosyanın kendi SHA-256'sı).
    pub sha256: String,
    /// Sertifika thumbprint (signtool `/sha1` thumbprint, lower-hex).
    /// Thumbprint signtool `/sha1 <thumbprint>` ile eşleşmesi gereken SHA-1
    /// hash'idir; release notes'a bu değer yazılır.
    pub thumbprint: String,
    /// Kullanılan timestamp URL'i.
    pub timestamp_url: String,
    /// Kullanılan hash algoritması (`sha256` / `sha384` / `sha512`).
    pub hash_algorithm: String,
}

/// Code signer handle.
///
/// `CodeSigner::sign()` Faz 8.0+ implementasyonu gerçek `signtool.exe`
/// invocation yapar. Yalnızca **release engineering** ortamında
/// (CI runner + sertifika erişimi) çalıştırılmalıdır; runtime'da
/// imzalama YOK (Security §3).
#[derive(Debug, Clone)]
pub struct CodeSigner {
    config: SignerConfig,
}

impl CodeSigner {
    /// Yeni code signer.
    ///
    /// Config ile birlikte handle oluşturur. Config'in `cert_password_env`
    /// env var'ı resolve edilmez — yalnızca saklanır (Security §2).
    #[must_use]
    pub const fn new(config: SignerConfig) -> Self {
        Self { config }
    }

    /// Self-signed default config ile signer.
    ///
    /// v1 release öncesi geliştirme/testing için uygundur; production release
    /// OV/EV sertifikası ile `SignerConfig::self_signed_default()` çağrısını
    /// override ederek ayrı config kurulmalıdır.
    #[must_use]
    pub fn self_signed() -> Self {
        Self::new(SignerConfig::self_signed_default())
    }

    /// Config'i döndür.
    #[must_use]
    pub const fn config(&self) -> &SignerConfig {
        &self.config
    }

    /// Binary'yi `signtool.exe` ile imzala.
    ///
    /// Faz 8.0+ davranışı: `signtool sign /fd SHA256 /tr <tsa_url> /td SHA256
    /// /f <cert.pfx> /p <password> /d <desc> /du <url> <target>` çağrısı.
    /// Başarı durumunda hedef binary'nin SHA-256'sını hesaplar ve
    /// `SignedArtifact` döndürür. Şifre `Zeroizing<String>` ile sarılıdır
    /// ve drop anında bellekten silinir.
    ///
    /// # Errors
    ///
    /// - `SignerError::SigntoolMissing` — `signtool.exe` bulunamadı.
    /// - `SignerError::CertMissing` — `.pfx` dosyası yok.
    /// - `SignerError::PasswordEnvMissing` — env var tanımsız.
    /// - `SignerError::SigntoolFailed` — exit code != 0.
    /// - `SignerError::Spawn` — child process başlatılamadı.
    /// - `SignerError::TargetUnreadable` — hedef binary okunamadı.
    pub async fn sign(&self, binary: &Path) -> Result<SignedArtifact> {
        let signtool = locate_signtool(self.config.signtool_path.as_deref())?;
        if !self.config.cert_path.exists() {
            return Err(SignerError::CertMissing(self.config.cert_path.clone()).into());
        }
        let password_env = &self.config.cert_password_env;
        let password = std::env::var(password_env)
            .map_err(|_| SignerError::PasswordEnvMissing(password_env.clone()))?;
        if password.is_empty() {
            return Err(SignerError::PasswordEnvMissing(password_env.clone()).into());
        }
        // SecretBox + Zeroizing: klonu signtool'a argüman olarak geçtikten sonra
        // scope bittiğinde Zeroizing drop'ı belleği siler.
        let password_secret: SecretBox<String> = SecretBox::new(Box::new(password));

        let cert_path_str = self
            .config
            .cert_path
            .to_str()
            .ok_or_else(|| SignerError::CertMissing(self.config.cert_path.clone()))?;
        let target_str = binary
            .to_str()
            .ok_or_else(|| SignerError::TargetUnreadable(binary.to_path_buf()))?;

        // signtool password'u `/p` flag'i ile alır; Secret::expose_secret ile
        // geçici referans. Argümanlar signtool process'inin arg list'ine kopyalanır
        // (Windows: command line string; Linux: argv). Arg list scope'u sona erince
        // Zeroizing drop edilir.
        let password_str = password_secret.expose_secret();
        let password_arg: Zeroizing<String> = Zeroizing::new(password_str.clone());

        tracing::info!(
            binary = %target_str,
            cert = %cert_path_str,
            timestamp = %self.config.timestamp_url,
            hash = %self.config.hash_algorithm,
            signtool = %signtool.display(),
            "CodeSigner::sign: invoking signtool"
        );

        let output = tokio::process::Command::new(&signtool)
            .arg("sign")
            .args(["/fd", &self.config.hash_algorithm])
            .args(["/tr", &self.config.timestamp_url])
            .args(["/td", &self.config.hash_algorithm])
            .args(["/f", cert_path_str])
            .arg("/p")
            .arg(password_arg.as_str())
            .args(["/d", &self.config.description])
            .args(["/du", &self.config.publisher_url])
            .arg(target_str)
            .output()
            .await?;

        // password_arg Zeroizing drop edilir; bellek silinir.
        drop(password_arg);
        drop(password_secret);

        if !output.status.success() {
            let exit_code = output.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            tracing::error!(
                exit_code,
                stderr = %stderr,
                "signtool invocation failed"
            );
            return Err(SignerError::SigntoolFailed { exit_code, stderr }.into());
        }

        let sha256 = compute_sha256(binary)
            .map_err(|_| SignerError::TargetUnreadable(binary.to_path_buf()))?;

        Ok(SignedArtifact {
            path: binary.to_path_buf(),
            sha256,
            // Thumbprint signtool stderr/stdout'tan parse edilebilir, ancak
            // signtool'un standart output formatı release-to-release değişebilir.
            // Faz 8.x'te cert'ten ayrıştırma eklenecek (insan PR — PowerShell
            // `Get-ChildItem Cert:\CurrentUser\My` veya openssl ile).
            // v1: SHA-256 fingerprint placeholder; release notes'a signtool
            // stderr çıktısı düşülmeli.
            thumbprint: String::new(),
            timestamp_url: self.config.timestamp_url.clone(),
            hash_algorithm: self.config.hash_algorithm.clone(),
        })
    }
}

/// `signtool.exe` lokasyon çözümlemesi.
///
/// Öncelik sırası:
/// 1. `SignerConfig::signtool_path` (explicit override).
/// 2. Windows SDK default: `C:\Program Files (x86)\Windows Kits\10\bin\<sdk_ver>\x64\signtool.exe`
///    (registry `KitsRoot10`'dan root, sonra en yüksek sdk_ver).
/// 3. PATH'te `signtool` (Windows'ta `signtool.exe`, POSIX'te `signtool`).
///
/// # Errors
///
/// Hiçbir konumda bulunamazsa `SignerError::SigntoolMissing` döner.
pub fn locate_signtool(override_path: Option<&Path>) -> std::result::Result<PathBuf, SignerError> {
    if let Some(p) = override_path {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        return Err(SignerError::SigntoolMissing);
    }

    if let Some(p) = locate_windows_sdk_signtool() {
        return Ok(p);
    }

    // PATH fallback.
    let exe_name = if cfg!(windows) {
        "signtool.exe"
    } else {
        "signtool"
    };
    if let Ok(found) = which(exe_name) {
        return Ok(found);
    }

    Err(SignerError::SigntoolMissing)
}

#[cfg(windows)]
fn locate_windows_sdk_signtool() -> Option<PathBuf> {
    // Registry: HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots → KitsRoot10
    let root = read_kits_root()
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Windows Kits\10"));
    let bin = root.join("bin");
    let read_dir = std::fs::read_dir(&bin).ok()?;
    let mut versions: Vec<PathBuf> = read_dir
        .filter_map(std::io::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|s| s.starts_with("10."))
        })
        .collect();
    // En yüksek sürüm önce.
    versions.sort_by_key(|p| std::cmp::Reverse(p.file_name().map(|n| n.to_os_string())));
    for vdir in versions {
        for arch in ["x64", "x86", "arm64"] {
            let candidate = vdir.join(arch).join("signtool.exe");
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(windows)]
fn read_kits_root() -> Option<PathBuf> {
    use std::process::Command;
    // reg query "HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots" /v KitsRoot10
    let output = Command::new("reg")
        .args([
            "query",
            r"HKLM\SOFTWARE\Microsoft\Windows Kits\Installed Roots",
            "/v",
            "KitsRoot10",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains("KitsRoot10")
            && let Some(value) = line.split_whitespace().last()
        {
            return Some(PathBuf::from(value));
        }
    }
    None
}

#[cfg(not(windows))]
fn locate_windows_sdk_signtool() -> Option<PathBuf> {
    None
}

/// PATH araması (küçük `which` implementasyonu, `which` crate'i eklemeden).
fn which(exe: &str) -> std::result::Result<PathBuf, SignerError> {
    let path_var = std::env::var_os("PATH").ok_or(SignerError::SigntoolMissing)?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe);
        if candidate.is_file() {
            return Ok(candidate);
        }
        // Windows: PATHEXT ile `.exe` / `.cmd` / `.bat` eklemeleri.
        #[cfg(windows)]
        {
            if let Some(stem) = exe.strip_suffix(".exe") {
                for ext in [".exe", ".cmd", ".bat"] {
                    let c = dir.join(format!("{stem}{ext}"));
                    if c.is_file() {
                        return Ok(c);
                    }
                }
            }
        }
    }
    Err(SignerError::SigntoolMissing)
}

/// Dosyanın SHA-256'sını hesapla.
fn compute_sha256(path: &Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    Ok(hex_encode(&digest))
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_signed_default_has_safe_values() {
        let cfg = SignerConfig::self_signed_default();
        assert_eq!(cfg.hash_algorithm, "sha256");
        assert!(cfg.timestamp_url.starts_with("http"));
        assert!(!cfg.cert_password_env.is_empty());
        assert!(!cfg.description.is_empty());
        assert!(!cfg.publisher_url.is_empty());
    }

    #[test]
    fn default_timestamp_url_is_digicert() {
        assert_eq!(
            SignerConfig::default_timestamp_url(),
            "http://timestamp.digicert.com"
        );
    }

    #[test]
    fn config_accessor_returns_same_reference() {
        let signer = CodeSigner::self_signed();
        let cfg1 = signer.config();
        let cfg2 = signer.config();
        assert!(std::ptr::eq(cfg1, cfg2));
    }

    #[test]
    fn hex_encode_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let empty = Sha256::digest(b"");
        assert_eq!(
            hex_encode(&empty),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn hex_encode_handles_full_byte_range() {
        let input = [0u8, 0x0f, 0x10, 0xff];
        assert_eq!(hex_encode(&input), "000f10ff");
    }

    #[test]
    fn signed_artifact_fields_round_trip() {
        let artifact = SignedArtifact {
            path: PathBuf::from("target/release/viscos.exe"),
            sha256: "deadbeef".to_string(),
            thumbprint: String::new(),
            timestamp_url: SignerConfig::default_timestamp_url().to_string(),
            hash_algorithm: "sha256".to_string(),
        };
        let json = serde_json::to_string(&artifact).expect("serialize");
        let back: SignedArtifact = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, artifact);
    }

    #[test]
    fn signer_error_displays_path() {
        let err = SignerError::CertMissing(PathBuf::from("certs/missing.pfx"));
        let display = err.to_string();
        assert!(display.contains("certs/missing.pfx"));
    }

    #[test]
    fn signer_error_signtool_failed_includes_exit_and_stderr() {
        let err = SignerError::SigntoolFailed {
            exit_code: 1,
            stderr: "SignTool Error: bad password".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("1"));
        assert!(display.contains("bad password"));
    }

    #[test]
    fn locate_signtool_missing_override_errors() {
        // Var olmayan path → SigntoolMissing.
        let bogus = PathBuf::from("Z:/this/does/not/exist/signtool.exe");
        let result = locate_signtool(Some(&bogus));
        assert!(matches!(result, Err(SignerError::SigntoolMissing)));
    }

    #[test]
    fn locate_signtool_finds_one_in_windows_sdk_or_path() {
        // Bu test ortam-bağımsızdır: Windows'ta SDK veya PATH'te bir tane bulunmalı.
        // Eğer hiçbir yerde yoksa skip.
        match locate_signtool(None) {
            Ok(p) => assert!(p.exists(), "located signtool must exist on disk"),
            Err(SignerError::SigntoolMissing) => {
                // Test makinesinde signtool yok — skip. CI'da Windows runner
                // varsa PATH'te veya SDK'ta bulunmalı.
                eprintln!("signtool not present in test env — skipped");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn which_finds_cmd_on_windows_or_skips() {
        // `cmd.exe` Windows'ta her zaman PATH'te; POSIX'te `sh`.
        let target = if cfg!(windows) { "cmd.exe" } else { "sh" };
        match which(target) {
            Ok(p) => assert!(p.exists()),
            Err(SignerError::SigntoolMissing) => {
                eprintln!("`{target}` not in PATH — skipped");
            }
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn password_zeroizing_wrapper_drops_cleanly() {
        use zeroize::Zeroize;
        let mut password = Zeroizing::new(String::from("super-secret-pw"));
        assert_eq!(password.as_str(), "super-secret-pw");
        password.zeroize();
        // zeroize sonrası heap buffer sıfırlanır; uzunluk değişmez ama
        // içerik boşalmış olmalı. Zeroizing deref'i String üzerinden
        // yaptığı için burada Zeroizing'in inner String'ini kontrol edemeyiz;
        // ancak zeroize() çağrısı panik atmamalı.
        // İkinci bir zeroize no-op olmalı (idempotent).
        password.zeroize();
    }
}
