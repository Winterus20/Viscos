---
name: Phase 5.0 — Native UI (iced) + Vencord POC
overview: iced native side panel (sunucu/kanal listesi, üye listesi, kullanıcı paneli, ayarlar, mesaj gönderme kutusu, autocomplete), tema sistemi, native bildirimler, tray context menu, Vencord/Equicord plugin uyumu POC.
isProject: false
todos:
  - id: theme-system
    content: Tema sistemi (Discord karanlık/açık + custom)
    status: pending
  - id: side-panel-decision
    content: Side panel mimari kararı: native mi Discord UI içinde mi
    status: pending
  - id: guild-list
    content: Sunucu listesi (guild icon, mention badge, animasyon)
    status: pending
  - id: channel-list
    content: Kanal listesi (kategori, kanal tipi, mention, mute)
    status: pending
  - id: member-list
    content: Üye listesi (rol bazlı gruplama, çevrimiçi durumu)
    status: pending
  - id: user-panel
    content: Kullanıcı paneli (avatar, username, mikrofon, ses)
    status: pending
  - id: message-composer
    content: Mesaj gönderme kutusu (focus, autocomplete placeholder)
    status: pending
  - id: settings-window
    content: Ayarlar penceresi (multi-tab, keyboard nav)
    status: pending
  - id: tray-menu
    content: Tray context menu (status, quick switcher)
    status: pending
  - id: native-notifications
    content: Native bildirimler (mention, DM, voice)
    status: pending
  - id: vencord-poc
    content: Vencord/Equicord plugin uyumu POC (preload bridge)
    status: pending
  - id: vencord-first-plugin
    content: İlk hedef plugin: BetterDiscord temaları Vencord üzerinden
    status: pending
---

# Phase 5.0 — Native UI (iced) + Vencord POC

> **Süre:** 3–4 hafta
> **Hedef:** Vesktop'tan gerçek farklılaşma: native side panel. iced widget'ları ile sunucu/kanal listesi. Vencord uyumu POC.
> **Önceki faz:** [`phase-4.0-cache-media.md`](./phase-4.0-cache-media.md)
> **Sonraki faz:** [`phase-6.0-hotkeys.md`](./phase-6.0-hotkeys.md)

---

## 1. Neden Erken?

Vesktop'tan gerçek farklılaşma noktası **native side panel**. Vencord uyumu Faz 5 sonunda POC, Faz 6'da tam entegrasyon.

---

## 2. Side Panel Mimari Kararı (KRİTİK)

İki seçenek:

### Seçenek A: Side Panel Tamamen iced Native (ÖNERİLEN)

```
┌──────┬───────────┬─────────────────────┐
│      │           │                     │
│ iced │ iced      │   WebView2          │
│Guild │ Channel   │   Discord.com/app   │
│List  │ List      │   (mesaj alanı)     │
│      │           │                     │
│ 80px │ 240px     │   880px             │
│      │           │                     │
└──────┴───────────┴─────────────────────┘
```

**+** En yüksek RAM tasarrufu (Discord web sadece mesaj alanı)
**+** Side panel native, hızlı, teması özelleştirilebilir
**+** Vesktop'tan farklılaşma
**−** Side panel + WebView senkronizasyonu zor (seçili kanal state paylaşımı)
**−** Discord'un kendi sol panelini "gizlemek" için CSS hack gerekli

### Seçenek B: Side Panel WebView2 İçinde (Vesktop/Dorion Yaklaşımı)

```
┌──────────────────────────────────────┐
│                                      │
│   WebView2 (Discord.com/app)         │
│   - Side panel (Discord'un kendi)     │
│   - Mesaj alanı                      │
│   - Üye listesi                      │
│                                      │
└──────────────────────────────────────┘
```

**+** Daha az kod
**+** Discord ile %100 uyumlu (özellikler anında)
**−** Vesktop'tan farkı sadece "Tauri'siz" — az
**−** RAM tasarrufu sınırlı

