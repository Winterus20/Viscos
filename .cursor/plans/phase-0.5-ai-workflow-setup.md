---
name: Phase 0.5 — AI Workflow Setup
overview: AI-yazar / insan-onay iş akışının altyapısı. .cursorrules, AI task template'leri, PR template, AI-validation CI workflow'su. Kod henüz yazılmaz, sadece "nasıl yazılacak" kuralları.
isProject: false
todos:
  - id: cursorrules
    content: .cursorrules dosyası (Rust + Viscos-spesifik kurallar)
    status: pending
  - id: task-templates
    content: .cursor/tasks/ altında 4 task template (feature-add, bugfix, refactor, crud-entity)
    status: pending
  - id: pr-template
    content: .github/PULL_REQUEST_TEMPLATE.md (AI-PR şablonu)
    status: pending
  - id: ai-validate-ci
    content: .github/workflows/ai-task-validate.yml (AI-PR CI kontrolü)
    status: pending
  - id: ai-workflow-doc
    content: docs/AI-WORKFLOW.md (tam workflow dokümanı)
    status: pending
  - id: decisions-template
    content: docs/DECISIONS.md (ADR template)
    status: pending
  - id: first-ai-task
    content: İlk deneme AI task'ı: Faz 1 viscos-shell crate iskeleti
    status: pending
---

# Phase 0.5 — AI Workflow Setup

> **Süre:** 3–5 gün
> **Hedef:** AI (Cursor agent) tüm kodu yazacak, insan mimari karar + review + acceptance test yapacak. Bu faz altyapıyı kurar.
> **Referans:** Master `viscos_index.md` Bölüm 4 (AI-Yazar, İnsan-Onay İş Akışı)
> **Sonraki faz:** [`phase-1.0-window-webview.md`](./phase-1.0-window-webview.md)

---

## 1. `.cursorrules` — AI Agent Kuralları

Dosya yolu: `/cursorrules` (workspace root).

```markdown
# Viscos — Cursor Agent Kuralları

## Genel
- Rust 1.80+ Edition 2024
- Tüm public API'lere rustdoc yorum
- "Neden" comment'leri yaz, "ne" yapan comment'leri yazma
- Büyük dosya (>400 satır) yazma, refactor öner
- Public API değişikliği insan onayı ister (PR description'da "API change" bölümü)

## Mimari
- viscos-core: domain types, no I/O
- viscos-api: REST + Gateway, depend on core only
- viscos-cache: storage, depend on core only
- viscos-shell: iced UI, depend on core + ipc
- viscos-webview: WebView2 abstraction (Faz 8.5'te CEF)
- viscos-ipc: pull-based commands/events, depend on core
- viscos-watchdog: cross-cutting, tüm crate'lere bağlı olabilir
- Cross-cutting: tracing, thiserror, figment

## Test
- Her public fonksiyon için en az 1 unit test
- Integration test: crates/viscos-*/tests/
- E2E: ayrı workspace, manual acceptance
- Coverage threshold: %80

## IPC Pattern
- Rust → JS: SADECE küçük olaylar (tray badge, notification) push
- JS → Rust: invoke() ile pull
- eval_script payload > 10KB → otomatik red flag (Faz 1.5'te tool)

## Performance
- Heap allocation hot path'te `Vec::new()` yerine `Vec::with_capacity()`
- String yerine `&str`, `Cow<str>` düşün
- `Arc<Mutex<T>>` sadece gerekli yerlerde; `RwLock` veya channels tercih
- Her PR'da `cargo bench` (kritik path'ler için)

## Yapma
- "Tüm dosyaları refactor et" scope creep yapma
- İstenmeyen feature ekleme
- BÜYÜK PR (>500 satır değişiklik) açma; parçala
- Force push yapma (main'e)
- Skip CI hook (--no-verify) yapma
- Düzeltilmemiş `clippy::pedantic` warning ile merge etme

## Bilinen Trade-off'lar
- WebView2 GDI leak: watchdog (Faz 1) + throttling (Faz 1.5) + CEF escape (Faz 8.5)
- Linux v2'de WebKitGTK sorunlu: pluggable backend mimarisi
- Self-bot ToS riski: mimari bunu çözemez, kullanıcı sorumluluğunda

## AI-PR'ları İçin Ek Kurallar
- PR description'da "AI-Generated: yes" label'ı zorunlu
- Co-author insan adı zorunlu
- Mimari karar gerektiren değişiklikte ADR taslağı ekle (docs/DECISIONS.md)
- Test yazmadan "done" deme
- `unsafe` kodu satır satır dokümante et, güvenlik gerekçesi yaz
```

