//! Windows audio controller — WASAPI mute/deafen bindings (MVP-3 scaffold).
//!
//! Faz 7.0 (v1 minimal): Discord-style mute + deafen toggle. Ctrl+Shift+M
//! fires `toggle_mute` (mic only); Ctrl+Shift+D fires `toggle_deafen` (both
//! mic + speaker muted). All toggles are reversible — calling twice restores
//! the original state.
//!
//! ## Platform support
//!
//! - **Windows (MVP-3 real):** `IAudioEndpointVolume::SetMute` via the
//!   `windows` crate's `Win32_Media_Audio` feature. Default render + capture
//!   endpoints are resolved through `IMMDeviceEnumerator`.
//! - **Other OS (MVP-3 stub):** construction succeeds as an inert handle;
//!   every operation returns `ViscosError::Unimplemented("WASAPI Windows-only")`.
//!   This is by design — Linux/macOS audio routing is out of v1 scope per
//!   `COMPREHENSIVE-AUDIT-STUBS-AND-TODOS-2026-06-19.md` §5.1.
//!
//! ## Safety
//!
//! The Windows `IAudioEndpointVolume` COM pointer is a raw `*mut c_void` and
//! all access is wrapped in `unsafe` blocks with explicit `SAFETY` comments.
//! Each FFI call goes through a fresh `CoCreateInstance` /
//! `IMMDeviceEnumerator::GetDefaultAudioEndpoint` chain so no COM pointer
//! escapes the calling function. The cached mute state uses interior
//! mutability (`parking_lot::Mutex`) so `toggle_mute(&self)` can update the
//! cache without requiring `&mut self`. `Send + Sync` are derived manually
//! only when `Mutex` already provides them; the Windows build's cache is
//! `Send + Sync` because `parking_lot::Mutex<bool>` is.
//!
//! Cross-references:
//! - [`phase-7.0-voice-video.md` §3 Audio Routing](../../../.cursor/plans/phase-7.0-voice-video.md)
//! - Audit §2.8, §5.1.

use parking_lot::Mutex;
use viscos_error::ViscosError;

/// Default audio controller handle (MVP-3 scaffold).
///
/// `AudioController` is the single entry point for WASAPI mute / deafen
/// operations on Windows and a no-op on other platforms. The struct holds
/// no platform-specific resources on non-Windows builds so the same API
/// surface compiles unchanged across targets.
#[derive(Debug)]
pub struct AudioController {
    /// Cached mute state (interior-mutable so `&self` methods can update it).
    state: Mutex<AudioState>,
}

#[derive(Debug, Clone, Copy, Default)]
struct AudioState {
    /// Last-known mic mute state (Windows: mirrors `IAudioEndpointVolume`).
    mic_muted: bool,
    /// Last-known speaker mute state.
    speaker_muted: bool,
}

impl Default for AudioController {
    fn default() -> Self {
        Self {
            state: Mutex::new(AudioState::default()),
        }
    }
}

impl AudioController {
    /// Construct a new audio controller.
    ///
    /// On Windows this initializes COM's `MMDeviceEnumerator` and resolves the
    /// default render + capture endpoints. On other platforms the call is a
    /// no-op and returns the inert controller.
    ///
    /// # Errors
    ///
    /// - `ViscosError::Unimplemented` — non-Windows platform (by design, MVP-3
    ///   does not yet implement PulseAudio/CoreAudio backends).
    /// - `ViscosError::Io` — Windows COM initialization failed (very rare;
    ///   usually indicates a broken Windows install).
    #[cfg(target_os = "windows")]
    pub fn new() -> Result<Self, ViscosError> {
        // MVP-3: we deliberately do *not* hold raw COM pointers across method
        // calls. Each WASAPI call goes through a fresh `CoCreateInstance` /
        // `GetDefaultAudioEndpoint` chain. This keeps the struct `Send + Sync`
        // without extra unsafe, and makes the destructor a no-op.
        tracing::debug!("AudioController::new (Windows WASAPI scaffold initialized)");
        Ok(Self::default())
    }

    /// Construct a new audio controller (non-Windows placeholder).
    ///
    /// # Errors
    ///
    /// Always succeeds on non-Windows; operations on the returned controller
    /// will return `ViscosError::Unimplemented("WASAPI Windows-only")`.
    #[cfg(not(target_os = "windows"))]
    pub fn new() -> Result<Self, ViscosError> {
        Ok(Self::default())
    }

    /// Toggle microphone mute and return the new mute state.
    ///
    /// # Errors
    ///
    /// - `ViscosError::Unimplemented("WASAPI Windows-only")` on non-Windows.
    /// - `ViscosError::Io` on Windows COM failure.
    #[cfg(target_os = "windows")]
    pub fn toggle_mute(&self) -> Result<bool, ViscosError> {
        let next = {
            let mut state = self.state.lock();
            state.mic_muted = !state.mic_muted;
            state.mic_muted
        };
        apply_endpoint_mute(EndpointKind::Capture, next)?;
        tracing::info!(target: "viscos.audio", mic_muted = next, "mic mute toggled");
        Ok(next)
    }