**Önerim: Seçenek A.** Vesktop'tan gerçek farklılaşma için native side panel kritik.

---

## 3. iced State Management

### 3.1 `crates/viscos-shell/src/state.rs`

```rust
use viscos_core::types::{Guild, Channel, Message, User};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct AppState {
    pub user: Option<User>,
    pub guilds: HashMap<String, Guild>,
    pub guild_order: Vec<String>,
    pub channels: HashMap<String, Channel>,
    pub members: HashMap<String, Vec<User>>,
    pub current_guild: Option<String>,
    pub current_channel: Option<String>,
    pub messages: HashMap<String, Vec<Message>>,
    pub unread_counts: HashMap<String, u32>,
    pub typing: HashMap<String, Vec<String>>,
    pub theme: Theme,
}

impl AppState {
    pub fn unread_for_guild(&self, guild_id: &str) -> u32 {
        self.channels.iter()
            .filter(|(_, c)| c.guild_id.as_deref() == Some(guild_id))
            .map(|(id, _)| self.unread_counts.get(id).copied().unwrap_or(0))
            .sum()
    }
}
```

### 3.2 Message Bus

```rust
// crates/viscos-shell/src/bus.rs
use viscos_core::events::GatewayEvent;
use tokio::sync::broadcast;

pub struct MessageBus {
    gateway_rx: broadcast::Receiver<GatewayEvent>,
    state_tx: iced::Subscription<Message>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Gateway(GatewayEvent),
    SelectChannel(String),
    SelectGuild(String),
    SendMessage(String),
    Settings,
    Quit,
}
```

---

## 4. Theme System

```rust
// crates/viscos-shell/src/theme.rs
use iced::{Color, Theme as IcedTheme};

pub struct ViscosTheme {
    pub background: Color,
    pub sidebar: Color,
    pub mention_badge: Color,
    pub text_primary: Color,
    pub text_muted: Color,
    pub accent: Color,
    pub hover: Color,
}

impl ViscosTheme {
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb(0.094, 0.094, 0.114),     // #18181D
            sidebar: Color::from_rgb(0.078, 0.078, 0.094),        // #141418
            mention_badge: Color::from_rgb(0.847, 0.298, 0.298),   // #D84C4C
            text_primary: Color::from_rgb(0.847, 0.847, 0.847),    // #D8D8D8
            text_muted: Color::from_rgb(0.467, 0.467, 0.502),      // #777880
            accent: Color::from_rgb(0.345, 0.396, 0.949),          // #5865F2 (Discord blurple)
            hover: Color::from_rgb(0.157, 0.157, 0.184),           // #28282F
        }
    }
    
    pub fn light() -> Self {
        Self {
            background: Color::WHITE,
            sidebar: Color::from_rgb(0.961, 0.961, 0.973),
            mention_badge: Color::from_rgb(0.847, 0.298, 0.298),
            text_primary: Color::from_rgb(0.063, 0.063, 0.094),
            text_muted: Color::from_rgb(0.345, 0.345, 0.376),
            accent: Color::from_rgb(0.345, 0.396, 0.949),
            hover: Color::from_rgb(0.918, 0.918, 0.937),
        }
    }
}
```

**Custom CSS injection (Faz 6'da):** Discord WebView2 içine custom CSS inject edilir (side panel'i gizle, native panel ile çakışmasın).

---

## 5. Side Panel Widget'ları

### 5.1 Guild List

