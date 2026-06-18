//! Drag & drop dosya paylaşımı (Faz 6.0).
//!
//! Faz 6.0'da `tao::Window` üzerinde `WindowEvent::DroppedFile(path)` hook'u
//! ile gelen dosya yolları handle edilir. Gerçek Discord upload Faz 5.x'te
//! `viscos-media::MediaUploader` ile entegre olunca.
//!
//! Cross-references:
//! - [`phase-6.0-hotkeys.md` §3 Drag & Drop](../../../.cursor/plans/phase-6.0-hotkeys.md)

use std::path::Path;
use viscos_error::ViscosError;

/// Drag & drop edilen dosyayı handle et (stub).
///
/// Faz 6.0'da:
/// 1. Path'in `is_file()` kontrolü
/// 2. Mevcut kanal ID'si ile `MediaUploader::upload(path, channel_id)` çağrısı
///
/// Faz 5.0'da sadece log + OK stub. Gerçek upload Faz 5.x'te.
///
/// # Errors
///
/// Path mevcut değilse veya okuma izni yoksa `ViscosError::Io` döner.
pub fn handle_drop(path: &Path) -> Result<(), ViscosError> {
    if !path.exists() {
        return Err(ViscosError::Media(format!(
            "dropped path does not exist: {}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(ViscosError::Media(format!(
            "dropped path is not a file: {}",
            path.display()
        )));
    }
    tracing::info!(
        path = %path.display(),
        "drag&drop received (Faz 5.0 stub — would upload via MediaUploader)"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn handle_drop_nonexistent_path_errors() {
        let result = handle_drop(&PathBuf::from("C:/totally/not/a/real/path.xyz"));
        assert!(result.is_err());
    }

    #[test]
    fn handle_drop_directory_errors() {
        // Temp dizin oluştur.
        let tmp = tempfile::tempdir().expect("tempdir");
        let result = handle_drop(tmp.path());
        assert!(result.is_err(), "directory drop should error");
    }

    #[test]
    fn handle_drop_valid_file_succeeds() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let file = tmp.path().join("test.txt");
        std::fs::write(&file, b"hello").expect("write");
        let result = handle_drop(&file);
        assert!(result.is_ok(), "valid file drop should succeed: {result:?}");
    }
}
