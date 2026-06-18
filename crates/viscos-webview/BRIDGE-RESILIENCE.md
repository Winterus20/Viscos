# Viscos Bridge Resilience Rules (ADR-0012 §2)

> **Durum:** 🟡 Proposed (ADR-0012 — insan review merge öncesi onayı bekliyor).
> **Faz:** 1.0 (Faz 1.6'da CEF default'a geçişte güncellenecek).
> **Kaynak:** [`packet-0012-frontend-hybrid.md`](../../../packets/packet-0012-frontend-hybrid.md),
> [`bridge-resilience-research.md`](../../../plans/bridge-resilience-research.md),
> [`docs/DECISIONS.md` ADR-0012 §2](../../../DECISIONS.md#adr-0012-frontend-mimari--hibrit-webview--native-shell-haziran-2026-trade-off-revizyonu).

Bu doküman, `frontend/src/bridge.ts` ve tüm native taraftan frontend'e veri çeken
TypeScript kodlarının **zorunlu** selector/IPC kurallarını belgeler. AI-PR review
checklist'i ve ESLint custom rule (`frontend/eslint-rules/no-hashed-class-selector.js`)
ile uygulanır.

---

## 1. Neden Bridge Resilience Gerekli?

Discord sık sık DOM/class değiştiriyor. Webpack-generated **hashed class name**'ler
her deploy'da farklı (`message__5126c`, `username_c19a55`, ...). Mart 2025 UI
revamp'ı bu kırılganlığı kanıtladı: Vencord ekibi tüm plugin'leri `aria-label` /
`role` selector'larına geçirmek zorunda kaldı.

Viscos `frontend/src/bridge.ts` aynı trajedyaya düşmemeli.

---

## 2. Selector Hiyerarşisi (Öncelik Sırası)

```text
┌─────────────────────────────────────────────────────────────┐
│ 1. [aria-label*="..."] veya [role="..."]  →  EN İYİ       │
│ 2. [data-*] veya [id^="prefix-"]           →  İYİ          │
│ 3. viscos.webpack.findByProps(...)         →  İLERİ        │
│ 4. [class*="prefix_"]                      →  SON ÇARE     │
│ 5. .message__5126c (hashed class)          →  YASAK        │
│ 6. MutationObserver + heuristic            →  YASAK        │
└─────────────────────────────────────────────────────────────┘
```

### 2.1 ✅ DOĞRU Örnekler

```typescript
// 1. ARIA selector (Discord bunları değiştirmez)
const messageItem = document.querySelector('[id^="message-content-"]');
const channelName = document.querySelector('[aria-label*="channel"]');
const sendButton  = document.querySelector('[aria-label="Send Message"]');

// 2. data-* veya id prefix (Discord stable)
const currentChannel = document.querySelector('[data-list-item-id^="channels___"]');
const guildIcon      = document.querySelector('[id^="guild-icon-"]');

// 3. Webpack module discovery (Vencord pattern'i)
//    Discord'un kendi internal store'una read-only hook.
const userStore = viscos.webpack.findByProps('getCurrentUser');
const unread    = userStore?.getUnreadCount(channelId);

// 4. Bridge state pull (ADR-0012 §3)
//    DOM observer'a güvenme, native Rust tarafı truth source.
const unreadCount = await viscos.invoke({
  type: 'GetUnreadCount',
  data: { guild_id: null },
});
```

### 2.2 ❌ YANLIŞ Örnekler (PR reject)

```typescript
// ❌ Hashed class name — her Discord deploy'da kırılır.
const messageItem = document.querySelector('.message__5126c');

// ❌ React prop'larını DOM'dan okumaya çalışmak.
//    Discord React state'i DOM'a yazmıyor (çok büyük performans kaybı olurdu).
const userName = document.querySelector('.username_c19a55')?.textContent;

// ❌ MutationObserver ile heuristic unread count tahmini.
//    Backpressure yok, false-positive yüksek, GC baskısı.
const observer = new MutationObserver(() => {
  const count = document.querySelectorAll('[class*="unread"]').length;
});
observer.observe(document.body, { childList: true, subtree: true });

// ❌ Bridge push (Rust → JS eval_script) — pull-based IPC ihlali.
//    10KB+ payload → CI red flag.
window.viscos.push({ kind: 'event', payload: hugeJsonBlob });
```

---

## 3. Webpack Module Proxy (Vencord Pattern'i)

Discord kendi webpack instance'ına sahip (`wreq` — Webpack Require). Vencord
ekibi bu instance'a read-only proxy koyar:

```typescript
// frontend/src/webpack-shim.ts (Faz 1.0 — preload'a dahil)
// Function.prototype.m setter'ı override ederek webpack chunk factory'lerini yakala.

let discordWebpack: { m: Record<number, unknown>; c: Record<number, unknown> } | null = null;
const originalSetter = Object.getOwnPropertyDescriptor(Function.prototype, 'm')?.set;

if (originalSetter) {
  Object.defineProperty(Function.prototype, 'm', {
    set(value) {
      if (typeof value === 'object' && value !== null && !discordWebpack) {
        const stack = new Error().stack ?? '';
        // Sadece Discord.com bundle'ından gelen set'leri yakala
        // (React DevTools hook'unu atla).
        if (stack.includes('discord.com/assets/') && !stack.includes('react-devtools')) {
          discordWebpack = { m: value as Record<number, unknown>, c: (this as { c?: Record<number, unknown> }).c ?? {} };
        }
      }
      originalSetter.call(this, value);
    },
    get() {
      return originalSetter;
    },
    configurable: true,
  });
}

// Public API (window.viscos.webpack)
window.viscos.webpack = {
  findByProps: (...propNames: string[]): unknown | null => {
    if (!discordWebpack) return null;
    for (const id in discordWebpack.c) {
      const module = (discordWebpack.c[id] as { exports?: Record<string, unknown> } | undefined)?.exports;
      if (module && propNames.every((p) => p in module)) return module;
    }
    return null;
  },
  findByCode: (...codes: string[]): unknown | null => {
    // ... kaynak string match
    return null;
  },
  waitFor: (filter: (m: unknown) => boolean): Promise<unknown> => {
    // ... Promise döner, filter true olunca resolve
    return Promise.reject(new Error('waitFor not yet implemented'));
  },
};
```

**Trade-off:** Function.prototype setter override'ı **Discord'un kendi kodu
tarafından** fark edilebilir (anti-debug mantığı). Şu ana kadar Vencord/
Equicord/BetterDiscord production'da sorun yaşamadı; **risk düşük**.

---

## 4. Pull-Based IPC Pattern

WebView2 ↔ Rust köprüsünde **asla** Rust → JS push yapma (büyük veri).

```text
Rust → Event Bus (moka / tokio broadcast)
                ↓
JS tarafı ihtiyaç duyduğunda invoke("get_state") ile pull eder
                ↓
Rust cache'ten döner (RAM moka veya SQLite)
```

### 4.1 `eval_script` Payload Limiti

- `eval_script` payload > 10KB → CI red flag.
- Push-based büyük veri (avatar blob, sticker, message history) → WebView2 IPC buffer
  şişmesi (tauri#13758). Faz 4'te `post_shared_buffer` ile çözülecek.

### 4.2 İzin Verilen Push İstisnaları

Yalnızca küçük ve gerçek zamanlı olaylar push kalabilir:

- Tray icon badge (mention count).
- Native notification (mention DM).
- Watchdog alert (GDI leak warning) — tray tooltip update.

Tüm diğer state transferi **pull-based**.

---

## 5. AI-PR Review Checklist

Her `bridge.ts` PR'ında code reviewer şunları kontrol eder:

- [ ] Yeni `querySelector` call'larında `[aria-label]` veya `[role]` attribute selector öncelikli mi?
- [ ] Hashed class name selector var mı? (`/\.[a-z]+_[a-f0-9]{5,}/`) → PR reject
- [ ] MutationObserver + heuristic var mı? → PR reject (bridge pull kullan)
- [ ] `viscos.webpack.findByProps` ile native tarafa veri çekilebilir mi?
- [ ] `eval_script` payload > 10KB mi? → Faz 4'te SharedBuffer'a geç
- [ ] Rust → JS push call'ı sadece küçük olaylar (tray badge, notification) için mi?

### 5.1 ESLint Custom Rule

`frontend/eslint-rules/no-hashed-class-selector.js` — Discord'un hashed class
pattern'ini tespit eden local rule. Custom plugin publish etmeye gerek yok
(local rule dosyası yeterli).

```javascript
// frontend/eslint-rules/no-hashed-class-selector.js
module.exports = {
  meta: {
    type: 'problem',
    docs: {
      description:
        'Discord hashed class name selector kullanımı yasak (her deploy\'da kırılır).',
    },
    schema: [],
    messages: {
      hashedClass:
        "Hashed class selector '{{selector}}' yasak. {{suggestion}}",
    },
  },
  create(context) {
    return {
      CallExpression(node) {
        if (
          node.callee.type === 'MemberExpression' &&
          node.callee.property?.name === 'querySelector' &&
          node.arguments[0]?.type === 'Literal' &&
          typeof node.arguments[0].value === 'string'
        ) {
          const sel = node.arguments[0].value;
          // .message__5126c, .username_c19a55 gibi webpack-hashed class'lar.
          if (/\.[a-zA-Z]+_[a-f0-9]{4,}/.test(sel)) {
            context.report({
              node: node.arguments[0],
              messageId: 'hashedClass',
              data: {
                selector: sel,
                suggestion:
                  '[aria-label*="..."] veya [data-*] kullan, ya da viscos.webpack.findByProps(...) ile native taraftan çek.',
              },
            });
          }
        }
      },
    };
  },
};
```

---

## 6. Referanslar

- [Vencord Webpack Find API](https://github.com/Vendetta-Mod-Team/Vendetta/blob/main/src/lib/webpack.ts)
- [BetterDiscord Selector Guide](https://docs.betterdiscord.app/)
- [browser-cli Discord skill](https://github.com/nickthecook/browser-cli)
- [WebView2Feedback #5536 — GDI Leak](https://github.com/MicrosoftEdge/WebView2Feedback/issues/5536)
- [tauri-apps/tauri #13758 — eval_script](https://github.com/tauri-apps/tauri/issues/13758)
- [ADR-0012 §2](../../../DECISIONS.md#2-bridgets-kırılganlık-azaltma-yeni-ek)
- [`webview2-hardening.md` Bölüm 3 — Pull-Based IPC](../../../plans/webview2-hardening.md#3-pull-based-ipc-pattern-kritik)

---

## 7. Değişiklik Geçmişi

| Tarih | Değişiklik | Gerekçe |
|-------|-----------|---------|
| 2026-06-18 | İlk yayın (Faz 1.0 Dalga 1) | ADR-0012 §2 ek — bridge.ts selector resilience kuralları |