```rust
// crates/viscos-shell/src/widgets/guild_list.rs
use iced::{
    widget::{Column, Container, Image, Row, Text, button, scrollable},
    Element, Length,
};
use viscos_core::types::Guild;

pub fn guild_list<'a>(
    guilds: &'a [Guild],
    selected: Option<&'a str>,
    unread_counts: impl Fn(&str) -> u32 + 'a,
    on_select: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let mut col = Column::new()
        .width(Length::Fixed(72.0))
        .padding(8)
        .spacing(8);
    
    // Home button (DM list)
    col = col.push(home_button(selected.is_none(), unread_dm_count));
    
    // Divider
    col = col.push(divider());
    
    for guild in guilds {
        let is_selected = selected == Some(guild.id.as_str());
        let unread = unread_counts(&guild.id);
        col = col.push(guild_button(guild, is_selected, unread, on_select.clone()));
    }
    
    // Add server button
    col = col.push(add_server_button());
    
    Container::new(scrollable(col))
        .style(theme::Sidebar)
        .height(Length::Fill)
        .into()
}

fn guild_button(
    guild: &Guild,
    is_selected: bool,
    unread: u32,
    on_select: impl Fn(String) -> Message,
) -> Element<Message> {
    let mut btn = button(
        Container::new(
            Image::new(guild_icon_url(guild))
                .width(48)
                .height(48)
                .content_fit(ContentFit::Cover)
        )
        .clip(Clip::Rounded(24.0))  // pill shape Discord-style
    )
    .on_press(on_select(guild.id.clone()))
    .padding(0)
    .style(if is_selected { theme::SelectedPill } else { theme::PillButton });
    
    if unread > 0 {
        btn = btn.overlay(mention_badge(unread));
    }
    
    btn.into()
}
```

### 5.2 Channel List

```rust
// crates/viscos-shell/src/widgets/channel_list.rs
pub fn channel_list<'a>(
    channels: &'a [Channel],
    selected: Option<&'a str>,
    on_select: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let mut col = Column::new().width(Length::Fixed(240.0)).padding(8);
    
    // Gruplama: category > channels
    let categories = channels.iter().filter(|c| c.kind == ChannelType::Category);
    for cat in categories {
        col = col.push(category_header(cat));
        for ch in channels.iter().filter(|c| c.parent_id.as_deref() == Some(&cat.id)) {
            col = col.push(channel_button(ch, selected == Some(ch.id.as_str()), on_select.clone()));
        }
    }
    
    // DM'ler ayrı grup
    col = col.push(divider());
    col = col.push(Text::new("Direct Messages").size(12));
    for ch in channels.iter().filter(|c| c.guild_id.is_none()) {
        col = col.push(channel_button(ch, selected == Some(ch.id.as_str()), on_select.clone()));
    }
    
    Container::new(scrollable(col)).style(theme::Sidebar).height(Length::Fill).into()
}
```

### 5.3 Member List (Sağ Panel)

```rust
// crates/viscos-shell/src/widgets/member_list.rs
pub fn member_list<'a>(
    members: &'a [User],
    online_only: bool,
) -> Element<'a, Message> {
    let mut col = Column::new()
        .width(Length::Fixed(240.0))
        .padding(8);
    
    let mut online: Vec<&User> = members.iter().filter(|u| !u.is_offline()).collect();
    let mut offline: Vec<&User> = members.iter().filter(|u| u.is_offline()).collect();
    online.sort_by_key(|u| u.display_name());
    offline.sort_by_key(|u| u.display_name());
    
    if !online.is_empty() {
        col = col.push(Text::new(format!("Online — {}", online.len())).size(12));
        for u in online {
            col = col.push(member_row(u));
        }
    }
    
    if !online_only && !offline.is_empty() {
        col = col.push(Text::new(format!("Offline — {}", offline.len())).size(12));
        for u in offline {
            col = col.push(member_row(u));
        }
    }
    
    Container::new(scrollable(col)).style(theme::Sidebar).height(Length::Fill).into()
}
```

### 5.4 Message Composer (Alt)

