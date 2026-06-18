<!--
Bu PR şablonu Viscos AI-Workflow'undan (Faz 0.5) gelir.
AI-PR'lar için zorunlu bölümler:
  - "AI-Generated" onay kutusu
  - "AI agent version/commit" + "AI task template"
  - "Co-authored-by: <insan adı>"
  - "AI-PR Self-Review Checklist" (aşağıda)
İnsan-PR'lar için bu bölümler doldurulmaz veya "no" işaretlenir.
-->

## Bu PR Kim Üretti?

- [ ] **AI (Cursor agent)** — insan tarafından review edildi (aşağıdaki AI bölümlerini doldur)
- [ ] Tamamen insan (contributor)

## AI-Generated ise (ZORUNLU bölümler)

- **AI agent version/commit:** `<ör. cursor-agent 1.2.3 / commit abc1234>`
- **AI task template:** `<feature-add / bugfix / refactor / crud-entity / spike>`
- **AI task dosyası:** `<.cursor/templates/feature-add.md veya issue #>`
- **Co-authored-by:** `<insan-adı> <insan@email>`
- **AI-PR Self-Review Checklist** (aşağıdaki kutucuklar doldurulmuş olmalı)

## Başlık ve Faz Referansı

**Başlık:** `<Conventional Commits — feat(core): kısa özet [AI]>`

**Faz:** `<Faz X.Y — .cursor/plans/phase-X.Y-*.md>`
**İlgili issue:** `<#123>`

## Ne Değişti?

(AI yazar — 3-6 madde, öz ve somut)

- <Değişiklik 1 — dosya:crate/yol, ne yapıldı>
- <Değişiklik 2>
- <Değişiklik 3>

## Neden?

(İnsan yazar — AI destek verebilir, ama gerekçe insan onaylı olmalı)

> <Gerekçe — bu değişiklik neden gerekli, hangi sorunu çözüyor, hangi kararı uyguluyor>

## ADR / Doküman Bağlantıları

- **İlgili ADR:** `<ADR-XXXX — docs/DECISIONS.md#adr-XXXX>`
- **İlgili faz planı:** `<.cursor/plans/phase-X.Y-*.md>`
- **Bridge / IPC contract:** `<crates/viscos-ipc/src/{commands,events}.rs>`
- **Frontend karşılığı:** `<frontend/src/bridge.ts — varsa>`

## Mimari Karar Gerektiren Değişiklikler (Hard Limit — `.cursorrules` Bölüm 4)

Aşağıdakilerden biri işaretliyse **insan onayı merge öncesi zorunludur** ve "Human Decision Required" bölümü doldurulmalıdır:

- [ ] Public API değişikliği (yeni tip / fonksiyon / trait / breaking change)
- [ ] Yeni external dependency (`Cargo.toml` diff)
- [ ] Davranış değişikliği (kullanıcı gözlenebilir davranış değişirse)
- [ ] Telemetri kapsamı değişikliği (yeni toplanan alan, üçüncü partiye gönderilen veri, opt-out default değişikliği)
- [ ] Güvenlik / auth etkisi (token storage, keyring, encryption, IPC auth, ToS)
- [ ] Breaking config değişikliği (schema, migration, default değer)

## Human Decision Required (varsa)

> Bu PR yukarıdaki hard limit'lerden birine dokunuyorsa, **insan review kararı** burada özetlenmeli:

- **Karar veren:** `<insan adı>`
- **Karar tarihi:** `<YYYY-MM-DD>`
- **Gerekçe:** <...>
- **İlgili ADR değişikliği:** `<ADR-XXXX — yeni veya güncellenen>`

## Breaking Changes (varsa)

- [ ] **Bu PR breaking change içeriyor** (public API / IPC contract / config schema / dosya formatı)
- **Migration planı:** <adım adım ne yapılmalı, hangi sürümden itibaren>
- **Changelog etkisi:** <CHANGELOG.md veya release notes — major / minor / patch>
- **Etkilenen tüketiciler:** <hangi crate'ler / UI / build script'ler>

## Test Kanıtı

