---
name: Phase 7.0 — Voice/Video (Opsiyonel, v1'de ATLANIR)
overview: DAVE E2EE voice protocol, webrtc-rs, libdave, libopus, libsodium entegrasyonu. WebView2 zaten DAVE yapıyor; v1 için atlanması önerilir. Kullanıcı feedback'ine göre v1.5/v2'de eklenebilir.
isProject: false
todos:
  - id: voice-deprioritize
    content: KARAR: v1'de voice atlanır, gerçek zamanlı feedback beklenir
    status: pending
  - id: audio-control
    content: Temel audio control (mic mute/deafen) — global hotkey destek için
    status: pending
  - id: webrtc-rs-deps
    content: webrtc-rs, libdave, libopus, libsodium Cargo deps (Faz 7'de aktif olursa)
    status: pending
  - id: dave-protocol
    content: DAVE (Discord Audio & Video End-to-end encryption) implementasyonu
    status: pending
  - id: voice-state-events
    content: Voice state event handling (VOICE_STATE_UPDATE, VOICE_SERVER_UPDATE)
    status: pending
  - id: opus-encoding
    content: Opus encode/decode
    status: pending
  - id: udp-transport
    content: UDP transport + RTP packet handling
    status: pending
  - id: e2ee-encrypt
    content: E2EE: AES-GCM ile media encryption
    status: pending
  - id: screen-share
    content: Screen share (opsiyonel, daha sonra)
    status: pending
---

# Phase 7.0 — Voice/Video (Opsiyonel, v1'de ATLANIR)

> **Süre:** 3+ hafta (eğer aktif edilirse)
> **Hedef:** DAVE E2EE voice + screen share (native).
> **KRİTİK:** Bu faz **v1'de atlanması önerilir.** Aşağıdaki "Neden Atla" bölümü.
> **Önceki faz:** [`phase-6.0-hotkeys.md`](./phase-6.0-hotkeys.md)
> **Sonraki faz:** [`phase-8.0-distribution.md`](./phase-8.0-distribution.md)

---

## 1. Neden Atla? (v1 İçin)

WebView2 içindeki Discord zaten DAVE E2EE, voice, video, screen share, libdave, Opus codec hepsini kendisi yapıyor. **Discord'un kendisi bile WebRTC kullanıyor.**

kind (C++/Qt) yıllardır voice (DAVE), animated emoji, sticker, forum, threads hepsini sıfırdan yazmaya çalışıyor.
Acheron (C++/Qt) libdave entegre etmiş ama animated emoji hâlâ Yapılacaklar'da.

**Viscos hibrit yaklaşımının en güçlü gerekçesi bu:** Native voice yazmak **aylarca** sürer, hibrit ise Discord'un kendi WebView2 implementasyonunu kullanır.

**v1 önerisi:**
- v1 release: voice yok, kullanıcı Discord web üzerinden voice kullanır (WebView2'de)
- v1.5: Kullanıcı feedback'i topla, eğer ciddi talep varsa native voice başla
- v2: Multi-account, native voice, full featured

**Bu fazda sadece TEMEL audio control (mic mute/deafen) yapılır** — global hotkey'ler (Ctrl+Shift+M) için gerekli.

---

## 2. Sadece Audio Control (Faz 7'nin v1 kapsamı)

```rust
// crates/viscos-audio/src/lib.rs
// Windows: WASAPI

use windows::Win32::Media::Audio::*;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy)]
pub struct AudioState {
    pub muted: bool,
    pub deafened: bool,
}

impl Default for AudioState {
    fn default() -> Self {
        Self { muted: false, deafened: false }
    }
}

pub struct AudioControl {
    state: Arc<RwLock<AudioState>>,
    // Windows COM audio session
    session: Arc<tokio::sync::Mutex<Option<IAudioSessionControl2>>>,
}

impl AudioControl {
    pub fn new() -> anyhow::Result<Self> {
        // COM initialize, WASAPI session
        Ok(Self {
            state: Arc::new(RwLock::new(AudioState::default())),
            session: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }
    
    pub async fn toggle_mute(&self) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.muted = !state.muted;
        // Discord WebView2'ye bildir (native → web push, küçük event)
        tracing::info!(muted = state.muted, "Mic toggled");
        Ok(())
    }
    
    pub async fn toggle_deafen(&self) -> anyhow::Result<()> {
        let mut state = self.state.write().await;
        state.deafened = !state.deafened;
        if state.deafened {
            state.muted = true;  // Deafen = mute
        }
        tracing::info!(deafened = state.deafened, "Deafen toggled");
        Ok(())
    }
    
    pub async fn get_state(&self) -> AudioState {
        *self.state.read().await
    }
}
```

**Push to web (küçük event):**
```rust
// WebView2'ye audio state değişikliğini bildir
self.ipc.emit(IpcEvent::TrayBadgeUpdate { count: ... });
// Veya
self.webview.eval_script(&format!(
    "window.dispatchEvent(new CustomEvent('viscos-audio-state', {{ detail: {} }}))",
    serde_json::to_string(&state)?
));
```

Discord WebView2 native hotkey'i alır, kendi mute state'ini günceller (kendi ses sistemini kullanır).

---

## 3. Eğer Aktif Edilirse (v1.5+)

### 3.1 Cargo Dependencies

```toml
[dependencies]
webrtc-rs = "0.10"
libdave = { git = "https://github.com/discord/libdave", features = ["rust-bindings"] }
opus = "0.3"
libsodium-sys = "0.2"
sodiumoxide = "0.2"
```

### 3.2 DAVE Protocol

Discord Audio & Video End-to-end encryption (DAVE):
- Her session için ECDH key exchange
- AES-GCM ile frame-level encryption
- Multiplexed RTP üzerinden

**Referans:** https://daveprotocol.com/

### 3.3 Voice State Event Flow

```
Discord Gateway
    ↓ VOICE_STATE_UPDATE { channel_id, session_id, user_id }
    ↓ VOICE_SERVER_UPDATE { token, endpoint }
    ↓
Client: GET /voice/connect (TLS handshake, ECDH)
    ↓
Discord Voice Server
    ↓ UDP transport (RTP)
    ↓
Opus encode + DAVE encrypt
    ↓
UDP packet send
```

### 3.4 Voice Connection Lifecycle

```rust
// Pseudo-code (v1.5'te implement)
pub struct VoiceConnection {
    session_id: String,
    channel_id: String,
    udp_socket: UdpSocket,
    opus_encoder: opus::Encoder,
    dave_session: libdave::Session,
    aes_key: [u8; 32],
}

impl VoiceConnection {
    pub async fn connect(voice_state: VoiceStateUpdate, voice_server: VoiceServerUpdate) -> Result<Self> {
        // 1. UDP discovery packet
        // 2. ECDH key exchange
        // 3. Session setup
        // 4. Ready to send
    }
    
    pub async fn send_audio(&self, pcm: &[i16]) -> Result<()> {
        // 1. Opus encode
        let opus_frame = self.opus_encoder.encode(pcm, 960)?;
        // 2. DAVE encrypt
        let encrypted = self.dave_session.encrypt(&opus_frame)?;
        // 3. RTP packet
        let rtp = RtpPacket::new(...);
        // 4. UDP send
        self.udp_socket.send(&rtp.bytes()).await?;
        Ok(())
    }
}
```

### 3.5 Screen Share (Daha İleride)

Screen share için:
- Windows: Desktop Duplication API (`dxgi`)
- Encode: libvpx (VP8/VP9) veya H.264 (libx264)
- Send: aynı DAVE transport

**v1'de yok, v2'de düşünülür.**

---

## 4. Test Stratejisi (Eğer Aktif Edilirse)

| Test | Tip | Kabul |
|------|-----|-------|
| Audio toggle (Faz 7 v1) | Unit | State doğru, log çıkıyor |
| Opus encode/decode | Unit | Roundtrip |
| DAVE encrypt/decrypt | Unit | Roundtrip, AES-GCM |
| UDP packet send | Integration | Local echo server |
| Voice connect | E2E (lokal) | Gerçek Discord ses kanalına bağlanma |
| E2EE end-to-end | Integration | Wireshark capture, payload encrypted |

---

## 5. Kabul Kriterleri (v1 kapsamı)

- [ ] AudioControl toggle çalışıyor
- [ ] Global hotkey (Ctrl+Shift+M) toggle tetikliyor
- [ ] State WebView2'ye push ediliyor
- [ ] Discord kendi ses sistemini WebView2'de güncelliyor
- [ ] `cargo clippy -- -D warnings` temiz

**v1.5/v2 (eğer aktif):**
- [ ] Native voice (DAVE E2EE)
- [ ] Opus encode/decode
- [ ] UDP transport
- [ ] Screen share (opsiyonel)

---

## 6. Karar Noktası (Faz 7 Sonu)

> 🔵 **İNSAN:** v1'de voice/video olsun mu?
> - **HAYIR (önerilen):** v1 release, voice yok, kullanıcı feedback'ini bekle
> - **EVET:** v1'de native voice, 3+ hafta ek geliştirme
> - Trade-off: feature parity vs release hızı

> 🔵 **İNSAN (eğer EVET):** Screen share dahil mi?
> - Voice only (sesli kanal)
> - Voice + screen share (full feature)
> - Trade-off: scope vs DAVE complexity

> 🔵 **İNSAN:** v1.5/v2 zamanlaması?
> - v1.5: küçük update, native voice POC
> - v2: full multi-account + voice
> - v3: Servo backend + native voice (eğer Servo gerekirse)

---

## 7. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| DAVE protocol değişir | Kırılma | libdave upstream takip, semver pin |
| Opus latency | Ses kalitesi | Buffer tuning, jitter compensation |
| UDP firewall | Ses gönderilemez | TCP fallback (Discord destekliyor), STUN/TURN |
| E2EE performance | CPU spike | Hardware AES-NI, jemalloc |
| Screen share izin | Kullanıcı reddederse | Default off, settings'ten aç |

---

## 8. Çıkış → Faz 8.0

Eğer atlandıysa (önerilen): Faz 7 dosyası kapatılır, release plan'ı Faz 8'e atlar.
Eğer aktif edildiyse: Voice çalışıyor, Faz 8 polish.

Faz 8.0 → Distribution + auto-update + WebView2 bellek yönetimi polish.