```rust
// crates/viscos-shell/src/widgets/composer.rs
use iced::widget::{TextInput, Container, Row};

pub struct ComposerState {
    text: String,
    replying_to: Option<String>,
}

pub fn composer(state: &ComposerState, on_send: impl Fn(String) -> Message) -> Element<Message> {
    Container::new(
        Row::new()
            .push(
                TextInput::new("Message #channel", &state.text)
                    .on_input(Message::ComposerChanged)
                    .on_submit(on_send(state.text.clone()))
                    .padding(12)
                    .width(Length::Fill)
            )
            .padding(8)
    )
    .style(theme::Composer)
    .height(Length::Fixed(60.0))
    .into()
}
```

### 5.5 Layout

```rust
// crates/viscos-shell/src/app.rs
pub fn view(&self) -> Element<Message> {
    Row::new()
        .push(guild_list(&self.state.guilds, ...))        // 72px
        .push(channel_list(&self.state.channels, ...))    // 240px
        .push(member_list(&self.state.members, ...))      // 240px
        .push(self.webview_area())                         // Fill
        .push(composer(&self.composer_state, ...))         // 60px alt
        .into()
}
```

**Not:** Aslında `composer` ve `webview_area` ortada (mesaj alanı) birlikte, guild/channel/member etrafını sarar.

```rust
// Gerçek layout:
Row::new()
    .push(guild_list)               // 72px sol
    .push(
        Column::new()
            .push(channel_list)     // 240px orta-sol
            .push(webview_area)     // Fill
            .push(composer)         // 60px alt
    )
    .push(member_list)              // 240px sağ
```

---

## 6. Native Bildirimler (tao + Windows)

```rust
// crates/viscos-shell/src/notification.rs
use tauri_winrt_notification::{Duration, Sound};
use std::path::Path;

pub fn show_mention(username: &str, content: &str, avatar_path: Option<&Path>) -> anyhow::Result<()> {
    let mut notification = tauri_winrt_notification::Notification::new();
    notification
        .title(format!("{} mentioned you", username))
        .text(content)
        .icon(avatar_path.unwrap_or(Path::new("assets/viscos.ico")).to_str().unwrap())
        .sound(Some(Sound::Default))
        .duration(Duration::Short);
    notification.show()?;
    Ok(())
}
```

---

## 7. Vencord/Equicord Plugin Uyumu (POC)

### 7.1 Mimari

Vesktop'un `VesktopNative` bridge API'si (11 namespace: `win`, `virtmic`, `settings`, `spellcheck`, `commands`, ...) referans alınarak Viscos'un kendi `ViscosNative` API'si tasarlanır.

### 7.2 `frontend/src/preload-bridge.ts` (Vencord köprüsü)

```typescript
// frontend/src/vencord-bridge.ts
// Preload script'in sonuna inject edilir
// Vencord'un beklediği window.Vencord, window.equicord global'lerini expose et

declare global {
  interface Window {
    Vencord?: any;
    equicord?: any;
    ViscosNative: ViscosNativeApi;
  }
}

interface ViscosNativeApi {
  win: {
    getVersion: () => Promise<string>;
    getFeatures: () => Promise<string[]>;
  };
  settings: {
    get: (key: string) => Promise<any>;
    set: (key: string, value: any) => Promise<void>;
  };
  spellcheck: {
    getAvailableLanguages: () => Promise<string[]>;
  };
  commands: {
    register: (cmd: any) => Promise<void>;
  };
  // ...
}

// Vencord API'sini simüle et (Equirust pattern)
window.Vencord = {
  Api: window.ViscosNative,
  // ... Vencord'un beklediği diğer alanlar
};
window.equicord = window.Vencord;
```

### 7.3 Custom Protocol: `viscos://plugin/`

```rust
// crates/viscos-webview/src/protocol.rs
use wry::WebViewExtWindows;

pub fn register_plugin_protocol(webview: &wry::WebView) -> anyhow::Result<()> {
    webview.with_webview(|webview2| unsafe {
        let core = webview2.controller().CoreWebView2().unwrap();
        core.AdditionalBrowserArguments(
            "viscos://"
        );
    })?;
    Ok(())
}
```

