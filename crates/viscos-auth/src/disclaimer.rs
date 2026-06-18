//! ToS Disclaimer canonical metin (ADR-0011 §7).
//!
//! **Kural:** Bu metin **4 yerde** birebir aynı olmalı —
//!
//! 1. `docs/DECISIONS.md` ADR-0011 Consequences bölümünde
//! 2. `README.md` → Disclaimer
//! 3. Viscos ilk açılışta modal (Faz 1+)
//! 4. Settings → About (kalıcı)
//!
//! Bu sabit, kod review'da "literal match" kontrolü yapılabilir.

/// ToS disclaimer — kısa versiyon (modal, settings).
pub const TOS_DISCLAIMER: &str = "Viscos, Discord'un RESMİ OLMAYAN bir istemcisidir. \
Kullanıcı kendi hesabıyla giriş yapar; ToS ihlali (otomasyon, scraping, mass DM) \
bu istemcinin tasarım amacı değildir ve tüm sorumluluk kullanıcıya aittir. \
Discord multi-layered detection (fingerprint + behavioral heuristics) ile \
self-bot tespit edip banlayabilir.";

/// ToS disclaimer — uzun versiyon (README, ADR). Kısa versiyonun tüm
/// noktalarını içerir + ek detay.
pub const TOS_DISCLAIMER_LONG: &str = "Viscos, Discord'un RESMİ OLMAYAN bir istemcisidir. \
Kullanıcı kendi hesabıyla giriş yapar; ToS ihlali (otomasyon, scraping, mass DM) \
bu istemcinin tasarım amacı değildir ve tüm sorumluluk kullanıcıya aittir. \
Discord multi-layered detection (fingerprint + behavioral heuristics) ile \
self-bot tespit edip banlayabilir.\n\n\
Viscos, user-token ile resmi rate-limit bucket kullanır (bot-token değil); \
anatomy'si: native shell + WebView2/Discord Web embed. Token saklama: \
keyring-core (DPAPI arkası, Windows).";

/// Modal başlığı (Faz 1+'ta `iced::Text` ile render).
pub const TOS_DISCLAIMER_TITLE: &str = "Viscos — Üçüncü Parti Discord İstemcisi";

/// Modal CTA metni.
pub const TOS_DISCLAIMER_ACCEPT_LABEL: &str = "Anladım, devam et";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_disclaimer_contains_key_phrases() {
        // "Code review checklist": disclaimer canonical metin içermeli.
        assert!(TOS_DISCLAIMER.contains("RESMİ OLMAYAN"));
        assert!(TOS_DISCLAIMER.contains("self-bot"));
        assert!(TOS_DISCLAIMER.contains("kullanıcıya aittir"));
    }

    #[test]
    fn long_disclaimer_extends_short() {
        assert!(
            TOS_DISCLAIMER_LONG.starts_with(TOS_DISCLAIMER),
            "long disclaimer must begin with short disclaimer text"
        );
        assert!(TOS_DISCLAIMER_LONG.contains("keyring-core"));
    }

    #[test]
    fn accept_label_is_actionable() {
        assert!(TOS_DISCLAIMER_ACCEPT_LABEL.contains("devam"));
    }

    #[test]
    fn title_identifies_app() {
        assert!(TOS_DISCLAIMER_TITLE.contains("Viscos"));
    }
}
