# Viscos — AI Workflow

> **Bu projede AI ajanlar X yapar, insanlar Y yapar.**
> **Referans:** [`.cursor/plans/viscos_index.md` Bölüm 4](../.cursor/plans/viscos_index.md) (AI-Yazar, İnsan-Onay İş Akışı) · [`.cursorrules`](../.cursorrules) (kural seti) · [`.cursor/plans/phase-0.5-ai-workflow-setup.md`](../.cursor/plans/phase-0.5-ai-workflow-setup.md) (bu dokümanın kurulum fazı)
>
> **Motto:** *"AI yazar, insan karar verir, AI yazar."* İnsan **"ne"** ve **"neden"** sorusunu sorar, AI **"nasıl"**ı yapar.

---

## 1. Felsefe

Viscos **4 yıllık, tek kişilik geliştirici + AI ajan** projesidir. Kodun **%100'ü AI (Cursor agent)** tarafından yazılır; insan yalnızca:

1. **Mimari kararlar** (backend, modül, dependency)
2. **Kod review** (her PR'da mimari tutarlılık, edge case, ADR uyumu)
3. **Acceptance test** (lokal build, gerçek hesap, soak test)
4. **Proje yönü** (önceliklendirme, release zamanlaması, etik sınırlar)

Bu ayrım **karar matrisini** netleştirir, sorumluluk çakışmasını önler ve 4 yıllık bir projede AI'ın **tutarlı, denetlenebilir** kod üretmesini sağlar.

---

## 2. Kim Ne Yapar?

### 2.1 AI Ajanların Yaptığı (ve yetkili olduğu) İşler

- ✅ **Kod yazma** — feature implementation, bugfix, refactor
- ✅ **Test yazma** — unit, integration, doctest, regression
- ✅ **Dokümantasyon** — rustdoc, README taslağı, ADR draft'ı (taslak), spike raporu
- ✅ **Boilerplate** — CRUD, migration, configuration
- ✅ **Refactoring** — mevcut pattern'i koru, kod kalitesini yükselt
- ✅ **Benchmark yazımı** — criterion, dhat, custom
- ✅ **Kod review (ilk tur)** — lint, format, basit bug yakalama
- ✅ **Issue triage** — şablon doldurma, kapsam daraltma, spike çıkarma

### 2.2 İnsanın Yaptığı (ve yetkili olduğu) İşler

- 🔵 **Mimari karar** — yeni crate, modül sınırı, dependency seçimi (ADR ile)
- 🔵 **ADR yazma** — Context, Decision, Consequences, Status (AI taslak önerebilir)
- 🔵 **Güvenlik review** — auth, encryption, ToS, key handling
- 🔵 **Telemetri kapsamı** — yeni toplanan alan, üçüncü partiye giden veri
- 🔵 **Trade-off değerlendirmesi** — performans vs okunabilirlik, RAM vs disk, hız vs güvenlik
- 🔵 **Önceliklendirme** — hangi feature önce, hangi bug kritik
- 🔵 **Dış iletişim** — issue yanıtı, release notes, topluluk yönetimi
- 🔵 **Son release kararı** — tagging, signing, publishing (Faz 8.0)
- 🔵 **Auto-merge kategorisi dışındaki her PR review** — her şeyin üstü, insan review zorunlu

### 2.3 Karar Matrisi (Özet)

| Karar Türü | İnsan | AI |
|------------|:-----:|:--:|
| Mimari (backend, modül, yeni crate) | ✅ | ❌ (öneri) |
| Dependency seçimi (versiyon) | ✅ onay | ✅ öneri |
| Public API shape | ✅ onay | ✅ öneri |
| Implementation detayı | ❌ | ✅ |
| Test stratejisi | ❌ | ✅ |
| Bug fix | ❌ | ✅ (insan review) |
| Trade-off kararı | ✅ | ❌ |
| Önceliklendirme | ✅ | ❌ |
| Release zamanlaması | ✅ | ❌ |
| Etik sınır (ToS) | ✅ | ❌ |
| Mimari refactor | ✅ onay | ✅ öneri |

---

## 3. PR Yaşam Döngüsü

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. İNSAN: Issue açar + label'lar (priority, area, faz)         │
│    ↓                                                            │
│ 2. İNSAN: AI task template seçer (.cursor/templates/*.md)      │
│    (feature-add, bugfix, refactor, crud-entity, spike)          │
│    ↓                                                            │
│ 3. AI: Branch açar, kodu okur, ilgili ADR'leri referans alır    │
│    ↓                                                            │
│ 4. AI: Implement + test + rustdoc + self-review checklist       │
│    ↓                                                            │
│ 5. AI: PR açar — .github/PULL_REQUEST_TEMPLATE.md doldurur     │
│    (AI-Generated kutusu, Co-authored-by, ADR referansları)      │
│    ↓                                                            │
│ 6. AI VALIDATION (CI):                                           │
│    - Pre-check: AI-Generated label var mı                       │
│    - cargo fmt --check, clippy, test                            │
│    - eval_script payload audit, unwrap/println scan             │
│    - Dosya boyutu sınırı (>600 satır fail)                      │
│    - Conventional Commits mesajları                             │
│    ↓                                                            │
│ 7. İNSAN: PR review (mimari, UX, edge case, acceptance)         │
│    ↓                                                            │
│ 8. AI: Review comment'lerine göre düzeltme                      │
│    ↓                                                            │
│ 9. İNSAN: Onay + merge                                          │
└─────────────────────────────────────────────────────────────────┘
```

### 3.1 Auto-Merge Kategorileri (Önerilen)

İnsan review olmadan auto-merge **YAPMA**. Aşağıdaki kategoriler insan review'ı kısa tutabilir (30 dk altı):

- 📝 docs typo fix (markdown lint clean)
- 📦 dependency minor version bump (CI geçerse, ADR-0010/0011 minor bump politikasına uygun)
- 📚 rustdoc iyileştirmesi (kod değişikliği yok, sadece yorum)
- 🧪 yeni test (mevcut kodu değiştirmez)

Bunun dışındaki her şey **tam insan review** ister.

> **Not:** Auto-merge kategorileri bile **CI yeşil + AI-Generated label** gerektirir. Branch protection'da "AI Task Validation" **required status check** olarak ayarlanmalıdır.

---

## 4. Etiket Kuralları

### 4.1 Zorunlu Etiketler

| Etiket | Zorunluluk | Anlam |
|--------|-----------|-------|
| `AI-Generated` | AI-PR ise ZORUNLU | PR'da AI katkısı var (label yoksa AI-Validation CI fail eder) |
| `phase-X.Y` | Önerilir | PR hangi fazda çalışıyor (örn. `phase-4.0`, `phase-1.6`) |
| `breaking` | Breaking change varsa ZORUNLU | Public API / config / IPC contract kırılıyor |
| `needs-human-decision` | Mimari karar gerektiriyorsa ZORUNLU | `.cursorrules` Bölüm 4 hard limit'lerden biri tetiklendi |

### 4.2 Önerilen Etiketler

| Etiket | Anlam |
|--------|-------|
| `area:<crate>` | Etkilenen crate (örn. `area:auth`, `area:webview`) |
| `priority:<high|med|low>` | Aciliyet |
| `security` | Güvenlik / auth etkisi |
| `do-not-merge` | WIP — squash/rebase sırasında unutulmasın |
| `ai:auto-merge-eligible` | Auto-merge kategorisinde (insan review kısa) |
| `ai:spike` | Sadece spike (master'a merge yok, branch'te kalır) |

### 4.3 Etiket Atama Kuralları

- AI agent **kendi PR'ı için etiket atayabilir** (auto-label, GitHub Action ile opsiyonel)
- İnsan **override** edebilir (acil durum, çakışma, vb.)
- `breaking` + `needs-human-decision` aynı PR'da olabilir
- Etiket kaldırılırsa PR description'da gerekçe yazılmalı

---

## 5. AI Hard Limitleri (`.cursorrules` ile senkron)

AI ajan **asla** aşağıdakileri insan onayı olmadan yapamaz:

- ❌ **Breaking API değişikliği** (insan onayı olmadan)
- ❌ **Yeni dependency** (`Cargo.toml`'a yeni crate)
- ❌ **Mimari karar** (sadece öneri)
- ❌ **ToS ihlali şüphesi** (Discord user-client sınırları)
- ❌ **Release publish** (sadece PR hazırlar)
- ❌ **Code signing** (private key'e erişemez)
- ❌ **Production secret** (env var veya hardcode)
- ❌ **Büyük refactor > 500 satır** (parçalamalı)
- ❌ **Yeni crate oluşturma** (mimari karar gerektirir)
- ❌ **Faz dışı scope dokunuşu** (`.cursor/plans/phase-X.Y-*.md` dışı)

### 5.1 CI Red Flag'leri (Otomatik Fail)

| Durum | Tespit | Aksiyon |
|-------|--------|---------|
| `eval_script` payload > 10KB | payload size check | Pull-based pattern ihlali → fail |
| `unsafe { }` (dokümante edilmemiş) | `cargo geiger` + code review | İnsan review zorla |
| Public API breaking change | `cargo semver-checks` | Major bump gerekli |
| Yeni dependency | `Cargo.toml` diff + PR review | İnsan onayı zorunlu |
| DB schema change | `migrations/` diff | Migration + rollback testi |
| `unwrap()` in production | AI-Validate CI scan | Test'te OK, runtime'da yasak |
| `println!` / `dbg!` | AI-Validate CI scan | tracing kullan |
| **Dosya > 600 satır** | AI-Validate CI scan | ZORUNLU refactor |
| `todo!()` / `unimplemented!()` | code review | Issue açılmadan merge yok |
| GDI watchdog bypass | grep + code review | İnsan review zorunlu |
| **Bilinen güvenlik açığı (transitive dep)** | `cargo audit` (haftalık + PR) | Justifiye `ignore` + issue linki yoksa fail |
| **Lisans uyumsuzluğu** | `cargo deny check licenses` | GPL/AGPL/LGPL default deny |
| **Güvenilmeyen kaynak** | `cargo deny check sources` | `crates.io` dışı registry fail |
| **Yasaklı crate** | `cargo deny check bans` | Banned crate eklenirse fail |
| **Binary bütçesi aşımı** | CI size job (25 MB) | Hedef metrikleri korumak için fail |

> **Tam CI workflow:** [`.github/workflows/ai-task-validate.yml`](../.github/workflows/ai-task-validate.yml)
> **Kural seti (davranışsal, AI'ın yorumladığı):** [`.cursorrules`](../.cursorrules)
> **Auto red flag listesi (master index):** [`viscos_index.md` Bölüm 4.7](../.cursor/plans/viscos_index.md)

---

## 6. ADR Değişikliği Nasıl Tetiklenir

Yeni bir pattern, mimari karar veya dış bağımlılık gerektiğinde:

```
┌────────────────────────────────────────────────────────────────┐
│ Yeni pattern / karar ihtiyacı fark edildi                       │
│   ↓                                                            │
│ 1. SPIKE → .cursor/templates/spike.md                          │
│    (Time-box: 1 hf. Kanıt toplama, karar VERME.)               │
│   ↓                                                            │
│ 2. ADR DRAFT → .cursor/commands/adr-new.md                     │
│    (Context, Decision, Consequences, Status: 🟡 Proposed)      │
│   ↓                                                            │
│ 3. İNSAN REVIEW                                                │
│    - Mimari uygunluk                                           │
│    - Trade-off kabul edilebilir mi                              │
│    - Alternatifler gözden geçirildi mi                          │
│    - Gözden geçirme tetikleyicileri                             │
│   ↓                                                            │
│ 4. STATUS: ✅ Accepted                                          │
│   ↓                                                            │
│ 5. MIGRATION PR                                                │
│    - Acceptance criteria                                       │
│    - Faz plan dosyası güncelleme                               │
│    - Code review + CI                                          │
└────────────────────────────────────────────────────────────────┘
```

### 6.1 ADR'yi Kim Yazabilir?

- **AI agent ADR draft önerebilir** (taslak), ancak **Status `🟡 Proposed`** kalır.
- **İnsan onayı olmadan** ADR `✅ Accepted` olamaz.
- **AI agent kabul edilmiş ADR'yi ihlal eden PR açamaz** — review'da otomatik reddedilir.

### 6.2 ADR Numarası Atama

- Mevcut numaralama korunur: ADR-0001, ADR-0002, ... ADR-0012 (Haziran 2026)
- Yeni ADR → bir sonraki boş numara (örn. ADR-0013)
- AI agent **numara atamaz**, taslakta `ADR-NNNN: <Başlık>` placeholder bırakır; insan onayında atanır.

---

## 7. Faz Sonu Karar Noktaları (İnsan Onayı)

Her faz sonunda insan ile kritik karar noktaları:

| Faz | Karar Noktası | Tetikleyici |
|-----|---------------|-------------|
| 0.0 sonu | Clippy seviyesi, default features | Foundation worker output |
| 0.5 sonu | AI-PR auto-merge kategorileri, PR review SLA | Bu doküman |
| 1.0 sonu | GDI threshold (7000/9000), auto-restart agresifliği | Watchdog telemetry |
| 1.5 sonu | Pointermove throttle (kanıtlanmış etkisiz → CEF kararı tetikleyici) | Mouse telemetry |
| 1.6 sonu | Win11 CEF default rollout koşulu | GDI restart trend |
| 2.0 sonu | Token storage encryption anahtarı (DPAPI vs passphrase) | ADR-0011 (Varyant A/B) |
| 3.0 sonu | Default intent'ler | Gateway telemetry |
| 4.0 sonu | Jemalloc geçiş (benchmark sonucuna göre) | RSS benchmark |
| 5.0 sonu | Side panel native vs WebView, Vencord API yüzeyi | UX test |
| 8.0 sonu | Faz 8.5 CEF aktif mi (auto-update, signing) | Release kanıtı |
| 8.5 sonu | CEF default-out yönetim (Faz 1.6 + ADR-0012 ile uyumlu) | Rollback gerekli mi |

> **Bu kararlar `_insan/` klasörü veya issue tracker'da toplanır; AI erişemez.**

---

## 8. AI-PR Metrikleri (Hedef)

| Metrik | Hedef | Ölçüm |
|--------|-------|-------|
| AI-PR insan review süresi | < 30 dk ortalama | PR review log |
| AI-PR merge oranı | > %70 | Merged / Opened |
| AI kod bug rate | < production ortalamasının 1.5× | Telemetry crash-free ratio |
| AI-PR redo oranı | < %20 | "Changes requested" / "Merged" |
| İnsan coding süresi | < 5 saat/hafta | Manuel time tracking |
| Coverage | > %80 her crate | `cargo llvm-cov` |
| Clippy warning | 0 (CI fail) | CI log |
| Memory regression | < %5 release-to-release | Telemetry |

> Bu metrikler **aylık** gözden geçirilir; sapma varsa workflow kuralları güncellenir (yeni ADR).

---

## 9. Sık Yapılan Hatalar ve Önleme

| Hata | Önleme |
|------|--------|
| AI scope creep — "fırsat bu, şunu da ekleyelim" | Out of Scope listesi kutsal; yeni feature template'i aç |
| AI ADR ihlali — kabul edilmiş pattern'i kırma | CI'da bilinen pattern tarama (Faz 1+) + code review |
| İnsan yorgunluk — her PR'ı tam review etmeme | Auto-merge kategorileri sıkı tutulur; güvenlik/auth PR'ları her zaman tam review |
| AI üretim kodu `unwrap`/`println` sızıntısı | AI-Validation CI tarama + code review checklist |
| Commit mesajı informal | Conventional Commits zorunlu (CI warning + review'da düzeltme) |
| Co-author unutulması | PR template'te zorunlu alan, commit hook ile kontrol (Faz 0.0+) |
| Büyük refactor PR > 500 satır | CI ve `.cursorrules` uyarısı; parçala zorunluluğu |

---

## 10. Referanslar

| Amaç | Yol |
|------|-----|
| **AI kuralları** | [`.cursorrules`](../.cursorrules) |
| **Master index (AI/human ayrımı)** | [`.cursor/plans/viscos_index.md` Bölüm 4](../.cursor/plans/viscos_index.md) |
| **Faz 0.5 playbook (kaynak)** | [`.cursor/plans/phase-0.5-ai-workflow-setup.md`](../.cursor/plans/phase-0.5-ai-workflow-setup.md) |
| **AI task template'leri** | [`.cursor/templates/*.md`](../.cursor/templates/) |
| **Cursor slash command'ları** | [`.cursor/commands/*.md`](../cursor/commands/) |
| **PR şablonu** | [`.github/PULL_REQUEST_TEMPLATE.md`](../.github/PULL_REQUEST_TEMPLATE.md) |
| **CI workflow** | [`.github/workflows/ai-task-validate.yml`](../.github/workflows/ai-task-validate.yml) |
| **ADR listesi** | [`docs/DECISIONS.md`](./DECISIONS.md) |
| **CEF vs WebView2** | [`docs/CEF-VS-WEBVIEW2.md`](./CEF-VS-WEBVIEW2.md) |

---

## 11. Changelog

| Tarih | Değişiklik | Yazar |
|-------|-----------|-------|
| 2026-06-18 | İlk yayın (Faz 0.5 deliverable) | AI-Workflow Worker (Faz 0.5) |
