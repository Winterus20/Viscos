---
id: refactor
owner: ai
phase: all
created: 2026-06-18
purpose: Refactor için AI task şablonu (davranış değişmeyecek garantisi zorunlu)
---

# Refactor: <İNSAN'IN AÇIKLAMASI — kısa, tek satır>

## Meta

| Alan | Değer |
|------|-------|
| Faz | `<numara>` |
| Öncelik | `<yüksek / orta / düşük>` |
| Tahmini değişiklik | `<satır sayısı>` |
| Bağımlılıklar | `<etkilenen crate listesi>` |
| ADR referansları | `<ADR-XXXX — varsa>` |
| Tahmini süre | `<gün>` |

## Motivasyon (Neden Refactor?)

En az birini seç, somut kanıt ver:

- [ ] **Tekrar eden pattern** → `<örnek dosya/konum, kaç yerde>`
- [ ] **Performans sorunu** → `<benchmark, profil, varsayımsal — somut sayı>`
- [ ] **Okunabilirlik** → `<kod bloğu引用, kaç satır, kaç iç içe>`
- [ ] **Test edilebilirlik** → `<mevcut testlerin neden yetersiz olduğu>`
- [ ] **Tek sorumluluk ihlali (SRP)** → `<modül N sorumluluk taşıyor, ayrıştırma planı>`
- [ ] **Dosya boyutu sınırı** → `<mevcut satır, 400 / 600 eşik durumu>`
- [ ] **Mimari uyumsuzluk** → `<yeni ADR ile çelişen eski kod>`

## Kapsam (Scope)

### ✅ Değişecek dosyalar

- `<dosya:crate/yol/dosya.rs> — neden>`
- `<...>`

### ❌ Değişmeyecek dosyalar (Out of Scope — KUTSAL)

- `<dosya — neden dokunulmuyor>`
- `<...>`

> **Uyarı:** Out of Scope listesi kutsal. Refactor PR'ı bu listeyi genişletirse kapsam creep olur — yeni PR + yeni görev aç.

## Davranış Değişmeyecek Garantisi (ZORUNLU)

Refactor'ın tanımı gereği **kullanıcı gözünden davranış aynı kalmalı**. Bunu kanıtlamak için:

- [ ] **Public API imzası korunuyor** (breaking change YASAK — ayrı PR + ADR ister)
- [ ] **Davranış spec'i korunuyor** — mevcut testler pass etmeli, eklenen testler yoksa yeni feature anlamına gelmez
- [ ] **Performance non-regression** — `cargo bench` önce/sonra karşılaştırma PR description'da
- [ ] **Memory non-regression** — kritik path'lerde allocation count veya byte sayısı
- [ ] **Snapshot / golden file** testleri varsa güncellenmemeli (güncellenirse davranış değişmiş demektir)

### Korunan Davranış Testleri

Mevcut testler refactor sonrası **aynen geçmeli**:

- [ ] Mevcut unit testler: <crate/test/dosya> — pass
- [ ] Mevcut integration testler: <...> — pass
- [ ] Mevcut doctest'ler: <...> — pass
- [ ] Mevcut benchmark: <...> — regression yok

### Davranış Değişikliği Tespit Edilirse

Eğer refactor sırasında **gerçek bir bug** bulunursa veya davranış değişmesi **gerekirse**:

1. Refactor PR'ını durdur
2. Bugfix template'i (`bugfix.md`) ile ayrı PR aç
3. İnsan review ile önceliklendir
4. Refactor PR'ını yeni davranış spec'ine göre tekrar başlat

## Mimari Karar Gerektiriyor mu?

- [ ] **Evet — <karar>:** (örn. yeni pattern, dependency değişikliği, modül sınırı değişikliği) → İNSAN yanıtlar; gerekirse `spike.md` veya yeni ADR draft'ı tetiklenir.
- [ ] **Hayır — AI serbest** (mevcut pattern'i koru)

## Etkilenen API'ler

| API | Tür (public / internal) | Değişiklik (imza / davranış / sil) |
|-----|------------------------|-------------------------------------|
| `<fn / struct / trait>` | `<public / pub(crate) / private>` | `<...>` |

**Breaking change varsa → bu template kullanılamaz**, `feature-add.md` veya ayrı ADR PR'ı aç.

## Metrikler (Before / After)

Refactor'ın faydasını ölçülebilir hale getir:

| Metrik | Before | After | Δ | Ölçüm yöntemi |
|--------|--------|-------|---|----------------|
| Dosya satır sayısı | `<X>` | `<Y>` | `<Δ>` | `wc -l` |
| Cyclomatic complexity (max fn) | `<X>` | `<Y>` | `<Δ>` | `cargo clippy` veya `tokei` |
| Test coverage | `<X%>` | `<Y%>` | `<Δ>` | `cargo llvm-cov` |
| `cargo build --release` süresi | `<X s>` | `<Y s>` | `<Δ>` | CI log |
| `cargo bench` (kritik path) | `<X ns>` | `<Y ns>` | `<Δ>` | criterion |
| Allocation sayısı (hot path) | `<X>` | `<Y>` | `<Δ>` | dhat / heaptrack |

> Boş bırakılırsa, refactor "iyi hissettirdiği için" yapılmış olur — kabul kriteri değil.

## Test Coverage

- [ ] Mevcut testler hala geçerli (yeşil)
- [ ] Yeni internal testler eklendi (gerekiyorsa, refactor'ın kendisi için)
- [ ] `cargo bench` çalıştırıldı, regression yok
- [ ] Coverage düşmedi (`%80` altına inmemeli)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean

## Kabul Kriterleri (Acceptance)

- [ ] Davranış değişmedi (golden test / regression test ile kanıtlandı)
- [ ] Public API imzası korundu
- [ ] Performance non-regression (bench ile)
- [ ] Memory non-regression (kritik path'lerde)
- [ ] Out of Scope dosyalara dokunulmadı (PR diff'te `git diff --stat main...` ile doğrula)
- [ ] PR 500 satırın altında (parçala gerekirse)
- [ ] ADR'ye aykırı mimari karar yok

## PR Description Taslağı

```markdown
### Refactor Özeti
<tek cümle — neden>

### Davranış Değişmedi Kanıtı
- Public API imzası: <aynı>
- Mevcut testler: <N test, hepsi geçiyor>
- Benchmark: <X → Y, Δ>

### Metrikler
<yukarıdaki tablo>

### Etkilenen Dosyalar
- `<dosya>` — <değişiklik özeti>
- `<...>`

### Doğrulama
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` yeşil
- [ ] `cargo bench` regression yok
- [ ] Out of Scope dosyalara dokunulmadı
- [ ] Co-authored-by: <insan adı>

### AI-Generated
- [x] yes — Cursor agent
- [ ] no — tamamen insan
```

## Notlar

- **"Tüm dosyaları refactor et" tarzı scope creep YASAK** — `.cursorrules` Bölüm 3.
- Refactor PR'ı **feature veya bugfix ile karıştırılmaz**. Gerekirse birden fazla PR aç.
- Refactor sonrası **davranış değişikliği fark edilirse**, refactor başarısız — geri al veya bugfix template'iyle ayır.
