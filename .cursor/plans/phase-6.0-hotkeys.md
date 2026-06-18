---
name: Phase 6.0 — Hotkeys + Entegrasyon
overview: Global hotkeys (mute mic, deafen), window-specific hotkeys (quick switcher, settings), drag & drop dosya paylaşımı, deep linking (viscos://), Windows auto-start, single-instance (named pipe/mutex), Vencord/Equicord plugin tam entegrasyonu.
isProject: false
todos:
  - id: global-hotkeys
    content: Global hotkeys (Ctrl+Shift+M: mute, Ctrl+Shift+D: deafen)
    status: pending
  - id: window-hotkeys
    content: Window hotkeys (Ctrl+K: quick switcher, Ctrl+/: settings)
    status: pending
  - id: drag-drop
    content: Drag & drop dosya paylaşımı
    status: pending
  - id: deep-linking
    content: Deep linking (viscos://channel/123, viscos://invite/xyz)
    status: pending
  - id: auto-start
    content: Windows auto-start (registry HKCU\...\Run)
    status: pending
  - id: single-instance
    content: Single-instance (named pipe / mutex)
    status: pending
  - id: vencord-full
    content: Vencord/Equicord plugin tam entegrasyonu (Faz 5 POC'den)
    status: pending
  - id: vencord-marketplace
    content: Plugin marketplace endpoint'leri (settings, install, list)
    status: pending
---

# Phase 6.0 — Hotkeys + Entegrasyon

> **Süre:** 1 hafta
> **Hedef:** Klavye kısayolları, dosya paylaşımı, deep linking, single-instance, Vencord tam entegrasyon.
> **Önceki faz:** [`phase-5.0-native-ui.md`](./phase-5.0-native-ui.md)
> **Sonraki faz:** [`phase-7.0-voice-video.md`](./phase-7.0-voice-video.md)

---

## 1. Workspace Dependencies

```toml
[workspace.dependencies]
# Hotkeys
global-hotkey = "0.6"
muda = "0.15"

# Single-instance
single-instance = "0.3"
named-lock = "0.4"

# Auto-start (Windows)
auto-launch = "0.5"
```

---

## 2. Global Hotkeys

### 2.1 `crates/viscos-shell/src/hotkeys.rs`

```rust
use global_hotkey::{GlobalHotKeyManager, HotKeyState, GlobalHotKey, hotkey::{HotKey, Modifiers, Code}};
use viscos_audio::AudioControl;
use std::sync::Arc;

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    audio: Arc<AudioControl>,
}

impl HotkeyManager {
    pub fn new(audio: Arc<AudioControl>) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        Ok(Self { manager, audio })
    }
    
    pub fn register_defaults(&self) -> anyhow::Result<()> {
        // Mute toggle: Ctrl+Shift+M
        let mute = GlobalHotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Code::KeyM,
            "viscos.mute",
        );
        // Deafen toggle: Ctrl+Shift+D
        let deafen = GlobalHotKey::new(
            Some(Modifiers::CONTROL | Modifiers::SHIFT),
            Code::KeyD,
            "viscos.deafen",
        );
        
        self.manager.register(mute)?;
        self.manager.register(deafen)?;
        Ok(())
    }
    
    pub async fn handle_event(&self, event: GlobalHotKeyEvent) -> anyhow::Result<()> {
        match event.id().as_str() {
            "viscos.mute" => {
                self.audio.toggle_mute().await?;
                tracing::info!("Mute toggled");
            }
            "viscos.deafen" => {
                self.audio.toggle_deafen().await?;
                tracing::info!("Deafen toggled");
            }
            _ => {}
        }
        Ok(())
    }
}
```

**Config-time customization:** Kullanıcı settings'ten tuş kombinasyonlarını değiştirebilir (`config.toml`).

### 2.2 Window-Specific Hotkeys (muda)

```rust
// crates/viscos-shell/src/window_hotkeys.rs
use muda::{Menu, MenuItem, accelerator::{Accelerator, Code, Modifiers}, MenuEvent};

pub fn register_app_hotkeys(app: &tao::AppHandle) -> anyhow::Result<()> {
    // Ctrl+K: Quick switcher
    let quick_switcher = MenuItem::with_id(
        "quick_switcher",
        "Quick Switcher",
        true,
        Some(Accelerator::new(Some(Modifiers::CONTROL), Code::KeyK)),
    );
    // Ctrl+/: Settings
    let settings = MenuItem::with_id(
        "settings",
        "Settings",
        true,
        Some(Accelerator::new(Some(Modifiers::CONTROL), Code::Slash)),
    );
    // Ctrl+Shift+I: DevTools
    let devtools = MenuItem::with_id(
        "devtools",
        "Toggle DevTools",
        true,
        Some(Accelerator::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyI)),
    );
    
    let menu = Menu::new();
    menu.append_items(&[&quick_switcher, &settings, &devtools])?;
    app.set_menu(menu);
    
    Ok(())
}
```

---

## 3. Drag & Drop Dosya Paylaşımı

```rust
// crates/viscos-shell/src/drag_drop.rs
use tao::event::WindowEvent;
use viscos_media::MediaUploader;

pub fn handle_drop(
    file_paths: Vec<std::path::PathBuf>,
    current_channel: &str,
    uploader: &MediaUploader,
) -> anyhow::Result<()> {
    for path in file_paths {
        if path.is_file() {
            uploader.upload(path, current_channel)?;
        }
    }
    Ok(())
}

// tao::WindowBuilder'da:
// .with_file_drop_enabled(true)
```

Frontend tarafında:
```typescript
// frontend/src/drop.ts
window.addEventListener('dragover', (e) => e.preventDefault());
window.addEventListener('drop', (e) => {
  e.preventDefault();
  const files = Array.from(e.dataTransfer?.files ?? []);
  if (files.length > 0) {
    window.viscos.invoke({
      type: 'UploadFiles',
      data: { 
        paths: files.map(f => (f as any).path),  // Electron-style, wry desteği farklı
        channel_id: getCurrentChannelId(),
      }
    });
  }
});
```

---

## 4. Deep Linking

### 4.1 Custom URI Scheme Kaydı (Windows)

```rust
// crates/viscos-shell/src/deep_link.rs
use winreg::enums::*;
use winreg::RegKey;
use std::path::PathBuf;

pub fn register_uri_scheme(exe_path: &PathBuf) -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.create_subkey(r"Software\Classes\viscos")?;
    key.set_value("URL Protocol", &"")?;
    key.set_value("", &"URL:Viscos Protocol")?;
    
    let icon = key.create_subkey("DefaultIcon")?;
    icon.set_value("", &format!("\"{}\",0", exe_path.display()))?;
    
    let shell = key.create_subkey(r"shell\open\command")?;
    shell.set_value("", &format!("\"{}\" \"%1\"", exe_path.display()))?;
    
    Ok(())
}
```

### 4.2 URI Parsing

```rust
// crates/viscos-shell/src/deep_link.rs (devamı)
#[derive(Debug, Clone)]
pub enum DeepLink {
    Channel { guild_id: Option<String>, channel_id: String },
    Invite { code: String },
    User { user_id: String },
    Plugin { plugin_id: String, action: Option<String> },
    Unknown(String),
}

pub fn parse(uri: &str) -> DeepLink {
    let stripped = uri.strip_prefix("viscos://").unwrap_or(uri);
    let parts: Vec<&str> = stripped.split('/').collect();
    
    match parts.first().copied() {
        Some("channel") => {
            // viscos://channel/{id} veya viscos://channel/{guild}/{id}
            if parts.len() == 2 {
                DeepLink::Channel { guild_id: None, channel_id: parts[1].to_string() }
            } else if parts.len() == 3 {
                DeepLink::Channel { 
                    guild_id: Some(parts[1].to_string()),
                    channel_id: parts[2].to_string(),
                }
            } else {
                DeepLink::Unknown(uri.to_string())
            }
        }
        Some("invite") if parts.len() == 2 => {
            DeepLink::Invite { code: parts[1].to_string() }
        }
        Some("user") if parts.len() == 2 => {
            DeepLink::User { user_id: parts[1].to_string() }
        }
        Some("plugin") if parts.len() >= 2 => {
            DeepLink::Plugin {
                plugin_id: parts[1].to_string(),
                action: parts.get(2).map(|s| s.to_string()),
            }
        }
        _ => DeepLink::Unknown(uri.to_string()),
    }
}

pub async fn handle_deep_link(
    link: DeepLink,
    app: &tao::AppHandle,
    state: &AppState,
) -> anyhow::Result<()> {
    match link {
        DeepLink::Channel { guild_id, channel_id } => {
            // Pencereyi öne getir, kanalı seç
            app.show();
            state.select_channel(channel_id);
        }
        DeepLink::Invite { code } => {
            // API call: accept invite
            let rest = state.rest_client();
            let invite: Invite = rest.get(&format!("/invites/{}", code)).await?;
            rest.post_empty(&format!("/invites/{}", code)).await?;
            state.refresh_guild(invite.guild_id).await?;
        }
        DeepLink::User { user_id } => {
            // DM aç
            let rest = state.rest_client();
            let dm: DMChannel = rest.post_json("/users/@me/channels", &json!({ "recipient_id": user_id })).await?;
            state.select_channel(dm.id);
        }
        DeepLink::Plugin { plugin_id, action } => {
            // Plugin yükle/etkinleştir
            state.plugin_manager().handle_deep_link(plugin_id, action).await?;
        }
        DeepLink::Unknown(uri) => {
            tracing::warn!(%uri, "Unknown deep link");
        }
    }
    Ok(())
}
```

### 4.3 Single-Instance + Deep Link Forward

İlk instance URI'yi handle eder, ikinci instance komut satırından URI alıp birinciye forward eder:

```rust
// crates/viscos/src/main.rs
use single_instance::SingleInstance;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let deep_link = args.get(1).filter(|s| s.starts_with("viscos://")).cloned();
    
    let instance = SingleInstance::new("viscos-single-instance")?;
    if !instance.is_single() {
        // İkinci instance: URI'yi birinciye gönder
        if let Some(uri) = deep_link {
            send_to_primary_instance(&uri)?;
        }
        return Ok(());
    }
    
    // Birincil instance
    if let Some(uri) = deep_link {
        // Direkt handle et
        let link = deep_link::parse(&uri);
        // ... event loop'ta handle edilecek
    }
    
    // ... normal başlatma
    Ok(())
}
```

---

## 5. Auto-Start (Windows Registry)

```rust
// crates/viscos-shell/src/auto_start.rs
use auto_launch::AutoLaunchBuilder;
use std::path::PathBuf;

pub struct AutoStart;

impl AutoStart {
    pub fn enable(exe: &PathBuf) -> anyhow::Result<()> {
        let launcher = AutoLaunchBuilder::new()
            .set_app_name("Viscos")
            .set_app_path(exe.to_str().unwrap())
            .set_args(&["--minimized"])
            .build()?;
        launcher.enable()?;
        Ok(())
    }
    
    pub fn disable() -> anyhow::Result<()> {
        let launcher = AutoLaunchBuilder::new()
            .set_app_name("Viscos")
            .build()?;
        launcher.disable()?;
        Ok(())
    }
    
    pub fn is_enabled() -> anyhow::Result<bool> {
        let launcher = AutoLaunchBuilder::new()
            .set_app_name("Viscos")
            .build()?;
        Ok(launcher.is_enabled()?)
    }
}
```

Settings UI toggle: "Sistem açılışında başlat" + "--minimized" argümanı (tray-only mode).

---

## 6. Single-Instance

```rust
// crates/viscos/src/main.rs
use single_instance::SingleInstance;

let instance = SingleInstance::new("viscos-singleton-v1")?;
if !instance.is_single() {
    // İkinci instance: birincil'e mesaj gönder, çık
    if let Some(uri) = std::env::args().nth(1) {
        let _ = send_ipc_message(&uri);  // NamedPipe üzerinden
    }
    std::process::exit(0);
}
```

**Named pipe server (birincil instance):**
```rust
// NamedPipeServer::new("viscos-ipc")
// Gelen mesajları dinle, deep link olarak parse et, event loop'a forward et
```

---

## 7. Vencord/Equicord Tam Entegrasyonu (Faz 5 POC'den)

### 7.1 Plugin Manager

```rust
// crates/viscos-plugin/src/manager.rs
use viscos_ipc::IpcBridge;
use std::path::PathBuf;
use std::collections::HashMap;

pub struct PluginManager {
    plugins: HashMap<String, Plugin>,
    storage_dir: PathBuf,
}

pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub path: PathBuf,
}

impl PluginManager {
    pub async fn load_from_dir(&mut self, dir: &Path) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Some(plugin) = Plugin::from_dir(entry.path())? {
                    self.plugins.insert(plugin.id.clone(), plugin);
                }
            }
        }
        Ok(())
    }
    
    pub async fn install(&mut self, plugin_id: &str, source: PluginSource) -> anyhow::Result<()> {
        // İndir, doğrula (imza/sha256), self.plugins'a ekle
        todo!("Faz 6'da implement")
    }
    
    pub fn enable(&mut self, plugin_id: &str) -> anyhow::Result<()> {
        if let Some(p) = self.plugins.get_mut(plugin_id) {
            p.enabled = true;
        }
        Ok(())
    }
}
```

### 7.2 Plugin Storage Layout

```
%APPDATA%/Viscos/plugins/
├── vencord-betterdiscord-themes/
│   ├── manifest.json
│   ├── index.js
│   └── styles.css
├── viscos-spellcheck-tr/
│   ├── manifest.json
│   └── index.js
└── ...
```

`manifest.json`:
```json
{
  "id": "vencord-betterdiscord-themes",
  "name": "BetterDiscord Themes",
  "version": "1.0.0",
  "author": "Vencord",
  "description": "Loads BetterDiscord themes via Vencord",
  "entry": "index.js",
  "vencord_compat": "1.5.0+",
  "permissions": ["themes.read", "css.inject"]
}
```

### 7.3 Preload Bridge Genişletme

```typescript
// frontend/src/vencord-bridge.ts (Faz 5'ten genişletilmiş)
window.ViscosNative = {
  win: {
    getVersion: () => window.viscos.invoke({ type: 'AppVersion' }),
    getFeatures: () => Promise.resolve(['native-shell', 'plugin-system', 'gdi-watchdog']),
  },
  settings: {
    get: (key: string) => window.viscos.invoke({ type: 'SettingsGet', data: { key } }),
    set: (key: string, value: any) => window.viscos.invoke({ type: 'SettingsSet', data: { key, value } }),
  },
  spellcheck: {
    getAvailableLanguages: () => window.viscos.invoke({ type: 'SpellcheckLanguages' }),
  },
  commands: {
    register: (cmd: any) => window.viscos.invoke({ type: 'CommandRegister', data: cmd }),
  },
  plugins: {
    list: () => window.viscos.invoke({ type: 'PluginList' }),
    install: (id: string) => window.viscos.invoke({ type: 'PluginInstall', data: { id } }),
    enable: (id: string) => window.viscos.invoke({ type: 'PluginEnable', data: { id } }),
    disable: (id: string) => window.viscos.invoke({ type: 'PluginDisable', data: { id } }),
  },
  // ... 11 namespace'in tamamı
};

window.Vencord = { Api: window.ViscosNative, /* ... */ };
```

### 7.4 Plugin Marketplace (İlk Versiyon)

Yerel klasör tabanlı (remote registry Faz v2'de):

```rust
// Yerel bilinen plugin'ler listesi
const FEATURED_PLUGINS: &[(&str, &str, &str)] = &[
    ("vencord-betterdiscord-themes", "BetterDiscord Themes via Vencord", "1.0.0"),
    ("vencord-message-link-embeds", "Message Link Embeds", "1.5.0"),
    ("vencord-platform-indicators", "Platform Indicators", "1.5.0"),
];
```

Settings UI'da "Browse Plugins" sekmesi → listele → install butonu → GitHub Releases'den indir → validate → etkinleştir.

---

## 8. Test Stratejisi (Faz 6.0)

| Test | Tip | Kabul |
|------|-----|-------|
| Global hotkey kayıt | Integration | Ctrl+Shift+M basıldığında audio callback |
| Quick switcher | Manuel (lokal) | Ctrl+K → kanal arama açılır |
| Drag drop dosya | Integration | Discord channel'a dosya yüklenir |
| Deep link `viscos://channel/123` | Integration | Kanal seçiliyor, pencere önde |
| Auto-start enable/disable | Integration (Windows) | Registry doğru yazılıyor |
| Single-instance | Integration | İkinci instance URI'yi birinciye forward ediyor |
| Plugin install | Integration | `viscos://plugin/...` yüklüyor |
| Plugin enable | Integration | WebView'de plugin aktif |

---

## 9. Kabul Kriterleri (Definition of Done)

- [ ] Global hotkeys (mute, deafen) çalışıyor
- [ ] Window hotkeys (quick switcher, settings, DevTools) çalışıyor
- [ ] Drag & drop dosya paylaşımı çalışıyor
- [ ] Deep linking: `viscos://channel/`, `viscos://invite/`, `viscos://user/`, `viscos://plugin/`
- [ ] Auto-start: registry enable/disable
- [ ] Single-instance: ikinci açılışta birincil'e forward
- [ ] Vencord tam entegrasyon: 11 namespace expose
- [ ] En az 1 plugin marketplace'ten yüklenip etkinleştirilebiliyor
- [ ] `cargo clippy -- -D warnings` temiz
- [ ] Tüm testler geçer

---

## 10. Karar Noktası (Faz 6.0 Sonu)

> 🔵 **İNSAN:** Auto-start varsayılanı ne olsun?
> - Disabled (önerilen, kullanıcı bilinçli açar)
> - Enabled (--minimized) (Discord hissi)
> - First-launch sor

> 🔵 **İNSAN:** Deep link hangi event'leri desteklesin?
> - Channel, invite, user, plugin (önerilen, hepsi)
> - Sadece channel + invite (minimal)
> - Tüm event'ler + custom URL'ler (Vesktop eşdeğeri)

> 🔵 **İNSAN:** Plugin imzalama/doğrulama?
> - Hash check (her plugin için sha256, kullanıcı onaylar)
> - İmzalı plugin (GPG/sertifika, daha sonra)
> - Trust-on-install (Discord/npm modeli)
> - Trade-off: güvenlik vs UX

---

## 11. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| Hotkey çakışması (başka uygulama) | Çalışmaz | Çakışma kontrolü, alternatif kısayol |
| Deep link kötüye kullanım | Keyfi komut | Whitelist scheme, validate URI |
| Auto-start yavaşlatır | Boot süresi | --minimized argümanı, lazy load |
| Single-instance race | İkisi de birincil olur | Named lock atomic |
| Plugin zararlı | Sistem compromise | Permission system, hash check, sandbox (ileride) |
| Registry yanlış yazılır | Windows bozulma | Try/catch, rollback |

---

## 12. Çıkış → Faz 7.0

Bu faz tamamlandığında:
- Klavye kısayolları, deep linking, single-instance çalışıyor
- Vencord plugin ekosistemi hazır
- Drag & drop dosya paylaşımı var

Faz 7.0 → Voice/Video (opsiyonel, v1'de atlanması önerilir).
