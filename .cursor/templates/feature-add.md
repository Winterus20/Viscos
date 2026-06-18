---
id: feature-add
owner: ai
phase: all
created: 2026-06-18
purpose: Yeni feature eklemek için AI task şablonu
---

# Feature: <İNSAN'IN VERDİĞİ İSİM>

## Meta

| Alan | Değer |
|------|-------|
| Faz | `<numara — ör. 4.0>` |
| Öncelik | `<yüksek / orta / düşük>` |
| Sahip | `<insan adı>` |
| AI agent | `<cursor / claude / codey>` |
| Faz dosyası | `.cursor/plans/phase-X.Y-*.md` |
| Bağımlılıklar | `<crate listesi>` |
| ADR referansları | `<ADR-XXXX, ADR-YYYY>` |
| Tahmini PR sayısı | `<N>` |
| Tahmini süre | `<gün>` |

## Amaç

<Bu feature ne yapıyor? Kullanıcı gözünden tek cümle.>

## Kapsam (Scope)

- ✅ **Yapılacaklar:** (maddeli, her biri implementasyonun atomik parçası)
- ❌ **Out of Scope** (bu feature'ın parçası DEĞİL — bu görev kapsamında dokunma):
  - `<konu 1>`
  - `<konu 2>`

## Mimari Karar Gerektiriyor mu?

- [ ] **Evet — <karar açıklaması>:** (İNSAN yanıtlar, gerekirse yeni ADR draft'ı tetiklenir → `.cursor/templates/spike.md` veya `adr-new` komutu)
- [ ] **Hayır — AI serbest**

## ADR / Doküman Bağlantıları

- İlgili ADR: `docs/DECISIONS.md#adr-XXXX`
- İlgili plan: `.cursor/plans/phase-X.Y-*.md` (satır/anchor)
- Bridge / contract: `crates/viscos-ipc/src/{commands,events}.rs`
- Frontend karşılığı: `frontend/src/bridge.ts` (yeni invoke type ekleniyor mu?)

## Kabul Kriterleri (Acceptance Criteria)

Test edilebilir, otomatik doğrulanabilir her kriter ayrı madde:

- [ ] <Kriter 1 — örn. "yeni `SendMessage` command'ı `crates/viscos-ipc/src/commands.rs`'a eklendi">
- [ ] <Kriter 2 — örn. "Rust tarafı unit test'leri yeşil, coverage >%80">
- [ ] <Kriter 3 — örn. "Frontend `bridge.ts` yeni tipi yansıtıyor, `pnpm tsc --noEmit` clean">
- [ ] <Kriter 4 — örn. "Kullanıcı gözünden manuel acceptance adımı">
- [ ] <Kriter 5 — örn. "dokümantasyon güncel: rustdoc + bu PR description'ın `## Docs` bölümü">

## Test Planı

### Unit / Integration

- [ ] `cargo test -p viscos-<crate>` yeşil
- [ ] Yeni public API'ler için en az 1 unit test
- [ ] Edge case'ler: `<boş input, büyük payload, hata yolu, paralel erişim>`

### Manuel Acceptance

- [ ] `<Senaryo 1: kullanıcı adım adım>`
- [ ] `<Senaryo 2: negatif test>`
- [ ] Cross-platform: Win10 / Win11 ayrı test edildi (gerekiyorsa)

### Performance / Regression

- [ ] `cargo bench` değişiklik öncesi/sonrası çalıştırıldı, regression yok
- [ ] RAM / binary / cold start ölçümleri PR description'da (kritik path'lerde)

## Bağımlılıklar ve Etki

| Etkilenen crate | Değişiklik türü | Yorum |
|-----------------|-----------------|-------|
| `<crate>` | `<ekle / değiştir / sil>` | <...> |
| `<crate>` | `<ekle / değiştir / sil>` | <...> |

**Yeni external dependency?** → İnsan onayı zorunlu (Bölüm 4, `.cursorrules`). Yeni dep varsa burada ayrıca listele ve ADR draft'ı tetikle.

## Riskler ve Kabul Edilen Trade-off'lar

- <Risk 1 — kabul edilen gerekçe>
- <Risk 2 — kabul edilen gerekçe>

## Açık Sorular (PR öncesi İNSAN ile netleşmeli)

- <Soru 1>
- <Soru 2>

## PR Description Taslağı (Self-Review)

PR açmadan önce doldur:

```markdown
### Ne Değişti?
<AI yazar, 3-6 madde>

### Neden?
<İnsan yazar, AI destek verebilir>

### Mimari Karar Değişikliği
- [ ] Public API eklendi / değişti
- [ ] Yeni dependency
- [ ] DB schema değişikliği
- [ ] IPC command/event eklendi
- [ ] WebView backend davranışı değişti

### Test Kanıtı
- `cargo test --workspace` çıktısı: <link veya yapıştır>
- Manuel acceptance: <senaryo>

### Doğrulama
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` yeşil
- [ ] Production `unwrap` / `expect` / `println!` / `dbg!` yok
- [ ] Eklenen dosyalar <400 satır
- [ ] rustdoc her yeni public API'de var
- [ ] Co-authored-by: <insan adı>

### AI-Generated
- [x] yes — Cursor agent
- [ ] no — tamamen insan
```

## Notlar

- Bu template bir **AI task'ıdır**, insan PR review'ı ayrıca yapılır.
- Bu template'i doldururken **faz dosyası kapsamını aşma**; "Out of Scope" listesi kutsal.
- "Bunu da ekleyelim" tuzağına düşmeden önce yeni feature template'i aç.