Vencord plugin'leri `viscos://plugin/{id}/...` üzerinden kendi dosyalarını okur.

### 7.4 İlk Hedef Plugin

BetterDiscord temalarını Vencord üzerinden yükleme:
- Vencord zaten CSS injection destekliyor
- Viscos preload bridge'i `ViscosNative.themes.install(cssUrl)` sağlar
- Tema `viscos://themes/{id}.css` üzerinden diskten okunur

---

## 8. Test Stratejisi (Faz 5.0)

| Test | Tip | Kabul |
|------|-----|-------|
| Side panel render | Integration (lokal) | 50 sunucu scroll akıcı |
| Channel switch | Integration | Seçili kanal değişiyor |
| Mention badge animasyon | Manuel | Pulse efekti görünüyor |
| Message send | Integration (mock API) | Mesaj REST'e gidiyor |
| Native notification | Manuel (lokal) | Mention bildirimi çıkıyor |
| Vencord bridge | Integration | `window.Vencord.Api.settings.get()` çalışıyor |
| Theme switch | Integration | Karanlık → açık geçiş |

---

## 9. Kabul Kriterleri (Definition of Done)

- [ ] Side panel iced native render
- [ ] Sunucu listesi + kanal listesi + üye listesi + kullanıcı paneli
- [ ] Mesaj gönderme kutusu (composer) çalışıyor
- [ ] Tema karanlık/açık
- [ ] Native bildirimler (mention, DM)
- [ ] Tray context menu (status, quick switcher)
- [ ] **Vencord POC:** `window.Vencord.Api` expose, en az 1 plugin yüklenebiliyor
- [ ] WebView2 ile side panel senkronize (seçili kanal)
- [ ] `cargo clippy -- -D warnings` temiz
- [ ] Tüm testler geçer
- [ ] 1 saatlik lokal soak: UI lag yok, memory growth < %15

---

## 10. Karar Noktası (Faz 5.0 Sonu)

> 🔵 **İNSAN:** Side panel tamamen native mi, yoksa Discord'un UI'ı içinde mi?
> - **Seçenek A (önerilen):** iced native, Vesktop'tan farklılaşma
> - **Seçenek B:** WebView2 içinde, az kod

> 🔵 **İNSAN:** Vencord API yüzeyi ne kadar expose edilecek?
> - **Minimal:** sadece settings + themes (güvenli)
> - **Tam:** 11 namespace (Vesktop eşdeğeri, plugin ekosistemi dev)
> - Trade-off: güvenlik yüzeyi vs plugin ekosistemi

> 🔵 **İNSAN:** Custom CSS injection agresifliği?
> - Sadece side panel gizleme (varsayılan)
> - Full Discord custom CSS (kullanıcı ayarlardan)
> - Trade-off: özelleştirme vs stabilite (Discord UI değişirse kırılır)

---

## 11. Riskler ve Azaltma

| Risk | Etki | Azaltma |
|------|------|---------|
| Side panel + WebView senkronizasyonu | UI flicker | IPC bridge, debounce 50ms |
| iced API breaking change | Refactor | iced 0.13 stable'a pin, breaking'de güncelle |
| Vencord API değişir | Plugin kırılma | Versiyon check, fallback "Vencord yok" modu |
| Custom CSS Discord update | Kırılma | Kullanıcı sıfırlayabilsin, minimal default |
| Native notification spam | Kullanıcı rahatsız | Ayarlardan mention-only seçeneği |
| 50+ sunucu scroll lag | UI donma | Virtual scroll (sadece görünür), pagination |

---

## 12. Çıkış → Faz 6.0

Bu faz tamamlandığında:
- Vesktop'tan farklılaşmış native UI
- Side panel + mesaj alanı senkron
- Vencord uyumu POC başarılı
- Tema, bildirim, tray çalışıyor

Faz 6.0 → Hotkeys + Vencord tam entegrasyonu + drag&drop + deep linking.
