# /checkpoint — Faz Sonu Checklist'i

> **Cursor slash command stub'u.** `/checkpoint` komutu çağrıldığında Cursor bu prompt'u agent'a gönderir.
> **Kaynak:** `.cursor/plans/viscos_index.md` Bölüm 2 (Faz sırası), `.cursor/plans/phase-X.Y-*.md` (Kabul Kriterleri).

## Kullanım

```
/checkpoint <X.Y>
```

Örnek: `/checkpoint 1.0`

## Agent'a Gönderilen Prompt

Aşağıdaki prompt'u olduğu gibi (veya bu şablona uygun şekilde) kullan:

---

Sen **Viscos AI-Workflow Worker**sın. **Faz `<X.Y>`** tamamlandı. Faz sonu **checkpoint** doğrulaması yapıyorsun.

## Adım 1: Faz dosyasını oku

```
.cursor/plans/phase-X.Y-<kısa-ad>.md
```

Faz dosyasının **Kabul Kriterleri (Definition of Done)** bölümünü özellikle oku. Her kutucuğu teker teker doğrula.

## Adım 2: Definition of Done Doğrulaması

Her madde için **kanıt topla**:

- [ ] **Tüm PR'lar merge edildi** (PR listesi issue tracker'dan)
- [ ] **Tüm testler yeşil** (`cargo test --workspace`)
- [ ] **Lint clean** (`cargo clippy --workspace --all-targets -- -D warnings`)
- [ ] **Format clean** (`cargo fmt --all -- --check`)
- [ ] **Coverage hedefi karşılandı** (`cargo llvm-cov` ile >%80 her crate)
- [ ] **rustdoc her public API'de** (`cargo doc --workspace --no-deps` clean)
- [ ] **ADR'ler güncel** (yeni karar varsa ADR eklendi, kabul edildi)
- [ ] **Faz plan dosyası güncel** (varsa değişiklikler plan'a yansıdı)
- [ ] **Dokümanlar güncel** (README, CHANGELOG, docs/)

## Adım 3: Metrikleri özetle

`.cursor/plans/viscos_index.md` Bölüm 10'daki **AI Workflow Metrikleri**'ni hesapla:

- AI-PR insan review süresi (ortalama)
- AI-PR merge oranı
- AI kod bug rate (varsa telemetry)
- AI-PR redo oranı
- Coverage ortalaması
- Clippy warning sayısı (0 olmalı)

## Adım 4: Bilinen Sorunlar ve Spike'lar

- Açık issue'lar
- Devam eden spike'lar (master'a merge edilmemiş, branch'te)
- Varsa yeni ADR taslakları (🟡 Proposed durumda)
- Teknik borç notları (master index Bölüm 6'daki değişiklik gerekçeleri)

## Adım 5: Karar Noktaları (İnsan Onayı)

Faz dosyasında listelenen **karar noktaları**'nı özetle. Her biri için:

- Karar verildi mi?
- Karar veren (insan adı)
- Karar tarihi
- İlgili ADR

Eğer karar verilmemişse → checkpoint **bloklanmış**, insan onayı bekle.

## Adım 6: Sonraki Faz Geçişi

Faz dosyasının **"Çıkış → Faz X+1"** bölümünü kontrol et. Sonraki faza geçiş koşulları:

- [ ] Tüm Definition of Done kutucukları yeşil
- [ ] Karar noktaları insan onaylı
- [ ] Açık spike / ADR / issue yok (veya kabul edilen istisna var)
- [ ] Sonraki faz plan dosyası var ve okunabilir

## Çıkış Raporu

Aşağıdaki formatta özet PR veya issue olarak sun:

```markdown
# Faz <X.Y> Checkpoint Raporu

## Definition of Done
- ✅ <Madde 1>
- ✅ <Madde 2>
- ⚠️ <Madde 3 — gerekçe>
- ❌ <Madde 4 — blokaj, neden>

## Metrikler
<yukarıdaki tablo>

## Karar Noktaları
- ✅ <Karar 1 — insan, tarih, ADR>
- 🟡 <Karar 2 — bekliyor>

## Açık Sorunlar
- <issue #N>
- <spike branch adı>

## Sonraki Faz
- Faz <X+1>.0 → <.cursor/plans/phase-X+1.0-*.md>
- Geçiş için blokaj: <yok / var — neden>
```

## Yasak

- ❌ Definition of Done kutucukları boşken checkpoint'i yeşil işaretleme
- ❌ Karar noktalarını insan onayı olmadan kapatma
- ❌ Sonraki faza blokaj varken geçme

---

## Notlar

- Bu stub **Cursor slash command** olarak tasarlandı. Cursor'un command discovery mekanizması `.cursor/commands/*.md` dosyalarını otomatik tanır.
- **Şu anki sürüm:** stub — gerçek implementasyon Cursor sürümüne bağlı.
- Her faz sonunda **insan review** checkpoint raporunu onaylar. Bu, AI'ın bir sonraki faza geçmeden önceki son kontrol noktasıdır.