---

## 2. AI Task Template'leri

Dizin: `.cursor/tasks/`.

### 2.1 `feature-add.md`

```markdown
# Feature: <İNSAN'IN VERDİĞİ İSİM>

## Context
- Faz: <numara>
- Öncelik: <yüksek/orta/düşük>
- Bağımlılıklar: <crate'ler>

## Acceptance Criteria
- <Test edilebilir kriter 1>
- <Test edilebilir kriter 2>

## Mimari Karar Gerektiren mi?
- [ ] Evet — <karar> (İNSAN yanıtlar)
- [ ] Hayır — AI serbest

## Out of Scope
- <Bu feature'ın PARÇASI OLMAYAN şeyler>

## Test Senaryoları
- <Manuel test 1>
- <Manuel test 2>
```

### 2.2 `bugfix.md`

```markdown
# Bug: <İNSAN'IN AÇIKLAMASI>

## Reproduce
<Adımlar>

## Expected
<Olması gereken>

## Actual
<Olan>

## Acceptance Criteria
- [ ] Bug fix'lendi ve regresyon test eklendi
- [ ] 24 saat soak test'te tekrar oluşmuyor (eğer memory leak ise)
```

### 2.3 `refactor.md`

```markdown
# Refactor: <İNSAN'IN AÇIKLAMASI>

## Neden Refactor?
- <Tekrar eden pattern>
- <Performans sorunu>
- <Okunabilirlik>

## Mimari Karar Gerektiren mi?
- [ ] Evet — <karar>
- [ ] Hayır — AI serbest

## Kapsam Sınırı
- <Değişecek dosyalar>
- <Değişmeyecek dosyalar — "out of scope">

## Test Coverage
- [ ] Mevcut testler hala geçerli
- [ ] Yeni testler eklendi (gerekirse)
- [ ] cargo bench çalıştırıldı, regression yok
```

### 2.4 `crud-entity.md`

```markdown
# Entity: <İNSAN'IN VERDİĞİ İSİM>

## Domain
- <Bu ne için? Mesaj mı, kanal mı, kullanıcı mı?>

## Alanlar
- <field1>: <tip> — <açıklama>
- <field2>: <tip> — <açıklama>

## Davranışlar
- <Method 1>: <ne yapar>
- <Method 2>: <ne yapar>

## Acceptance Criteria
- [ ] viscos-core'a type eklendi
- [ ] Serde derive (Serialize, Deserialize)
- [ ] Default impl (gerekiyorsa)
- [ ] Unit test'ler (constructor, getters, validation)
```

---

## 3. PR Template

Dosya: `.github/PULL_REQUEST_TEMPLATE.md`.

```markdown
## Bu PR Kim Üretti?
- [ ] AI (Cursor agent) — insan tarafından review edildi
- [ ] Tamamen insan (contributor)

## AI-Generated ise
- AI agent version/commit:
- AI task template: feature-add / bugfix / refactor / crud-entity
- Co-authored-by: <insan-adı>

## Ne Değişti?
(Buraya AI yazsın, insan kontrol etsin)

## Neden?
(İnsan yazsın, AI destekleyebilir)

## Mimari Karar Gerektiren Değişiklikler
- [ ] Public API değişikliği
- [ ] Yeni dependency
- [ ] Veritabanı schema değişikliği
- [ ] IPC command/event eklendi
- [ ] WebView backend davranışı değişti

## Acceptance Test Senaryoları
- [ ] Manuel test: <senaryo>
- [ ] Cross-platform: Win10 / Win11 test edildi
- [ ] 24 saatlik soak test (Watchdog aktifken crash yok)

## Checklist
- [ ] cargo fmt --check
- [ ] cargo clippy -- -D warnings
- [ ] cargo test (tüm crate'ler)
- [ ] Memory regression yok (önceki release ile karşılaştır)
- [ ] Dokümantasyon güncel (rustdoc + DECISIONS.md)
```