    /// Toggle microphone mute (non-Windows placeholder).
    ///
    /// # Errors
    ///
    /// Always `ViscosError::Unimplemented("WASAPI Windows-only")`.
    #[cfg(not(target_os = "windows"))]
    pub fn toggle_mute(&self) -> Result<bool, ViscosError> {
        Err(ViscosError::Unimplemented("WASAPI Windows-only"))
    }

    /// Toggle deafen: mutes (or unmutes) both mic and speaker together.
    ///
    /// Discord convention: deafen implies mic mute, but the reverse is not
    /// required. We mirror that semantic so `toggle_deafen` is idempotent —
    /// if both endpoints are already in the requested state we still flip
    /// both consistently.
    ///
    /// # Errors
    ///
    /// - `ViscosError::Unimplemented("WASAPI Windows-only")` on non-Windows.
    /// - `ViscosError::Io` on Windows COM failure.
    pub fn toggle_deafen(&self) -> Result<(), ViscosError> {
        #[cfg(target_os = "windows")]
        {
            // Deafen drives speaker state; mic follows.
            let next = {
                let mut state = self.state.lock();
                state.speaker_muted = !state.speaker_muted;
                state.mic_muted = state.speaker_muted;
                state.speaker_muted
            };
            apply_endpoint_mute(EndpointKind::Render, next)?;
            apply_endpoint_mute(EndpointKind::Capture, next)?;
            tracing::info!(
                target: "viscos.audio",
                speaker_muted = next,
                mic_muted = next,
                "deafen toggled"
            );
            Ok(())
        }
        #[cfg(not(target_os = "windows"))]
        {
            Err(ViscosError::Unimplemented("WASAPI Windows-only"))
        }
    }

    /// True iff the underlying platform supports WASAPI (Windows only).
    #[must_use]
    pub const fn is_supported() -> bool {
        cfg!(target_os = "windows")
    }

    /// Last-known mic mute state (MVP-3 scaffold; Windows only).
    ///
    /// On non-Windows this always returns `false` because the controller
    /// never touches the real audio endpoints.
    #[must_use]
    pub fn mic_muted(&self) -> bool {
        self.state.lock().mic_muted
    }

    /// Last-known speaker mute state (MVP-3 scaffold; Windows only).
    #[must_use]
    pub fn speaker_muted(&self) -> bool {
        self.state.lock().speaker_muted
    }
}

/// Which endpoint family to operate on (MVP-3 scaffold).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndpointKind {
    /// Speaker / render endpoint.
    Render,
    /// Microphone / capture endpoint.
    Capture,
}

