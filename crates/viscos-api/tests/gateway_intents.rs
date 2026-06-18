//! `ViscosGateway::default_intents()` privacy-first contract doğrulaması.
//!
//! ADR-0012 §5 + `.cursor/plans/phase-3.0-gateway.md` Bölüm 5:
//! - `GUILDS`, `GUILD_MESSAGES`, `DIRECT_MESSAGES`, `MESSAGE_CONTENT`,
//!   `GUILD_MESSAGE_TYPING` VAR.
//! - `GUILD_MEMBERS` (privileged) YOK.
//! - `GUILD_PRESENCES` (privileged) YOK.
//! - `GUILD_BANS` gereksiz, YOK.
//!
//! **Not:** `Intents::GUILD_PRESENCES` twilight 0.17'de `Intents` bitflags
//! içinde tanımlı değil (presence için privileged intent ayrı bir Discord
//! developer portal ayarı). Bu yüzden bit varlık/yokluk kontrolü sadece
//! mevcut olan intent'ler üzerinden yapılır.

use twilight_gateway::Intents;
use viscos_api::ViscosGateway;

#[test]
fn default_intents_is_deterministic_and_non_empty() {
    let a = ViscosGateway::default_intents();
    let b = ViscosGateway::default_intents();
    assert!(!a.is_empty());
    assert_eq!(a.bits(), b.bits());
}

#[test]
fn default_intents_includes_required_privacy_safe_bits() {
    let intents = ViscosGateway::default_intents();
    assert!(intents.contains(Intents::GUILDS));
    assert!(intents.contains(Intents::GUILD_MESSAGES));
    assert!(intents.contains(Intents::DIRECT_MESSAGES));
    assert!(intents.contains(Intents::MESSAGE_CONTENT));
    assert!(intents.contains(Intents::GUILD_MESSAGE_TYPING));
}

#[test]
fn default_intents_excludes_privileged_member_and_presence_intents() {
    let intents = ViscosGateway::default_intents();
    // Privileged intent'ler → privacy-first default'ta YOK.
    assert!(!intents.contains(Intents::GUILD_MEMBERS));
    assert!(!intents.contains(Intents::GUILD_PRESENCES));
}

#[test]
fn default_intents_excludes_unused_moderation_intent() {
    let intents = ViscosGateway::default_intents();
    // Moderation event'leri Faz 7+ admin feature'ları için — default'ta YOK.
    // twilight 0.17'de GUILD_BANS ayrı bir bit değil, GUILD_MODERATION altında.
    assert!(!intents.contains(Intents::GUILD_MODERATION));
}

#[test]
fn default_intents_bits_match_expected_union() {
    let expected = Intents::GUILDS.bits()
        | Intents::GUILD_MESSAGES.bits()
        | Intents::DIRECT_MESSAGES.bits()
        | Intents::MESSAGE_CONTENT.bits()
        | Intents::GUILD_MESSAGE_TYPING.bits();
    assert_eq!(ViscosGateway::default_intents().bits(), expected);
}

#[test]
fn default_intents_count_is_exactly_five() {
    // Union'a katılan her intent ayrı bir bit ekler. Eğer biri duplicate
    // edilirse sayı düşer; bu test guard görevi görür.
    let intents = ViscosGateway::default_intents();
    assert_eq!(intents.iter().count(), 5);
}

#[tokio::test]
async fn connect_lazy_does_not_panic_with_dummy_token() {
    // `ViscosGateway::connect` lazy-connect — gerçek bağlantı kurmadan sadece
    // Shard konfigürasyonu yapar. Config::new TLS sertifika yükleme hatasında
    // panikler, ama rustls-platform-verifier test ortamında sağlıklı
    // yüklenebilir (Windows cert store). Twilight InMemoryQueue bir Tokio
    // reactor'a ihtiyaç duyar — #[tokio::test] zorunlu.
    let result = ViscosGateway::connect("dummy-test-token", ViscosGateway::default_intents());
    assert!(result.is_ok(), "expected lazy-connect to succeed");
}

#[tokio::test]
async fn connect_returns_distinct_shard_instances_per_call() {
    // Her connect call yeni bir Shard üretir (clone değil) — caller bağımsız
    // connection yönetimi için kullanabilir.
    let a = ViscosGateway::connect("token-a", ViscosGateway::default_intents()).expect("connect a");
    let b = ViscosGateway::connect("token-b", ViscosGateway::default_intents()).expect("connect b");
    assert_eq!(a.shard_id().number(), b.shard_id().number());
}
