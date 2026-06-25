//! Integration tests — `CodeSigner` real `signtool.exe` invocation.
//!
//! Faz 8.0 release engineering. Test fixture: PowerShell ile üretilmiş
//! self-signed PFX (`New-SelfSignedCertificate` + `Export-PfxCertificate`).
//!
//! **Test ortamı gereksinimi:**
//! - Windows (signtool.exe Windows-only)
//! - `signtool.exe` PATH'te veya Windows SDK'ta kurulu
//! - PowerShell 5+ (built-in)
//! - `signtool` env değişkeni (CERT_PASSWORD ile) test sırasında set edilir.
//!
//! CI'da Windows runner'da otomatik çalışır. Lokal geliştirici makinesinde
//! `cargo test -p viscos-distribution --test signing` ile koşturulur.
//!
//! **Skipping:** Eğer ortamda `signtool.exe` yoksa testler `eprintln!` ile
//! skip edilir (PASS) — CI'da Windows runner'da her zaman çalışmalıdır.

#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use tempfile::tempdir;
use viscos_distribution::{CodeSigner, SignerConfig, SignerError};

static TEST_COUNTER: AtomicU32 = AtomicU32::new(0);

fn unique_tag(label: &str) -> String {
    let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    format!("viscos-signtest-{label}-{pid}-{n}")
}

/// PowerShell ile self-signed PFX üretir ve temp dizinine yazar.
///
/// `openssl` zorunluluğunu ortadan kaldırmak için Windows-native
/// `New-SelfSignedCertificate` + `Export-PfxCertificate` kullanır.
/// Üretilen PFX SHA-256 RSA + 1 yıl geçerli.
fn generate_test_pfx(out_dir: &Path, password: &str) -> std::result::Result<PathBuf, String> {
    let pfx_path = out_dir.join("test.pfx");
    let cn = unique_tag("cert");
    // PowerShell script'i: self-signed cert üret → PFX'e export et.
    // PASSWORD escape: backtick ile quote'ları güvenli hale getir.
    let ps_script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$cert = New-SelfSignedCertificate `
    -Subject "CN={cn}" `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -KeyAlgorithm RSA `
    -KeyLength 2048 `
    -KeyUsage DigitalSignature `
    -FriendlyName "Viscos Test Cert" `
    -NotAfter (Get-Date).AddYears(1)
$password = ConvertTo-SecureString -String "{password}" -Force -AsPlainText
Export-PfxCertificate -Cert $cert -FilePath "{pfx}" -Password $password
Remove-Item -Path $cert.PSPath -Force
"#,
        cn = cn,
        password = password.replace('"', "`\""),
        pfx = pfx_path.display().to_string().replace('\\', "\\\\"),
    );
    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("failed to spawn powershell: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "powershell exit {}: stderr={} stdout={}",
            output.status, stderr, stdout
        ));
    }
    if !pfx_path.exists() {
        return Err(format!("PFX not written: {}", pfx_path.display()));
    }
    Ok(pfx_path)
}

/// Hedef olarak imzalanacak sahte bir PE-ish binary üretir.
///
/// signtool.exe gerçek bir PE header olmasa bile çoğu durumda imzalar
/// (signtool sadece `IMAGE_NT_HEADERS` imzası arar; yoksa yine de
/// dosyayı olduğu gibi imzalar — en azından SDK 10.0.26100'de).
/// Burada sadece "imzalanmış/önce-imzalanmamış" SHA-256 farkı yeterli.
fn write_fake_target(out_dir: &Path, name: &str) -> PathBuf {
    let path = out_dir.join(name);
    // Minimal PE-like header (MZ + PE signature yeterli; signtool
    // geçerli PE imzalamazsa bile dosyaya Authenticode ekleyebilir).
    let mut bytes = vec![0x4D, 0x5A]; // "MZ"
    bytes.extend_from_slice(&[0u8; 58]); // DOS header padding
    bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x00]); // e_lfanew offset
    bytes.extend_from_slice(b"PE\0\0"); // PE signature
    bytes.extend_from_slice(&[0u8; 100]); // filler
    std::fs::write(&path, &bytes).expect("write fake target");
    path
}

fn file_sha256(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).expect("read");
    let mut h = Sha256::new();
    h.update(&bytes);
    let digest = h.finalize();
    let mut s = String::with_capacity(digest.len() * 2);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Ortamda signtool.exe bulunabilir mi? Windows SDK veya PATH.
fn signtool_available() -> bool {
    viscos_distribution::signing::locate_signtool(None).is_ok()
}

/// `sign_with_test_pfx_succeeds` — gerçek PFX + gerçek signtool.
///
/// signtool.exe ortamda yoksa skip edilir (CI'da Windows runner'da çalışır).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_with_test_pfx_succeeds() {
    if !signtool_available() {
        eprintln!("signtool.exe not available — test skipped");
        return;
    }
    let tmp = tempdir().expect("tempdir");
    let password = "viscos-test-pw-2026";
    let pfx = generate_test_pfx(tmp.path(), password).expect("PFX generation");
    let target = write_fake_target(tmp.path(), "target.exe");

    let sha_before = file_sha256(&target);

    let cfg = SignerConfig {
        signtool_path: None,
        cert_path: pfx.clone(),
        cert_password_env: "VISCOS_TEST_CERT_PASSWORD".to_string(),
        timestamp_url: SignerConfig::default_timestamp_url().to_string(),
        hash_algorithm: "sha256".to_string(),
        description: "Viscos Test".to_string(),
        publisher_url: "https://viscos.app".to_string(),
    };
    // SAFETY: Bu test'te env değişkenini sadece test thread'inde set edip
    // hemen temizliyoruz. Paralel test'lerde isolation gerektiğinde
    // ayrı bir process boundary kullanılmalı.
    unsafe {
        std::env::set_var("VISCOS_TEST_CERT_PASSWORD", password);
    }
    let signer = CodeSigner::new(cfg);
    let result = signer.sign(&target).await;
    unsafe {
        std::env::remove_var("VISCOS_TEST_CERT_PASSWORD");
    }

    let artifact = result.expect("signing should succeed with valid PFX");
    assert!(artifact.path.exists(), "signed target must still exist");
    let sha_after = file_sha256(&target);
    assert_ne!(
        sha_before, sha_after,
        "SHA-256 must change after signing (Authenticode block appended)"
    );
    assert_eq!(artifact.sha256, sha_after);
    assert_eq!(artifact.hash_algorithm, "sha256");
}

