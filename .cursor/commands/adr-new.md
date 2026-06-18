# /adr-new — Yeni ADR Şablonu Tetikleme

> **Cursor slash command stub'u.** `/adr-new` komutu çağrıldığında Cursor bu prompt'u agent'a gönderir.
> **Kaynak:** `docs/DECISIONS.md` (mevcut ADR'ler), `.cursor/plans/viscos_index.md` Bölüm 4 (ADR değişikliği tetikleme).

## Kullanım

```
/adr-new <başlık>
```

Örnek: `/adr-new Keyring 4.0 migrasyonu` veya `/adr-new CEF default backend kararı`

## Agent'a Gönderilen Prompt

Aşağıdaki prompt'u olduğu gibi (veya bu şablona uygun şekilde) kullan:

---

Sen **Viscos AI-Workflow Worker**sın. **Yeni bir ADR (Architecture Decision Record)** oluşturuyorsun.

> ⚠️ **Önemli:** AI agent ADR **yazabilir** (taslak), ancak **Status `🟡 Proposed`** kalır. **`✅ Accepted` yapamaz** — bu insan onayı gerektirir (master index Bölüm 6.1, `.cursorrules` Bölüm 4 hard limit).

## Adım 1: Mevcut ADR'leri oku

```
docs/DECISIONS.md
```

Tüm mevcut ADR'leri (ADR-0001 → son) hızla gözden geçir. Yeni ADR:

- Mevcut ADR'lerle **çelişmemeli** (çelişiyorsa, yeni ADR eskiyi **Supersede** eder veya yeniden yazılır)
- Benzer pattern'leri **referans alabilir** (Context veya Consequences'da)
- Gözden geçirme tetikleyicileri **somut ve ölçülebilir** olmalı

## Adım 2: Numaralandırma

Mevcut en yüksek ADR numarasını bul (örn. ADR-0012). Yeni numara = mevcut + 1. **AI agent numarayı taslakta `ADR-NNNN` placeholder olarak bırakır**; insan onayında atanır (master index 6.2).

> **Yeni ADR numarası `<N>` = en yüksek + 1**

## Adım 3: ADR Taslağı Oluştur

`docs/DECISIONS.md` dosyasının **en sonuna** aşağıdaki şablonu kullanarak yeni bölüm ekle:

```markdown
---

## ADR-NNNN: <Başlık>

- **Tarih:** YYYY-MM-DD (taslak tarihi)
- **Durum:** 🟡 Proposed (insan onayı bekliyor)
- **Faz:** X.Y
- **Önceki plan:** <referans — varsa>
- **Araştırma dokümanı:** <referans — varsa>

### Context

<Neden karar gerekiyor? Problem, kuvvetler, kısıtlar. Somut ve kanıta dayalı.>

**Güçler:**
- <Güç 1 — ör. "Mevcut crate X, 6 aydır güncelleme almadı">
- <Güç 2>

**Kısıtlar:**
- <Kısıt 1 — ör. "25 MB binary bütçesi, MSRV 1.89">
- <Kısıt 2>

**Neden şimdi:**
- <Somut tetikleyici — ör. "Faz 4 plan dosyası bu kararı bekliyor">

### Decision

<Alınan karar, somut ve net. "X'i Y şekilde yapacağız" cümlesi.>

```toml
# Gerekirse Cargo.toml veya başka config snippet
```

**Kararın kapsamı:**
- ✅ <Kapsam dahili 1>
- ✅ <Kapsam dahili 2>
- ❌ <Kapsam dışı 1 — bu karar bunu kapsamaz>

### Consequences

**Olumlu:**
- <Fayda 1 — ölçülebilir, ör. "Compile time %30 azalır">
- <Fayda 2>

**Olumsuz / Kabul edilen riskler:**
- <Risk 1 — kabul edilen gerekçe>
- <Risk 2>

**Alternatifler neden seçilmedi:**
- <Alternatif A> — <seçilmeme gerekçesi>
- <Alternatif B> — <seçilmeme gerekçesi>

**Gözden geçirme tetikleyicileri:**
- <Tetikleyici 1 — ör. "X sürümü 1.0 çıkarsa major bump değerlendirmesi">
- <Tetikleyici 2>

---

## Revizyon Geçmişi (ADR-NNNN)

| Tarih | Revizyon | Gerekçe |
|-------|----------|---------|
| YYYY-MM-DD | İlk taslak | <gerekçe> |
```

## Adım 4: PR Aç

1. Branch: `docs/adr-NNNN-<kısa-ad>` (örn. `docs/adr-0013-keyring-4-migration`)
2. **Tek dosya değişikliği:** sadece `docs/DECISIONS.md` (yeni bölüm eklendi)
3. **PR title:** `docs(decisions): ADR-NNNN <başlık> (Proposed)`
4. **PR description:**
   - Bağlam (bu ADR neden gerekli)
   - Trade-off özeti
   - Karar verilmesi beklenen kişi / rol
   - Migration planı (kabul edilirse)
5. **Etiketler:** `AI-Generated`, `needs-human-decision`, `area:docs`, `priority:<high|med|low>`
6. **Co-author:** insan adı
7. **CI:** AI-Validation workflow yeşil olmalı (sadece docs değişikliği, hızlı geçer)

## Adım 5: İnsan Onayı Bekleme

PR açıldıktan sonra:

- İnsan review (mimari, trade-off, alternatifler)
- `Status: 🟡 Proposed` → `✅ Accepted` (veya `❌ Superseded` eski ADR'yse)
- **Numara atama** (insan onayında): `ADR-NNNN: <Başlık>` → `ADR-<gerçek-numara>: <Başlık>`
- Merge → master'a girer
- Sonraki migration PR'ı ayrı açılır (acceptance criteria ile)

## Çıkış Koşulları

- [ ] ADR taslak dosyada (`docs/DECISIONS.md` sonuna eklendi)
- [ ] PR açıldı (`AI-Generated` + `needs-human-decision` label)
- [ ] Co-author insan adı
- [ ] CI yeşil (sadece docs değişikliği)
- [ ] İnsan onayı bekleniyor (Status: 🟡 Proposed)

## Yasak

- ❌ **Status'u kendin `✅ Accepted` yapma** — bu insan onayı gerektirir
- ❌ **Numara atama** — placeholder `ADR-NNNN` bırak, insan atar
- ❌ **Mevcut kabul edilmiş ADR'yi sessizce değiştirme** — yeni ADR ile supersede et
- ❌ **Karar olmadan doküman yazma** — önce spike (`.cursor/templates/spike.md`) ile kanıt topla, sonra ADR

---

## Notlar

- Bu stub **Cursor slash command** olarak tasarlandı. Cursor'un command discovery mekanizması `.cursor/commands/*.md` dosyalarını otomatik tanır.
- **Şu anki sürüm:** stub — gerçek implementasyon Cursor sürümüne bağlı.
- ADR yazımı **mimari karar gerektirir** → master index Bölüm 4.6 (AI'ın Hard Limitleri) ve Bölüm 6.1.
- ADR değişikliği için tipik akış: yeni pattern → spike → ADR draft → insan approve → migration PR. Bu stub sadece **ADR draft** aşamasını tetikler; spike ve migration ayrı adımlardır.
