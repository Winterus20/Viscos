---
name: Bridge Resilience Research (Haziran 2026)
overview: Discord web client'ın DOM/class churn davranışı, Vencord + BetterDiscord + browser-cli kanıtları, Viscos frontend/src/bridge.ts için zorunlu selector hiyerarşisi, Vencord webpack module proxy şablonu, AI-PR review checklist. ADR-0012 §2 + Faz 1.0 deliverable (crates/viscos-webview/BRIDGE-RESILIENCE.md) kaynağı.
isProject: false
---

# Bridge Resilience Research — Discord DOM Churn Stratejisi

> **Kaynak:** [`docs/DECISIONS.md` ADR-0012 §2](../DECISIONS.md), [`phase-1.0-window-webview.md` Bölüm 4.5](phase-1.0-window-webview.md#45-bridge-resilience-rules-adr-0012-2--yeni).
> **İlgili planlar:** Faz 1.0 (deliverable), Faz 5.0 (side panel eklenirken uygulanır), Faz 1.6 (CEF IPC shim).

Bu doküman **Viscos'un `frontend/src/bridge.ts` dosyasının Discord'un sık sık değişen DOM/class yapısına karşı nasıl dayanıklı (resilient) olacağını** kanıta dayalı olarak belgeler. Yeni katkıcılar ve AI agent'lar bu dokümanı okuyup bridge.ts yazarken selector seçimi yapacak.

---

## 1. Problem: Discord Sık Sık DOM Değiştiriyor

Discord mühendis ekibi haftalık (bazen günlük) deploy yapıyor. Her deploy'da:

### 1.1 Webpack-Generated Hashed Class Name'ler

Discord'un React component'leri **webpack** ile bundle edilmiş. Production build'de class name'ler hash'lenir:

```html
<!-- Discord bundle: web.3293dbbf90bafac60d3c.js -->
<article class="message__5126c messageCompact_c19a55">
  <div class="username_c19a55">alice</div>
  <div class="content_f8d9b3">Hello world</div>
</article>
```

**Sorun:** `5126c`, `c19a55`, `f8d9b3` hash'leri **her deploy'da değişir**. Selector `.message__5126c` Discord'un 1 hafta sonraki sürümünde `.message__7a3b1` olur.

**Kanıt:** browser-cli Discord skill dokümantasyonu:
> "Discord uses auto-generated, unstable class names (e.g., `message5126c`). These change between deploys. Prefer `aria-label`, `role`, `id` prefix, and partial class matches (`[class*="username"]`) over exact class names."

### 1.2 Virtual Scrolling

Mesaj listesi sadece **viewport'taki mesajları DOM'da tutar**. Scroll yaptıkça eski mesajlar DOM'dan çıkar, yeniler eklenir. Yan etki:

- "Mention var mı?" gibi DOM observer-based tespit yanlış pozitif/negatif verir.
- Side panel'in "unread count" için DOM scrape etmesi unreliable.

### 1.3 React Hydration + SPA Navigation

Discord tek sayfa uygulama (SPA). React hydration gecikmesi olabilir → DOM var ama data henüz yüklenmemiş.

### 1.4 Cloudflare Bot Detection

Discord **Cloudflare** kullanıyor. headless browser / automation tool'larını tespit edip IP ban atabiliyor (Viscos için risk: bridge.ts içinde suspicious pattern olursa). Kanıt: Scraperly Discord guide (2026 Nisan, 4/5 difficulty).

---

## 2. Çözüm: Selector Hiyerarşisi (Öncelik Sırasıyla)

### 2.1 Hiyerarşi

```text
1. [aria-label] / [role]   ← Değişmez (accessibility contract)
2. [id^="prefix-"]         ← Discord internal ID'ler (mesaj-id, channel-id)
3. viscos.webpack.findByProps  ← Vencord pattern: Discord'un kendi state'ine read-only
4. viscos.invoke (IPC)      ← Native Rust tarafı truth source olur
5. [class*="partial"]      ← Hashed class fallback (sadece class başlangıcı biliniyorsa)
6. .exact-class             ← ❌ KULLANMA (her deploy'da kırılır)
```

### 2.2 Örnek Implementasyonlar

#### ✅ DOĞRU: aria-label + role

```typescript
// frontend/src/bridge.ts

// Kanal listesi öğesi (aria-label Discord tarafından set ediliyor)
const channelItem = document.querySelector('[aria-label*="channel"]');

// Mesaj container (role + id prefix)
const messageItem = document.querySelector('[id^="message-content-"]');

// Ses ayarı slider (role="slider", aria-label)
const volumeSlider = document.querySelector('[role="slider"][aria-label*="volume"]');
```

#### ✅ DOĞRU: Webpack Module Proxy (Vencord pattern)

```typescript
// Discord'un kendi internal state'ine read-only erişim
// Vencord ekibi bu pattern'i production'da kullanıyor (13k+ star)
// Detay: https://github.com/Vendicated/Vencord/blob/main/src/main/patcher.ts

const userStore = viscos.webpack.findByProps('getCurrentUser');
const currentUser = userStore?.getCurrentUser();

const channelStore = viscos.webpack.findByProps('getChannel', 'getDMFromUserId');
const channel = channelStore?.getChannel(channelId);

const unreadStore = viscos.webpack.findByProps('getUnreadCount');
const unreadCount = unreadStore?.getUnreadCount(channelId);
```

#### ✅ DOĞRU: Native IPC Pull (Truth Source)

```typescript
// Side panel her zaman native taraftan çeker, DOM observer'a güvenmez
const unreadCount = await viscos.invoke({
  type: 'GetUnreadCount',
  data: { channel_id: channelId },
});

// Mention list native taraftan
const mentions = await viscos.invoke({
  type: 'GetRecentMentions',
  data: { since_ms: Date.now() - 24 * 60 * 60 * 1000 },
});
```

#### ❌ YANLIŞ: Hashed Class Selector

```typescript
// Bu selector Discord'un sonraki deploy'unda kırılır!
const messageItem = document.querySelector('.message__5126c');
const userName = document.querySelector('.username_c19a55')?.textContent;
```

#### ❌ YANLIŞ: MutationObserver + Heuristic

```typescript
// Bu unreliable — virtual scrolling + Discord re-render'ları false-positive üretir
const observer = new MutationObserver(() => {
  const unreadCount = document.querySelectorAll('[class*="unread"]').length;
  tray.setBadge(unreadCount); // Yanlış değer gösterebilir
});
```

---

## 3. Vencord Webpack Module Proxy (Tam Implementasyon)

Viscos'un `frontend/src/webpack-shim.ts`'i Vencord'un production-tested patch'ini temel alır:

```typescript
// frontend/src/webpack-shim.ts (Faz 1.0 deliverable — preload script'e dahil)

let discordWebpack: any = null;

const originalSetter = Function.prototype.m;

// Function.prototype.m setter'ı override et
// Discord webpack runtime'ı bu setter'ı çağırarak wreq.m = moduleFactories atar
Object.defineProperty(Function.prototype, 'm', {
  set(value) {
    if (typeof value === 'object' && value !== null && !discordWebpack) {
      const stack = new Error().stack || '';
      // Discord'un kendi bundle'ından mı? React DevTools'tan mı? Doğrula.
      if (
        stack.includes('discord.com/assets/') &&
        !stack.includes('react-devtools') &&
        !stack.includes('chrome-extension')
      ) {
        discordWebpack = { m: value, c: (this as any).c };
        originalSetter.call(this, value);
        return;
      }
    }
    originalSetter.call(this, value);
  },
  get() {
    return originalSetter;
  },
  configurable: true,
});

// Public API
window.viscos.webpack = {
  findByProps: (...propNames: string[]) => {
    if (!discordWebpack) return null;
    for (const id in discordWebpack.c) {
      const module = discordWebpack.c[id]?.exports;
      if (module && propNames.every(p => p in module)) return module;
    }
    return null;
  },
  
  findByCode: (...codes: string[]) => {
    if (!discordWebpack) return null;
    for (const id in discordWebpack.c) {
      const source = discordWebpack.c[id]?.toString() || '';
      if (codes.every(code => source.includes(code))) {
        return discordWebpack.c[id]?.exports;
      }
    }
    return null;
  },
  
  waitFor: (filter: (module: any) => boolean, timeoutMs = 5000) => {
    return new Promise((resolve, reject) => {
      const start = Date.now();
      const interval = setInterval(() => {
        if (!discordWebpack) return;
        for (const id in discordWebpack.c) {
          const module = discordWebpack.c[id]?.exports;
          if (module && filter(module)) {
            clearInterval(interval);
            resolve(module);
            return;
          }
        }
        if (Date.now() - start > timeoutMs) {
          clearInterval(interval);
          reject(new Error('waitFor timeout'));
        }
      }, 100);
    });
  },
};
```

**Güvenlik notu:** Bu proxy sadece **read-only** kullanılır. Discord'un internal state'ini mutate etmek ToS ihlali riski taşır + Vencord'un kendisi de bu prensibi korur.

---

## 4. AI-PR Review Checklist

Her `frontend/src/bridge.ts` (veya ilgili `.ts` dosyası) PR'ında code reviewer şunları kontrol eder:

### 4.1 Zorunlu Kontroller (PR reject on violation)

- [ ] Yeni `querySelector` / `querySelectorAll` call'larında `[aria-label]` veya `[role]` attribute selector öncelikli mi?
- [ ] Yeni selector'larda hashed class name var mı? (`\.message_[a-f0-9]+` veya `[a-z]+_[a-f0-9]{5,}` pattern regex'i ile kontrol)
- [ ] MutationObserver + heuristic var mı? (Side panel "unread count" gibi şeyler için — native IPC kullanılmalı)
- [ ] `viscos.webpack.findByProps` ile native tarafa veri çekilebilir mi?
- [ ] Cloudflare bot detection tetikleyebilecek suspicious pattern var mı? (örn. rapid-fire `fetch()` Discord API'ye)

### 4.2 Önerilen Kontroller

- [ ] Selector fallback chain var mı? (örn. önce `[aria-label]`, yoksa `[class*="prefix"]`)
- [ ] `waitFor` kullanılmış mı? (Discord lazy-load modülleri için)
- [ ] WebGL/Canvas fingerprint'inde parity check var mı? (Faz 1.5 telemetry)
- [ ] Cross-reference: [`docs/DECISIONS.md` ADR-0012 §2](../DECISIONS.md) ile uyumlu mu?

---

## 5. Referans Kanıtlar

### 5.1 browser-cli Discord Skill

[github.com/six-ddc/browser-cli](https://github.com/six-ddc/browser-cli/blob/main/skills/browser-cli/references/sites/discord.com.md) — Discord scraping için hazır `discord.mjs` script'i.

**Önemli alıntı:**
> "Auto-generated CSS Classes: Discord uses hashed class names (e.g., `message__5126c`, `username_c19a55`). These change between deploys. Prefer `aria-label`, `role`, `id` prefix, and partial class matches (`[class*="username"]`) over exact class names."

> "Virtual scrolling: Message lists use virtual scrolling. Older messages are removed from the DOM as you scroll down. If collecting a large history, use scroll-and-extract loops."

### 5.2 BetterDiscord Styling Guide

[docs.betterdiscord.app/themes/introduction/environment](https://docs.betterdiscord.app/themes/introduction/environment) ve styling guide — Discord CSS variable'ları ve selector stratejileri.

**Önemli alıntı:**
> "Discord uses webpack-generated class names that include a hash. Always use attribute selectors: `[class*="message_"]`"

### 5.3 Vencord Webpack Integration

[Vendicated/Vencord/blob/main/src/main/patcher.ts](https://github.com/Vendicated/Vencord/blob/main/src/main/patcher.ts) — production'da 13k+ star, Discord'un webpack instance'ına proxy.

**Önemli alıntı (interception logic):**
```typescript
// Vencord Function.prototype.m setter override
const proxiedValue = new Proxy(value, {
    get(target, prop) {
        // Discord'un kendi internal store'una lazy hook
        ...
    },
});
```

### 5.4 Scraperly Discord Guide (Nisan 2026)

Discord scraping **4/5 difficulty** (Hard), Cloudflare bot detection aktif. Residential proxy + Playwright stealth plugin gerekli (Viscos için doğrudan geçerli değil çünkü Viscos kendi kullanıcısı adına authenticated session kullanıyor, ama bridge.ts suspicious pattern'lerden kaçınmalı).

---

## 6. Mart 2025 Discord UI Revamp — Bridge Dayanıklılık Testi

Mart 2025'te Discord büyük UI revamp yaptı (Onyx theme + resizeable channel list + game overlay widgets). Kullanıcıların önemli kısmı memnun değildi ([The Verge](https://www.theverge.com/news/635435/discord-ui-refresh-dark-mode-new-overlay)). **Vencord plugin'leri eski UI'a döndürmek zorunda kaldı.**

Viscos'un bridge.ts'i bu revamp'ta **kırılmamalı** çünkü:

1. **`aria-label` / `role` selector'ları Discord contract'ı** — UI revamp'ında bile korunur (accessibility zorunluluğu).
2. **Vencord webpack proxy** Discord'un React component tree'sinden bağımsız — internal state store revamp'ta da erişilebilir.
3. **Native IPC pull** side panel "unread count" / "mention list" gibi özellikleri Discord DOM'undan tamamen bağımsız yapar.

**Test planı (Faz 1.0 Kabul Kriteri):** Discord'un bir sonraki UI revamp'ından sonra 24 saat içinde bridge.ts güncellemesi gerekmiyorsa dayanıklılık kanıtlanmış sayılır.

---

## 7. Faz 1.0 Deliverable

### 7.1 crates/viscos-webview/BRIDGE-RESILIENCE.md

Bu dokümanın kısa versiyonu (sadece zorunlu kurallar + checklist) bridge.ts'in yanında durur. AI-PR review sırasında bu dosya referans alınır.

### 7.2 frontend/src/webpack-shim.ts

Tam Vencord webpack proxy implementasyonu (Bölüm 3). Preload script'e dahil edilir. `cargo build` ile `dist/preload.js`'e esbuild bundle edilir.

### 7.3 frontend/test/selector-resilience.test.ts

Vitest unit test'leri:

- Hashed class selector regex'i → bridge.ts source'unda bulunursa test fail.
- `aria-label` / `role` selector yokluğu → bridge.ts export'larında kontrol, fail.
- Webpack proxy fonksiyonları `findByProps`, `findByCode`, `waitFor` mock ile test.

---

## 8. Gelecek: Discord Chat Input / Virtual Scroll Bridge (Faz 5)

Faz 5 (Native UI + Vencord) ile birlikte side panel büyüyecek:
- Channel list tree (kategoriler + kanallar)
- Member list (online/offline)
- Message composer (mention autocomplete)
- Voice channel participant list

Bu özelliklerin hepsi **native taraftan veri çekecek** (`viscos.invoke`), DOM observer'a güvenmeyecek. Bölüm 2.2 "DOĞRU" örnekleri referans alınır.

---

## 9. Değişiklik Geçmişi

| Tarih | Değişiklik | Gerekçe |
|-------|-----------|---------|
| 2026-06-18 | İlk yayın | ADR-0012 §2 frontend mimari kararı + Faz 1.0 deliverable kaynağı. Haziran 2026 trade-off analizinde Discord DOM churn + Vencord/BetterDiscord/browser-cli kanıtları derlendi. |
