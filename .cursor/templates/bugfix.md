---
id: bugfix
owner: ai
phase: all
created: 2026-06-18
purpose: Bug fix için AI task şablonu (yeniden üretme adımları + regression test zorunlu)
---

# Bug: <İNSAN'IN AÇIKLAMASI — kısa, tek satır>

## Meta

| Alan | Değer |
|------|-------|
| Faz | `<numara>` |
| Öncelik | `<P0 / P1 / P2 / P3>` |
| Reproducible | `<her zaman / arada bir / bir kere>` |
| Etkilenen crate'ler | `<crate listesi>` |
| İlgili issue | `<#123>` |
| ADR referansları | `<ADR-XXXX>` |
| Tahmini süre | `<saat>` |

## Yeniden Üretme Adımları (Reproduce)

Koşulları netleştir:

- **Platform:** `<Win10 22H2 / Win11 23H2 / RDP / vs.>`
- **Backend:** `<WebView2 / CEF>`
- **Build:** `<tag veya commit sha>`
- **Hesap durumu:** `<login olmuş / login yok / çoklu hesap>`
- **Önceki adımlar:** (varsa gerekli state)

```
1. <Adım 1>
2. <Adım 2>
3. <Adım 3>
   → <Gözlem: panic, deadlock, yanlış output, crash, vs.>
```

## Beklenen Davranış (Expected)

<Olması gereken — kullanıcı gözünden, ideal olarak tek cümle.>

## Gerçek Davranış (Actual)

<Olan — hata mesajı, log çıktısı, screenshot, panic backtrace, performans sayısı.>

## Kök Neden Analizi (Root Cause)

AI agent doldurur:

- **Tespit edilen yer:** `<dosya:satır>`
- **Tespit edilen neden:** <kodun neden yanlış davrandığı, hangi invariant / edge case kaçırıldı>
- **Neden CI yakalamadı:** <mevcut test coverage'ın neresinde boşluk vardı>

## Düzeltme Önerisi (Fix)

- **Strateji:** `<kısa açıklama — ör. "yokmuş gibi davranmak yerine Result döndür, çağıranı zorla">`
- **Etkilenen dosyalar:** `<listele>`
- **Yeni bağımlılık:** `<hayır / evet — gerekçe>`
- **Public API etkisi:** `<yok / breaking / additive — gerekçe>`

## Regression Test (ZORUNLU)

Bu bug bir kez oluştuysa tekrar oluşmayacak test ekle:

- [ ] Unit test ekleyen commit PR'a dahil (failure → bug, fix → pass)
- [ ] Test adı: `<test_xxx_regression_for_issue_123>` veya benzeri açıklayıcı isim
- [ ] Test: `<dosya:yol>` (issue / commit referansı ile)
- [ ] Soak / 24h test gerekli mi? (memory leak / watchdog restart gibi):
  - [ ] Evet — plan eklendi (`docs/AI-WORKFLOW.md` soak runbook)
  - [ ] Hayır — gerekçe

## Kabul Kriterleri (Acceptance)

- [ ] Bug fix'lendi ve **regression test** eklendi
- [ ] `cargo test --workspace` yeşil
- [ ] Manuel reproduce adımları artık expected davranışa götürüyor
- [ ] (Memory leak ise) 24 saat soak test'te tekrar oluşmuyor
- [ ] İlgili crate'in coverage'ı düşmedi
- [ ] Production `unwrap` / `expect` / `println!` / `dbg!` yok
- [ ] Mevcut davranış değişmedi (kapsam dışı regression yok)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean

## Yan Etkiler ve Doğrulama

- **Başka hangi crate'ler bu kodu çağırıyor?** (grep ile bul ve etki analizi yap)
- **Kullanıcı davranışı değişti mi?** (bu bir bugfix olduğu için sadece buggy davranış düzelmeli)
- **Güvenlik / auth etkisi?** (varsa → Bölüm 4 hard limit, insan onayı)
- **Telemetri etkisi?** (yeni event loglanıyor mu?)

## İzleme (Monitoring)

Bug fixed olduktan sonra **24–48 saat telemetry gözlemi** önerilir:

- Restart count
- Error log spike
- IPC error rate
- Crash-free user ratio

## İlgili Sorular

- Bu bug **aynı root cause'dan** kaynaklanan başka yerlerde de olabilir mi? (benzer pattern taraması)
- Bu bug'un **daha büyük bir mimari sorunun** semptomu mu? (örn. memory leak → ADR-0010 tier sizing tekrar değerlendirmesi)

## PR Description Taslağı

```markdown
### Bug Özeti
<tek cümle — issue'ya link>

### Kök Neden
<AI yazar — 1-2 paragraf>

### Düzeltme
<AI yazar — 1-2 paragraf + etkilenen dosyalar listesi>

### Test Kanıtı
- Eski test fail: `<log>` (veya repro screenshot)
- Yeni regression test: `<test adı, dosya yolu>`
- Full `cargo test --workspace`: yeşil

### Doğrulama
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` yeşil
- [ ] `cargo bench` regression yok (kritik path'lerde)
- [ ] Co-authored-by: <insan adı>

### AI-Generated
- [x] yes — Cursor agent
- [ ] no — tamamen insan
```
