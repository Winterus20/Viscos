---
id: spike
owner: ai
phase: all
created: 2026-06-18
purpose: Mimari spike / araştırma görevi için AI task şablonu (karar öncesi kanıt toplama)
---

# Spike: <ARAŞTIRMA KONUSU — kısa, tek satır>

## Meta

| Alan | Değer |
|------|-------|
| Faz | `<numara>` |
| Araştırma türü | `<poc / benchmark / literature / comparative>` |
| Öncelik | `<yüksek / orta / düşük>` |
| Sahip | `<insan adı>` |
| ADR tetikleme potansiyeli | `<evet / hayır / bilinmiyor>` |
| Tahmini süre | `<saat / gün>` |
| Zaman kutusu | `<time-box: ör. 1 hafta, sonra "yapmadık" denilebilir>` |

## Araştırma Sorusu

**Tek cümle ile:** `<Hangi kararı vermemiz gerekiyor ve bu kararın önemi ne?>`

**Alt sorular:**

1. <Alt soru 1 — ör. "Stretto moka'dan gerçek workload'da %15+ hit ratio farkı sağlıyor mu?">
2. <Alt soru 2>
3. <Alt soru 3>

## Arka Plan ve Motivasyon

<Bu spike neden şimdi? Hangi karar yolda, hangi risk / fırsat tetikliyor?>

- İlgili ADR: `<ADR-XXXX — varsa>`
- İlgili plan: `.cursor/plans/phase-X.Y-*.md`
- İlgili issue: `<#123>`
- Mevcut durum: <...>

## Denenecekler (Yapılacak İş)

**Time-box kapsamında** yapılacak somut işler:

- [ ] `<Madde 1 — ör. "cachebench OLTP trace'i ile Stretto vs moka benchmark, 16 concurrent client">`
- [ ] `<Madde 2 — ör. "scan-heavy trace'de davranış">`
- [ ] `<Madde 3 — ör. "PoC: viscos-cache'in Stretto backend'i ile build">`
- [ ] `<Madde 4 — ör. "binary boyutu ve cold start ölçümü">`

**Yapılmayacaklar** (scope creep önleme):

- ❌ <Bu spike kapsamında değil>
- ❌ <Bu spike kapsamında değil>

## Değerlendirme Matrisi

Spike sonunda her seçenek için **somut kanıt** toplanır:

| Kriter | Ağırlık | Seçenek A | Seçenek B | Seçenek C |
|--------|---------|-----------|-----------|-----------|
| **Performans** (somut sayı) | yüksek | <X> | <Y> | <Z> |
| **API ergonomics** (1-5) | orta | <puan> | <puan> | <puan> |
| **Bakım maliyeti** (aktif commit / yıl) | orta | <X> | <Y> | <Z> |
| **Lisans** (GPL-3.0 uyumlu mu?) | yüksek | <yes/no> | <yes/no> | <yes/no> |
| **Binary etkisi** (+MB) | orta | <+X> | <+Y> | <+Z> |
| **Compile time etkisi** (+sn) | düşük | <+X> | <+Y> | <+Z> |
| **AI-yazar uyumluluğu** (örnek kod var mı) | orta | <yes/no> | <yes/no> | <yes/no> |
| **Production kanıtı** (who uses it) | yüksek | <refs> | <refs> | <refs> |
| **Toplam skor** | — | <X> | <Y> | <Z> |

> **Skor nasıl hesaplanır:** Ağırlık × (1-5 puan veya somut metrik) toplamı. Skor tek başına karar verdirmez — kanıt + uzun vadeli risk ile birlikte değerlendirilir.

## Karar Önerisi (Spike Sonunda)

AI agent doldurur, **insan onayı** ile kesinleşir:

### Önerilen: <Seçenek X>

**Gerekçe:**

1. <Somut kanıt 1>
2. <Somut kanıt 2>
3. <Somut kanıt 3>

**Trade-off'lar (kabul edilen):**

- <Trade-off 1 — neden kabul ediyoruz>
- <Trade-off 2>

**Sonraki adımlar:**

- [ ] ADR draft'ı (`adr-new` komutu) → 🟡 Proposed
- [ ] İnsan review + onay → ✅ Accepted
- [ ] Migration PR planı (Acceptance criteria ile)
- [ ] Faz plan dosyasına ekleme / güncelleme (`.cursor/plans/phase-X.Y-*.md`)

### Reddedilen: <Seçenek Y / Z>

**Gerekçe:** <somut kanıt veya eksik kanıt — "trade-off kabul edilemez">

## Çıktılar (Spike Deliverables)

Spike sonunda üretilen artefaktlar:

- [ ] Benchmark raporu (CSV / grafik / sayılar)
- [ ] PoC kodu (branch + tag, master'a merge yok)
- [ ] Gözlem / anekdot notları (`docs/research/<konu>.md` veya PR description)
- [ ] ADR draft taslağı (kabul edilirse)
- [ ] Faz plan güncelleme önerisi (kabul edilirse)

## Zaman Kutusu ve "Hayır" Hakkı

**Time-box:** `<ör. 1 hafta / 3 gün / 5 gün>`

Süre dolunca:

- **Sonuç pozitif** → ADR draft'ı tetikle, insan review'a sun.
- **Sonuç negatif** → "Yapmadık, gerekçe: <...>" notu ile kapat. Faz plan dosyasına kısa not düş.
- **Sonuç belirsiz** → İnsan ile ikinci time-box kararı.

**"Hayır" demenin maliyeti düşük** — spike pozitif sonuç verse bile, sonradan mimari olarak vazgeçilebilir. Bu yüzden spike'lar **düşük riskli keşif aracıdır**.

## Riskler

- **Spike → production drift:** Spike PoC'si master'a merge edilir, ama önerilen yaklaşım kabul edilmezse PoC stale olur. Çözüm: spike branch'i izole tut, merge etme.
- **Scope creep:** Spike sırasında "fırsat bu, şunu da ekleyelim" tuzağı. Çözüm: time-box dışına çıkan her şeyi ayrı spike / görev olarak kaydet.
- **Confirmation bias:** AI'ın aradığı kanıtı bulma eğilimi. Çözüm: negatif sonuçları da raporla, karşıt senaryoları da dene.

## PR (Spike Raporu) Description Taslağı

```markdown
### Spike Özeti
<tek cümle — ne araştırıldı, hangi karar için>

### Yöntem
<benchmark / PoC / literatür taraması — kısa>

### Bulgular
<değerlendirme matrisi — tablo>

### Karar Önerisi
<Seçenek X — gerekçe>

### Sonraki Adımlar
- [ ] ADR draft'ı (kabul edilirse)
- [ ] Faz plan güncellemesi
- [ ] Migration PR planı

### Doğrulama
- [ ] `cargo fmt --all -- --check` clean (PoC kodu varsa)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo test --workspace` yeşil
- [ ] Co-authored-by: <insan adı>

### AI-Generated
- [x] yes — Cursor agent
- [ ] no — tamamen insan
```

## Notlar

- Bu template **karar öncesi kanıt toplama** içindir. Karar verilmiş ve uygulamaya geçiliyorsa → `feature-add.md` veya `refactor.md`.
- Spike **kanıt toplar, karar vermez** — kesin karar insan onayı ile olur.
- Spike çıktısı **gözlem ve öneri**'dir, **production kodu** değil. PoC kodu spike branch'inde kalır, master'a merge edilmeden insan onayı bekler.
