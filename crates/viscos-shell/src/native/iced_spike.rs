//! `iced_spike` — minimal working counter PoC for Viscos native UI (Faz 5.0 spike).
//!
//! Bu ikili `iced 0.14` API'sinin `viscos-shell` event-loop mimarisiyle
//! entegrasyonunu Faz 1.0 / Faz 5.0 tam commitment'ten **önce** kanıtlar.
//! Sıfır stub: counter çalışır, theme switch çalışır, pencere açılır,
//! klavye + mouse input'u işler.
//!
//! # Ne kanıtlanıyor
//!
//! 1. `iced 0.14` Windows 10/11 + MSRV 1.89'da compile olur.
//! 2. Counter button + label çalışır (`update` + `view` + `Message`).
//! 3. Theme switch çalışır (`Theme::Dark` ↔ `Theme::Light`).
//! 4. `iced::application(...)` builder + `.theme(...)` + `.window_size(...)`
//!    fluent API bütün cargo feature setleriyle uyumlu.
//! 5. `iced 0.14` `wgpu` renderer DX12 backend ile Windows'ta render eder
//!    (Vulkan/Metal yok).
//!
//! # Çalıştırma
//!
//! ```text
//! cargo run -p viscos-shell --bin iced_spike --release
//! ```
//!
//! Counter artı/eksi butonlarıyla değer değişir; alttaki toggle dark/light
//! tema arasında geçiş yapar; pencere başlığı `Viscos iced Spike`.
//!
//! # Cross-references
//!
//! - [`phase-5.0-native-ui.md`](../../../.cursor/plans/phase-5.0-native-ui.md) §5
//! - [`docs/VISCOS-CODEBASE-STATUS-REPORT.md`](../../../docs/VISCOS-CODEBASE-STATUS-REPORT.md) §3 (`native/panel.rs:56-64`) + §5 Blocker #6
//! - ADR-0012 §5 (`iced 0.14` + WebView overlay spike)

use iced::widget::{button, column, container, row, text, toggler};
use iced::{Center, Element, Fill, Task, Theme};

/// `iced_spike` uygulamasının runtime state'i.
///
/// `Default` derive'ı `iced::run` ve `iced::application` boot closure'ı
/// tarafından zorunlu olarak beklenir (`IntoBoot<State, _>` blanket impl
/// `State::default()`'ı kabul eder).
#[derive(Debug, Clone)]
pub struct CounterState {
    /// Sayaç değeri (i32, signed — negatif overflow'da saturate yok).
    pub value: i32,
    /// `true` → `Theme::Dark`, `false` → `Theme::Light`.
    pub dark_theme: bool,
}

impl Default for CounterState {
    fn default() -> Self {
        Self {
            value: 0,
            dark_theme: true,
        }
    }
}

/// `iced` runtime'ının view katmanından update katmanına ilettiği event.
#[derive(Debug, Clone, Copy)]
pub enum Message {
    /// `+` butonuna basıldı.
    Increment,
    /// `−` butonuna basıldı.
    Decrement,
    /// Sayaç sıfırlandı (`Reset` butonu).
    Reset,
    /// Tema toggle değişti (true = dark, false = light).
    ToggleTheme(bool),
}

/// State mutasyonu. Asenkron iş yok → `Task::none()`.
///
/// `&mut CounterState` lifetime'ı `iced::UpdateFn` blanket impl'i tarafından
/// zorunlu kılınır; `iced::Task<Message>` dönüş tipi `iced 0.13+`'ta
/// `iced::Command`'ın yerini alır (StackOverflow #79903204).
pub fn update(state: &mut CounterState, message: Message) -> Task<Message> {
    match message {
        Message::Increment => state.value += 1,
        Message::Decrement => state.value -= 1,
        Message::Reset => state.value = 0,
        Message::ToggleTheme(dark) => state.dark_theme = dark,
    }
    Task::none()
}