/// Apply mute to a single endpoint via WASAPI.
///
/// MVP-3 scaffold: this is wired to the real `IAudioEndpointVolume::SetMute`
/// COM call when the `Win32_Media_Audio` feature is available. The COM
/// pointer is locally created and dropped inside the function — no state
/// escapes, so the surrounding `unsafe` block is bounded by the function
/// scope. Each call resolves the *current* default endpoint so device
/// hot-swap works without re-creating `AudioController`.
///
/// # Errors
///
/// `ViscosError::Io` on COM failure (propagated from the underlying FFI).
#[cfg(target_os = "windows")]
fn apply_endpoint_mute(kind: EndpointKind, mute: bool) -> Result<(), ViscosError> {
    use std::ptr;
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole, eRender,
    };
    use windows::Win32::System::Com::{
        CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
    };

    // SAFETY: `CoInitializeEx(nullptr, COINIT_MULTITHREADED)` is documented as
    // safe to call multiple times within a process as long as we balance with
    // `CoUninitialize`. We intentionally leak the COM init here — the audio
    // controller is process-scoped and the OS cleans up at process exit. The
    // alternative (thread-local init / uninit pairs) would require a `Drop`
    // impl that runs at unpredictable points and risks use-after-free on
    // background worker threads.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }

    // SAFETY: `CoCreateInstance` returns a valid COM pointer on success. We
    // own the returned reference for the duration of this function and call
    // `Release` on the way out (implicit via the temporary `Drop` of the
    // `IMMDeviceEnumerator` smart pointer). The CLSID is well-known and the
    // apartment is initialized above.
    let enumerator: IMMDeviceEnumerator = unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| {
            ViscosError::Io(std::io::Error::other(format!(
                "CoCreateInstance(MMDeviceEnumerator) failed: {e}"
            )))
        })?
    };

    // In windows 0.58 the data-flow / role constants are module-level
    // constants (not associated items on the `EDataFlow` / `ERole` newtype
    // structs). The match below is exhaustive but folds to a constant at
    // compile time so codegen is fine.
    let (data_flow, role) = match kind {
        EndpointKind::Render => (eRender, eConsole),
        EndpointKind::Capture => (eCapture, eConsole),
    };

    // SAFETY: `GetDefaultAudioEndpoint` fills the out-parameter with a valid
    // `IMMDevice` pointer on success. We immediately wrap it in a smart
    // pointer and never access the raw pointer outside this scope. `role` is
    // a valid ERole enum value and `data_flow` is a valid EDataFlow value.
    let device = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(data_flow, role)
            .map_err(|e| {
                ViscosError::Io(std::io::Error::other(format!(
                    "GetDefaultAudioEndpoint failed: {e}"
                )))
            })?
    };

    // SAFETY: `Activate<T>` returns an `IAudioEndpointVolume` after performing
    // the standard IUnknown::QueryInterface dance. The IID is derived from
    // the requested type parameter; the out-pointer is owned by us for the
    // rest of the function and dropped at scope exit. `pactivationparams`
    // is `None` because we do not pass activation parameters.
    let endpoint: windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume = unsafe {
        device.Activate(CLSCTX_ALL, None).map_err(|e| {
            ViscosError::Io(std::io::Error::other(format!(
                "IMMDevice::Activate(IAudioEndpointVolume) failed: {e}"
            )))
        })?
    };

    // SAFETY: `SetMute` writes the BOOL mute flag through the
    // `IAudioEndpointVolume::SetMute` vtable entry. The endpoint pointer is
    // valid for the duration of the call. The event-context pointer is
    // `nullptr` because we do not subscribe to mute-change notifications.
    // `BOOL` is a 4-byte signed newtype around `i32`; `1` is truthy and `0`
    // is falsy per Win32 convention.
    unsafe {
        endpoint
            .SetMute(BOOL(mute as i32), ptr::null())
            .map_err(|e| {
                ViscosError::Io(std::io::Error::other(format!(
                    "IAudioEndpointVolume::SetMute failed: {e}"
                )))
            })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_succeeds_on_current_platform() {
        // Construction must succeed everywhere — the controller itself is
        // platform-agnostic; only individual operations may return Unimplemented.
        let ctrl = AudioController::new().expect("controller must construct");
        if cfg!(target_os = "windows") {
            assert!(AudioController::is_supported());
        } else {
            assert!(!AudioController::is_supported());
        }
        // Cached state defaults to unmuted.
        assert!(!ctrl.mic_muted());
        assert!(!ctrl.speaker_muted());
    }

    #[test]
    fn toggle_mute_returns_unimplemented_off_windows() {
        if !cfg!(target_os = "windows") {
            let ctrl = AudioController::new().expect("construct");
            let result = ctrl.toggle_mute();
            match result {
                Err(ViscosError::Unimplemented(msg)) => {
                    assert_eq!(msg, "WASAPI Windows-only");
                }
                other => panic!("expected Unimplemented, got {other:?}"),
            }
        }
    }

    #[test]
    fn toggle_deafen_returns_unimplemented_off_windows() {
        if !cfg!(target_os = "windows") {
            let ctrl = AudioController::new().expect("construct");
            let result = ctrl.toggle_deafen();
            match result {
                Err(ViscosError::Unimplemented(msg)) => {
                    assert_eq!(msg, "WASAPI Windows-only");
                }
                other => panic!("expected Unimplemented, got {other:?}"),
            }
        }
    }

    #[test]
    fn is_supported_reflects_target_os() {
        // Compile-time branch: this is the canonical truth for the runtime check.
        assert_eq!(AudioController::is_supported(), cfg!(target_os = "windows"));
    }

    #[test]
    fn toggle_mute_does_not_update_cache_off_windows() {
        if !cfg!(target_os = "windows") {
            let ctrl = AudioController::new().expect("construct");
            // Stub controller never updates cached state because the call
            // errors before reaching the (real) SetMute step.
            let _ = ctrl.toggle_mute();
            assert!(!ctrl.mic_muted());
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[allow(clippy::collapsible_if)] // sequential checks: audio present → skip vs panic
    fn toggle_mute_windows_inverts_cached_state() {
        let ctrl = AudioController::new().expect("construct");
        // First toggle: WASAPI round-trip — if a Windows audio endpoint is
        // present the call will mutate real audio. We tolerate either outcome
        // but the cached state must be consistent with the returned bool.
        //
        // CI runners (Windows Server, no audio service) frequently lack a
        // default audio endpoint. `GetDefaultAudioEndpoint` returns
        // `Element not found (0x80070490)` in that case — skip the test
        // instead of panicking so CI stays green for non-audio changes.
        let first = ctrl.toggle_mute();
        if let Err(e) = &first {
            if e.to_string().contains("GetDefaultAudioEndpoint failed") {
                eprintln!("skipping: no audio endpoint in this environment ({e})");
                return;
            }
        }
        let new_state = first.expect("toggle");
        assert_eq!(ctrl.mic_muted(), new_state);
        // Toggle back.
        let new_state2 = ctrl.toggle_mute().expect("toggle back");
        assert_eq!(ctrl.mic_muted(), new_state2);
        assert_ne!(new_state, new_state2);
    }
}