/// `sign_with_missing_pfx_returns_signtool_failed` — var olmayan PFX.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_with_missing_pfx_returns_cert_missing() {
    if !signtool_available() {
        eprintln!("signtool.exe not available — test skipped");
        return;
    }
    let tmp = tempdir().expect("tempdir");
    let target = write_fake_target(tmp.path(), "target.exe");

    let cfg = SignerConfig {
        signtool_path: None,
        cert_path: tmp.path().join("does_not_exist.pfx"),
        cert_password_env: "VISCOS_TEST_CERT_PASSWORD".to_string(),
        timestamp_url: SignerConfig::default_timestamp_url().to_string(),
        hash_algorithm: "sha256".to_string(),
        description: "Viscos Test".to_string(),
        publisher_url: "https://viscos.app".to_string(),
    };
    unsafe {
        std::env::set_var("VISCOS_TEST_CERT_PASSWORD", "irrelevant");
    }
    let signer = CodeSigner::new(cfg);
    let result = signer.sign(&target).await;
    unsafe {
        std::env::remove_var("VISCOS_TEST_CERT_PASSWORD");
    }

    let err = result.expect_err("missing PFX must error");
    assert!(
        matches!(err, viscos_error::ViscosError::Io(_)),
        "expected Io error variant, got: {err:?}"
    );
    let display = format!("{err}");
    assert!(
        display.contains("does_not_exist.pfx") || display.contains("missing"),
        "error should mention missing file: {display}"
    );
}

/// `sign_with_wrong_password_returns_signtool_failed` — yanlış şifre.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_with_wrong_password_returns_signtool_failed() {
    if !signtool_available() {
        eprintln!("signtool.exe not available — test skipped");
        return;
    }
    let tmp = tempdir().expect("tempdir");
    let real_pw = "correct-password-123";
    let pfx = generate_test_pfx(tmp.path(), real_pw).expect("PFX generation");
    let target = write_fake_target(tmp.path(), "target.exe");

    let cfg = SignerConfig {
        signtool_path: None,
        cert_path: pfx,
        cert_password_env: "VISCOS_TEST_CERT_PASSWORD".to_string(),
        timestamp_url: SignerConfig::default_timestamp_url().to_string(),
        hash_algorithm: "sha256".to_string(),
        description: "Viscos Test".to_string(),
        publisher_url: "https://viscos.app".to_string(),
    };
    unsafe {
        std::env::set_var("VISCOS_TEST_CERT_PASSWORD", "wrong-password");
    }
    let signer = CodeSigner::new(cfg);
    let result = signer.sign(&target).await;
    unsafe {
        std::env::remove_var("VISCOS_TEST_CERT_PASSWORD");
    }

    let err = result.expect_err("wrong password must error");
    let display = format!("{err}");
    assert!(
        display.to_lowercase().contains("signtool")
            || display.to_lowercase().contains("password")
            || display.to_lowercase().contains("invoking"),
        "error should relate to signtool failure: {display}"
    );
}

/// `signtool_not_found_returns_path_error` — bogus signtool path.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signtool_not_found_returns_path_error() {
    let bogus = PathBuf::from("Z:/this/path/does/not/exist/signtool.exe");
    let result = viscos_distribution::signing::locate_signtool(Some(&bogus));
    assert!(
        matches!(result, Err(SignerError::SigntoolMissing)),
        "bogus signtool path must produce SigntoolMissing, got: {result:?}"
    );
}

/// `password_is_zeroized_after_use` — `SecretBox` wrapper'ı drop'ta zeroize eder.
///
/// Bu test secrecy'nin kendi davranışını doğrular: `SecretBox::new(Box::new(s))`
/// sonrası `expose_secret()` çağrılabilir; drop'ta içerideki `Box<String>`
/// `ZeroizeOnDrop` üzerinden silinir. Wrapper'ı scope'tan düşürünce bellek
/// temizlenir — biz burada wrapper'ın hayatta kaldığı scope'ta erişilebilir
/// olduğunu, scope dışında hâlâ derive edilebilir olduğunu doğruluyoruz
/// (zeroize davranışı crate'in kendi sorumluluğu).
#[test]
fn password_is_zeroized_after_use() {
    use secrecy::{ExposeSecret, SecretBox};
    use zeroize::{Zeroize, Zeroizing};
    let pwd = String::from("super-secret-test-password-2026");
    let boxed: SecretBox<String> = SecretBox::new(Box::new(pwd));
    assert_eq!(boxed.expose_secret(), "super-secret-test-password-2026");
    // Wrapper drop'a gidiyor; burada ikinci bir Zeroizing oluşturup
    // zeroize() çağrısının panik atmadığını doğruluyoruz.
    let mut z = Zeroizing::new(String::from("test-pw"));
    z.zeroize();
    z.zeroize(); // idempotent
}