### Otomatik Testler

**Komut ve sonuç:**

```bash
$ cargo test --workspace
<çıktının son 20-30 satırı — veya "all 234 tests passed" özeti>
```

**Coverage:** `<önceki: X% / sonraki: Y% — crate bazında>`

**Ek test kategorileri:**

- [ ] Unit testler eklendi / güncellendi
- [ ] Integration testler eklendi / güncellendi
- [ ] Doctest'ler güncel
- [ ] Migration testi (DB schema değiştiyse)
- [ ] Regression test (bugfix ise)
- [ ] Soak test (memory leak / watchdog etkisi varsa)

### Manuel Test Adımları

1. <Adım 1>
2. <Adım 2>
3. <Beklenen sonuç>

**Cross-platform:**

- [ ] Windows 10 (build `<numara>`) test edildi
- [ ] Windows 11 (build `<numara>`) test edildi
- [ ] RDP session test edildi (gerekliyse)
- [ ] 24 saatlik soak test (Faz 1+ watchdog / telemetry değişikliklerinde)

### Performance / Regression

- [ ] `cargo bench` çalıştırıldı, regression yok
- [ ] Binary boyutu: `<X MB> (limit: 25 MB)`
- [ ] Cold start: `<X sn>`
- [ ] RAM idle: `<X MB>`

## AI-PR Self-Review Checklist (ZORUNLU — `.cursorrules` Bölüm 11)

**Bu checklist'i AI agent PR'ı açmadan önce kendisi doldurmalı. Boş bırakılan madde merge'i bloklar.**

- [ ] `cargo fmt --all -- --check` **clean** (çıktı yapıştır veya "clean" yaz)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` **clean**
- [ ] `cargo test --workspace` **yeşil** (test çıktısı yukarıda)
- [ ] `cargo nextest run --workspace --retries 2` (alternatif)
- [ ] **Üretim kodunda `unwrap()` / `expect()` / `println!` / `dbg!` / `eprintln!` YOK** (`rg` çıktısı temiz)
- [ ] **Her yeni public API'de rustdoc var** (`.cursorrules` Bölüm 1)
- [ ] **Üretim kodu `thiserror` (lib) + `anyhow` (app) kullanıyor** (ADR-0007)
- [ ] **Async sadece `tokio` granular features ile** (ADR-0002)
- [ ] **Yeni dosya 400 satırın altında** (veya gerekçe + refactor planı PR description'da)
- [ ] **PR değişikliği 500 satırın altında** (veya bölünmüş)
- [ ] **Mevcut davranış değişmedi** (refactor ise — golden test / regression test)
- [ ] **Out of Scope dosyalara dokunulmadı** (`git diff --stat main...` ile doğrulandı)
- [ ] **Commit mesajları Conventional Commits** formatında
- [ ] **Co-authored-by: insan adı** her commit'te var
- [ ] **`AI-Generated` label** PR'a eklendi
- [ ] **Branch adı** `<type>/<scope>-<kısa>` formatında (örn. `feat/auth-totp`)

## Screenshots (UI değişikliği ise)

| Öncesi | Sonrası |
|--------|---------|
| `<screenshot — before>` | `<screenshot — after>` |

> Sadece UI etkisi varsa doldur. Backend-only değişikliklerde boş bırakılabilir.

## CI Doğrulama (PR merge öncesi tüm yeşil olmalı)

- [ ] **AI Task Validation** workflow — `ai-task-validate.yml` — yeşil
- [ ] **CI** workflow — `ci.yml` — yeşil (varsa, Faz 0.0+)
- [ ] **Required status check** ayarlandı (branch protection — Faz 0.0 README notu)

## Reviewer Notları

<AI veya insan — review yapan kişiye özel notlar, endişeler, dikkat edilecek yerler>

---

> **Bu şablon Viscos Faz 0.5 (AI Workflow Setup) kapsamında oluşturuldu.**
> **Referans:** [`docs/AI-WORKFLOW.md`](../../docs/AI-WORKFLOW.md), [`.cursorrules`](../../.cursorrules)