---

## 4. AI-Validation CI

Dosya: `.github/workflows/ai-task-validate.yml`.

```yaml
name: AI Task Validation

on:
  pull_request:
    types: [opened, synchronize, ready_for_review]

jobs:
  ai-validate:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Check AI-Generated label
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          LABELS=$(gh pr view ${{ github.event.pull_request.number }} --json labels -q '.labels[].name')
          if ! echo "$LABELS" | grep -q "AI-Generated"; then
            echo "::error::AI-Generated label zorunlu"
            exit 1
          fi
      
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      
      - name: Cache cargo
        uses: Swatinem/rust-cache@v2
      
      - name: Format check
        run: cargo fmt --all -- --check
      
      - name: Clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
      
      - name: Test
        run: cargo test --workspace --all-features
      
      - name: Eval script payload audit
        run: |
          # tauri#13758: payload > 10KB red flag
          PAYLOADS=$(rg "eval_script|execute_script" --type rust -A 1 -B 1 || true)
          echo "$PAYLOADS" | tee /tmp/eval_audit.txt
          # Manuel review gerekli: her payload boyutunu kontrol et
          echo "::warning::eval_script call'larını manuel review edin, payload >10KB ise refactor"
      
      - name: Unwrap detection
        run: |
          if rg "\.unwrap\(\)" crates/ --type rust -g '!*/tests/*' -g '!*/examples/*'; then
            echo "::error::Production code'da unwrap yasak (test ve example hariç)"
            exit 1
          fi
      
      - name: Println detection
        run: |
          if rg "println!|dbg!|eprintln!" crates/ --type rust -g '!*/tests/*' -g '!*/examples/*'; then
            echo "::error::Production code'da println/dbg yasak, tracing kullanın"
            exit 1
          fi
      
      - name: Large file detection
        run: |
          find crates/ -name '*.rs' -size +400c -exec echo "Büyük dosya: {}" \;
          # Sadece warning, fail değil
```

---

## 5. `docs/AI-WORKFLOW.md`

```markdown
# Viscos — AI Workflow

## Felsefe
"AI yazar, insan karar verir, AI yazar."

## İnsan Sadece Şunlara Karar Verir
1. Mimari kararlar (backend, modül, dependency)
2. Önceliklendirme
3. Trade-off değerlendirmesi
4. Mimari refactor onayı
5. Release zamanlaması
6. Etik sınırlar (ToS)

## AI Sadece Şunları Yapar
1. Kod yazma (feature, bugfix, refactor)
2. Test yazma
3. Dokümantasyon (rustdoc, ADR taslağı)
4. Boilerplate, CRUD, migration
5. Benchmark yazma

## Her Faz İçin Tipik Karar Noktaları
- **Faz 0.0 sonu:** Clippy seviyesi, default features
- **Faz 1 sonu:** GDI threshold, auto-restart agresifliği
- **Faz 1.5 sonu:** Pointermove throttle, throttling başarısızsa CEF kararı
- **Faz 2 sonu:** Token storage encryption anahtarı (DPAPI vs password)
- **Faz 3 sonu:** Default intent'ler
- **Faz 4 sonu:** Jemalloc geçiş (benchmark sonucuna göre)
- **Faz 5 sonu:** Side panel native vs WebView, Vencord API yüzeyi
- **Faz 8 sonu:** Faz 8.5 CEF aktif mi
- **Faz 8.5 sonu:** CEF default mu

## AI Hard Limitleri
- ❌ Breaking API değişikliği
- ❌ Yeni dependency
- ❌ Mimari karar
- ❌ ToS ihlali şüphesi
- ❌ Release publish
- ❌ Code signing
- ❌ Production secret
- ❌ Büyük refactor > 500 satır
```

