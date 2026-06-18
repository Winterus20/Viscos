---
id: crud-entity
owner: ai
phase: all
created: 2026-06-18
purpose: Yeni domain entity (CRUD) eklemek için AI task şablonu (SQLite + serde + UI)
---

# Entity: <İNSAN'IN VERDİĞİ İSİM>

## Meta

| Alan | Değer |
|------|-------|
| Faz | `<numara>` |
| Öncelik | `<yüksek / orta / düşük>` |
| Sahip crate | `<viscos-core / viscos-cache / vs.>` |
| ADR referansları | `<ADR-XXXX — schema / API için>` |
| Tahmini süre | `<gün>` |

## Domain (Bu Ne İçin?)

<Mesaj mı, kanal mı, kullanıcı mı, sunucu üyesi mi, ayar mı, draft mı? Domain bağlamını 1-2 cümleyle anlat.>

**Örnekler:**
- "Discord mesajı — webhook push'la gelen, kullanıcının gönderdiği, edit/history ile güncellenen."
- "Kullanıcı ayarı (per-account) — keyring'de tutulan `SecretValue`."
- "Mesaj taslağı — pre-restart autosave, watchdog hook."

## Alanlar (Fields)

| Field | Tip | Açıklama | Null/Optional | Default |
|-------|-----|----------|---------------|---------|
| `<field1>` | `<u64 / String / enum / struct>` | <...> | `<no / yes — neden>` | `<Default::default() veya somut>` |
| `<field2>` | `<...>` | <...> | <...> | <...> |

**Serde:** `#[derive(Serialize, Deserialize)]` (JSON + binary farklı format gerekirse ek feature'lar).
**Default:** Çoğu entity `Default` impl almalı; "yok" semantiği `Option<T>` ile ifade edilir.
**Validation:** Hangi alan hangi invariant'a sahip? (örn. `snowflake > 0`, `name non-empty`, `url max 2048 char`)

## Davranışlar (Methods / Constructors)

- `new(...)` — constructor: hangi alanlar required, hangi alanlar default?
- `<method 1>` — <ne yapar, hangi hata durumları>
- `<method 2>` — <ne yapar>
- `<as_ref / as_str>` — dönüşüm helper'ları (gerekiyorsa)

**Hata dönüş tipleri:** `thiserror` ile typed enum (ADR-0007), `#[non_exhaustive]`. `anyhow` YOK kütüphane katmanında.

## SQLite Şema Değişikliği

`crates/viscos-cache/src/schema/` veya refinery migration dizinine yeni dosya:

```sql
-- migrations/V00X__add_<entity>.sql
CREATE TABLE IF NOT EXISTS <entity> (
    id          INTEGER PRIMARY KEY,        -- veya snowflake BIGINT
    <field1>    <TYPE> NOT NULL,
    <field2>    <TYPE>,
    created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    updated_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_<entity>_<field1>
    ON <entity>(<field1>);
```

**Refinery kuralları:**
- Yeni migration dosyası **geriye dönük uyumlu** olmalı (eski sürümler yeni DB'yi açabilsin veya migration atlanabilsin)
- Down migration **sadece yeni feature, geri alma riski varsa açıkça belgelenir**
- WAL mode aktif (viscos-cache zaten)
- Index: hangi sorgu için, hangi field üzerinde, ne kadar cardinality

**Migration testi:** `cargo test -p viscos-cache` yeni migration'ı yükleyip eski DB fixture'ı ile uyumluluğu doğrular.

## API Endpoint'leri / IPC Contract

### Yeni / değişen command'lar

```rust
// crates/viscos-ipc/src/commands.rs
pub enum IpcCommand {
    // ... mevcut variant'lar ...
    CreateEntity(EntityInput),       // → Result<Entity>
    GetEntity(EntityId),             // → Result<Option<Entity>>
    UpdateEntity(EntityId, Patch),   // → Result<Entity>
    DeleteEntity(EntityId),          // → Result<()>
    ListEntities(ListFilter),        // → Result<Vec<Entity>>
}
```

### Yeni / değişen event'ler (push)

```rust
// crates/viscos-ipc/src/events.rs
pub enum IpcEvent {
    // ... mevcut variant'lar ...
    EntityCreated(Entity),
    EntityUpdated(Entity),
    EntityDeleted(EntityId),
}
```

**Push/policy:** Büyük blob → SharedBuffer (Faz 4+). Bu entity küçükse inline OK.

## Frontend / UI Etkisi

- [ ] `frontend/src/bridge.ts` — yeni tip, `viscos.invoke` wrapper'ı
- [ ] iced UI'da yeni component mi? → Faz 5 kapsamı, ayrı PR
- [ ] Tray / notification etkisi var mı?
- [ ] Hotkey tetikliyor mu? → Faz 6

## Bağımlılıklar

- Yeni external dep? → İnsan onayı (`.cursorrules` Bölüm 4)
- Mevcut crate'lere dokunma: `<crate listesi — bunlardan sadece core + cache + ipc>` dışı

## Kabul Kriterleri (Acceptance)

- [ ] `viscos-core`'a (veya ilgili crate'e) tip eklendi
- [ ] `#[derive(Serialize, Deserialize)]` ve gerekirse `Debug, Clone, PartialEq, Eq, Hash`
- [ ] `Default` impl (gerekiyorsa — empty/list vs. flag-based)
- [ ] **Yeni unit testler:** constructor, getter'lar, validation, edge case (invalid input → Err)
- [ ] `#[non_exhaustive]` enum'larda
- [ ] **rustdoc her public alan + method'da** (`.cursorrules` Bölüm 1)
- [ ] **SQLite migration** oluşturuldu (geriye dönük uyumlu)
- [ ] **Migration testi** (`cargo test -p viscos-cache` yeşil)
- [ ] **IPC command/event** eklendi, serde derive
- [ ] `frontend/src/bridge.ts` yeni tipi yansıtıyor, `pnpm tsc --noEmit` clean
- [ ] `cargo test --workspace` yeşil
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] Coverage >%80 (yeni dosya için)
- [ ] Production `unwrap` / `expect` / `println!` / `dbg!` yok