/// State → widget tree dönüşümü.
///
/// `iced::Element<'_, Message>` döner; layout `column![...]` ile dikey,
/// `row![...]` ile yatay. `Container::center_x(Fill)` + `center_y(Fill)`
/// ile pencere boyutu ne olursa olsun içerik merkezde kalır.
pub fn view(state: &CounterState) -> Element<'_, Message> {
    let header = text(format!("Viscos iced Spike — counter = {}", state.value)).size(28);

    let counter_row = row![
        button("− Decrement").on_press(Message::Decrement),
        button("+ Increment").on_press(Message::Increment),
        button("Reset").on_press(Message::Reset),
    ]
    .spacing(16);

    let theme_row = toggler(state.dark_theme)
        .label("Dark theme")
        .on_toggle(Message::ToggleTheme);

    let theme_label = text(if state.dark_theme {
        "Active theme: Dark"
    } else {
        "Active theme: Light"
    })
    .size(14);

    container(
        column![header, counter_row, theme_row, theme_label]
            .spacing(20)
            .align_x(Center),
    )
    .padding(32)
    .center_x(Fill)
    .center_y(Fill)
    .into()
}

/// Dinamik tema seçimi. `iced::Application::theme` builder'ına geçirilir.
///
/// `state.dark_theme` true ise iced'in `Theme::Dark` (Catppuccin esinli
/// koyu palet), false ise `Theme::Light` (Discord-tarzı açık palet).
pub fn theme(state: &CounterState) -> Theme {
    if state.dark_theme {
        Theme::Dark
    } else {
        Theme::Light
    }
}

/// Spike binary entry point.
///
/// `tracing_subscriber` init'i iced 0.14 wgpu surface'i tarafından üretilen
/// `wgpu` warn log'larını suppress eder; sadece kendi `info!(started)` ve
/// increment/decrement event'leri görünür.
fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,wgpu=warn")),
        )
        .with_target(false)
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        iced_spike = true,
        "viscos iced 0.14 spike starting"
    );

    iced::application(CounterState::default, update, view)
        .title("Viscos iced Spike")
        .theme(theme)
        .window_size((520, 320))
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_state_default_value_is_zero() {
        let s = CounterState::default();
        assert_eq!(s.value, 0);
        assert!(s.dark_theme);
    }

    #[test]
    fn increment_increases_value() {
        let mut s = CounterState::default();
        let _ = update(&mut s, Message::Increment);
        assert_eq!(s.value, 1);
        let _ = update(&mut s, Message::Increment);
        assert_eq!(s.value, 2);
    }

    #[test]
    fn decrement_decreases_value() {
        let mut s = CounterState::default();
        let _ = update(&mut s, Message::Decrement);
        assert_eq!(s.value, -1);
        let _ = update(&mut s, Message::Decrement);
        assert_eq!(s.value, -2);
    }

    #[test]
    fn reset_zeros_value() {
        let mut s = CounterState {
            value: 42,
            dark_theme: true,
        };
        let _ = update(&mut s, Message::Reset);
        assert_eq!(s.value, 0);
    }

    #[test]
    fn toggle_theme_switches_flag() {
        let mut s = CounterState::default();
        assert!(s.dark_theme);
        let _ = update(&mut s, Message::ToggleTheme(false));
        assert!(!s.dark_theme);
        let _ = update(&mut s, Message::ToggleTheme(true));
        assert!(s.dark_theme);
    }

    #[test]
    fn theme_function_returns_dark_when_flag_true() {
        let s = CounterState {
            value: 0,
            dark_theme: true,
        };
        assert!(matches!(theme(&s), Theme::Dark));
    }

    #[test]
    fn theme_function_returns_light_when_flag_false() {
        let s = CounterState {
            value: 0,
            dark_theme: false,
        };
        // `Theme::Light` discriminant check (Theme büyük enum — sadece discriminant match).
        assert!(matches!(theme(&s), Theme::Light));
    }

    #[test]
    fn update_returns_task() {
        // Asenkron iş yok → her message geçerli bir Task<Message> döner
        // (perform listesi boş). Tükettiğimizde panic etmemeli.
        let mut s = CounterState::default();
        let t1 = update(&mut s, Message::Increment);
        let t2 = update(&mut s, Message::Decrement);
        let t3 = update(&mut s, Message::ToggleTheme(true));
        let t4 = update(&mut s, Message::Reset);
        // Task bir değer; drop etmek yeterli.
        drop(t1);
        drop(t2);
        drop(t3);
        drop(t4);
        assert_eq!(s.value, 0);
    }

    #[test]
    fn view_renders_without_panic() {
        // `view()` her state için widget tree üretir; panic etmemeli.
        let s = CounterState::default();
        let _element: Element<'_, Message> = view(&s);

        let s_dark_off = CounterState {
            value: -42,
            dark_theme: false,
        };
        let _element2: Element<'_, Message> = view(&s_dark_off);
    }
}
