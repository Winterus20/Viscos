# /start-phase — Faz Başlatma Prompt'u

> **Cursor slash command stub'u.** `/start-phase` komutu çağrıldığında Cursor bu prompt'u agent'a gönderir.
> **Kaynak:** `.cursor/plans/viscos_index.md` Bölüm 2 (Faz Yol Haritası), `.cursor/plans/phase-X.Y-*.md`.

## Kullanım

```
/start-phase <numara>
```

Örnek: `/start-phase 1.0`

## Agent'a Gönderilen Prompt

Aşağıdaki prompt'u olduğu gibi (veya bu şablona uygun şekilde) kullan:

---

Sen **Viscos AI-Workflow Worker**sın. **Faz `<X.Y>`** için çalışmaya başlıyorsun.

## Adım 1: Faz dosyasını oku

```
.cursor/plans/phase-X.Y-<kısa-ad>.md
```

Faz dosyasını baştan sona oku. Aşağıdaki bölümleri özellikle özümse:

- **Amaç ve kapsam** — bu fazda ne yapılacak
- **Out of Scope** — bu fazda YAPILMAYACAK şeyler
- **Karar noktaları** — insan onayı gereken yerler
- **Kabul kriterleri** — Definition of Done
- **Sonraki faz** — çıkış koşulu

## Adım 2: Bağlam dosyalarını oku

- **Master index:** `.cursor/plans/viscos_index.md` (Bölüm 3, 6, 7)
- **AI Workflow:** `docs/AI-WORKFLOW.md` (kurallar, PR yaşam döngüsü, etiketler)
- **Kurallar:** `.cursorrules` (Rust standartları, scope disiplini, hard limit'ler)
- **İlgili ADR'ler:** `docs/DECISIONS.md` (fazda referans verilen tüm ADR'ler)

## Adım 3: Çalışma planını çıkar

Faz dosyasındaki **TODO listesi**'ni al ve `.cursor/templates/feature-add.md` (veya uygun template) ile ilk AI task'ı oluştur. Her task için:

- [ ] Tahmini kapsam (kaç PR, hangi crate'ler)
- [ ] Bağımlılıklar (hangi ADR'ler, hangi faz planları)
- [ ] İnsan onayı gereken noktalar
- [ ] Acceptance criteria (test edilebilir)
- [ ] Out of Scope (kutsal liste)

## Adım 4: Branch ve issue oluştur

İlk task için:

1. `git checkout -b <type>/<scope>-<kısa>` (örn. `feat/shell-window-init`)
2. GitHub issue aç ve label'la (`phase-X.Y`, `area:<crate>`, `priority:<...>`)
3. AI task template'i (`.cursor/templates/feature-add.md`) doldur ve issue'ya yapıştır
4. İnsana "Bu task'a başlıyorum" onayı iste (mimari karar gerektiriyorsa)

## Adım 5: Implement + test + self-review

`.cursorrules` Bölüm 9'deki self-review checklist'i ile paralel:

- Kod yaz → unit test yaz → rustdoc ekle → `cargo fmt` + `cargo clippy` + `cargo test` clean
- PR description'ı `.github/PULL_REQUEST_TEMPLATE.md`'a göre doldur
- AI-Generated label + Co-authored-by insan adı

## Adım 6: İnsan review'ı bekle

PR'ı aç, `phase-X.Y` + `AI-Generated` label'larını ekle, **AI Task Validation** CI'ın yeşil olmasını bekle. İnsan review geldikten sonra comment'leri address et, ikinci tur review'a sok.

## Çıkış

Faz dosyasındaki **Definition of Done** tüm kutucuklar yeşil olunca, `/checkpoint <X.Y>` komutunu çağır.

## Yasak

- ❌ Faz dışı dosyaya dokunma
- ❌ İnsan onayı olmadan mimari karar
- ❌ Faz dosyasında "Out of Scope" yazan şeyleri yapma
- ❌ PR'ı `--no-verify` ile skip etme
- ❌ Büyük refactor > 500 satır (parçala)

---

## Notlar

- Bu stub **Cursor slash command** olarak tasarlandı. Cursor'un command discovery mekanizması `.cursor/commands/*.md` dosyalarını otomatik tanır (Cursor sürümüne göre).
- **Şu anki sürüm:** stub — gerçek implementasyon Cursor sürümüne bağlı. Kullanıcı `/start-phase <X.Y>` çağırdığında agent bu dosyanın içeriğini alıp yukarıdaki adımları izler.
- Cursor'un command discovery mekanizması değişirse bu dosya güncellenmeli; ilgili doküman: [Cursor Commands docs](https://cursor.com/docs).