## Davranış Spec (Test ile Kanıtlanır)

- **Create:** aynı `id` ile iki kez çağrı → ikinci hata döner (`AlreadyExists`) veya upsert policy
- **Get:** var olmayan `id` → `Ok(None)` veya `Err(NotFound)` (tutarlı)
- **Update:** olmayan `id` → hata
- **Delete:** olmayan `id` → idempotent (no-op) veya hata
- **List:** filtre → doğru sonuç, pagination → sınır davranışı
- **Concurrent:** iki task aynı `id` ile create → race condition test

## Performans Notları

- Index'lenmiş alanlar (yukarıdaki CREATE INDEX)
- Büyük listeleme: pagination (limit + cursor) zorunlu, OFFSET YASAK
- Cache stratejisi: hot path'te moka (RAM), cold path SQLite
- Watchdog etkisi: bu entity GDI / IPC buffer'a dokunuyor mu? (Hayır olmalı)

## PR Description Taslağı

```markdown
### Entity: <İsim>

### Schema
<CREATE TABLE — diff olarak>

### IPC Contract
- Command: `CreateEntity`, `GetEntity`, `UpdateEntity`, `DeleteEntity`, `ListEntities`
- Event: `EntityCreated`, `EntityUpdated`, `EntityDeleted`

### Test Kanıtı
- Yeni unit test sayısı: <N>
- Migration testi: <yes/no>
- `cargo test --workspace`: yeşil
- Frontend: `pnpm tsc --noEmit` clean

### Doğrulama
- [ ] rustdoc her public API'de
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` yeşil
- [ ] Coverage >%80
- [ ] Co-authored-by: <insan adı>

### AI-Generated
- [x] yes — Cursor agent
- [ ] no — tamamen insan
```

## Notlar

- Bu template **domain entity** (CRUD) içindir. Davranış değişikliği olan refactor → `refactor.md`. Bug fix → `bugfix.md`. Yeni feature (entity dışı iş akışı) → `feature-add.md`.
- Şema tasarımı **mimari karar gerektirebilir** → insan onayı (Bölüm 4 hard limit, özellikle encryption-at-rest, multi-account, GDPR/PII).
- ADR değişikliği tetikleme: yeni bir pattern (örn. event sourcing, soft delete, audit log) → `adr-new` komutu.