---

## 6. `docs/DECISIONS.md` (ADR Template)

```markdown
# Architecture Decision Records

## ADR-001: WebView Backend Seçimi (Haziran 2026)

### Context
Discord client için WebView backend seçimi. Seçenekler: WebView2 (wry), CEF, custom Chromium.

### Decision
Varsayılan: WebView2 (wry). Opsiyonel (Faz 8.5): CEF.

### Consequences
- + Hafif (15–25 MB binary)
- + OS WebView, sistem güncellemesi ile güvenlik fix
- − Win11 GDI leak (wry#1691) upstream'te çözümsüz
- − Üç katmanlı savunma gerekiyor (watchdog + throttle + CEF escape)

### Status
Accepted.

### Alternatives Considered
- CEF default: +200 MB binary, leak yok. Disk alanı maliyeti kabul edilmedi.
- Custom Chromium: yıllar sürer, ekip büyütmek gerekir.
- Tauri: abstraction katmanı gereksiz, Leto kanıtladı.

---

## ADR-002: Tauri Kullanımı (HAYIR)

### Context
Leto ve Dorion aynı sonucu veriyor; Tauri sadece abstraction.

### Decision
Doğrudan `tao` + `wry` kullan, Tauri YOK.

### Consequences
- + Daha az dependency, daha küçük binary
- + Daha fazla kontrol (WebView2 lifecycle)
- − Tauri'nin plugin ekosistemi yok
- − Vencord uyumu için kendi bridge'i yazılacak (Faz 5)

### Status
Accepted.

---

## ADR-NNN: <Başlık>

### Context
<Neden karar gerekiyor>

### Decision
<Ne karar verildi>

### Consequences
+ <Pozitif>
− <Negatif>

### Status
<Proposed / Accepted / Deprecated>
```

---

## 7. Kabul Kriterleri (Definition of Done)

- [ ] `.cursorrules` workspace root'ta
- [ ] `.cursor/tasks/` altında 4 template
- [ ] `.github/PULL_REQUEST_TEMPLATE.md` var
- [ ] `.github/workflows/ai-task-validate.yml` CI yeşil (örnek PR ile test)
- [ ] `docs/AI-WORKFLOW.md` tamamlanmış
- [ ] `docs/DECISIONS.md` en az 2 ADR ile başlatılmış (webview backend, Tauri hayır)
- [ ] **İlk AI task denemesi başarılı:** Cursor agent'a verilen küçük bir görev (örn. `viscos-core`'a `pub struct AppContext` ekle) insan review'den geçti.

---

## 8. Karar Noktası (Faz 0.5 Sonu)

> 🔵 **İNSAN:** `.cursorrules`'a hangi kısıtlamalar eklenecek?
> - Daha sıkı mı (her crate için `deny(clippy::all)`, sıkı format)?
> - AI'ın hangi durumlarda önerisi otomatik merge edilebilir (örn. docs typo fix, dependency minor bump)?
> - Hangi durumlar her zaman insan review ister (yeni crate, public API, mimari)?

> 🔵 **İNSAN:** AI-PR auto-merge kategorileri:
> - docs typo (auto-merge OK)
> - dependency minor version bump (auto-merge OK, CI geçerse)
> - rustdoc iyileştirmesi (auto-merge OK)
> - yeni test (auto-merge OK)
> - her şeyin üstü: insan review zorunlu

---

## 9. Çıkış → Faz 1.0

Bu faz tamamlandığında:
- AI workflow kuralları net
- İlk deneme AI task başarılı
- İnsan-AI iş birliği ritmi oturmuş

Faz 1.0 → Pencere + WebView2 + GDI watchdog (kritik faz).
