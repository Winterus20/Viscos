/**
 * Viscos preload bridge — `window.viscos` global API.
 *
 * Faz 1.0 deliverable: this file is bundled to `dist/preload.js` via esbuild
 * and injected into the WebView as an initialization script.
 *
 * **Selector resilience rules:** see
 * [`crates/viscos-webview/BRIDGE-RESILIENCE.md`](../../crates/viscos-webview/BRIDGE-RESILIENCE.md).
 *
 * **Pull-based IPC:** Rust → JS push is restricted to small events only.
 * Large state transfers must use `await viscos.invoke(cmd)` (Faz 4 SharedBuffer).
 *
 * **Type source of truth:** `crates/viscos-ipc` Rust crate. TS types in
 * `./ipc/` are derived from it (PR-5 follow-up, ADR-0012 §bridge.ts).
 *
 * Cross-references:
 * - ADR-0012 §2 — selector resilience rules.
 * - [`webview2-hardening.md`](../../.cursor/plans/webview2-hardening.md) §3 — pull-based IPC pattern.
 * - [`crates/viscos-ipc`](../../crates/viscos-ipc) — Rust-side command/event enums.
 */

export type { IpcCommand } from './ipc/commands.js';
export type { IpcEvent, WatchdogKind } from './ipc/events.js';
export type { IpcCommandError, IpcCommandResult, IpcEventError, IpcEventResult } from './ipc/types.js';

import type { IpcCommand } from './ipc/commands.js';
import type { IpcEvent } from './ipc/events.js';

/**
 * Discord webpack module proxy — Vencord pattern.
 *
 * Faz 1.0: stub returning `null`. Real implementation lives in
 * `webpack-shim.ts` and is injected before this file.
 */
export interface WebpackProxy {
  findByProps(...propNames: string[]): Record<string, unknown> | null;
  findByCode(...codes: string[]): unknown | null;
  waitFor(filter: (module: unknown) => boolean): Promise<unknown>;
}

/**
 * Viscos global API exposed to Discord web content.
 */
export interface ViscosApi {
  /**
   * Pull-based IPC: send a command and await a typed response.
   *
   * Throws an `IpcCommandError`-shaped object if the Rust handler returns
   * an error (e.g. `IpcCommandError::Unimplemented` during Phase 1.0 stubs —
   * see `crates/viscos-ipc/src/router.rs::StubHandler`).
   */
  invoke<T = unknown>(cmd: IpcCommand): Promise<T>;

  /**
   * Subscribe to small Rust → JS events (tray badge, watchdog alert, theme,
   * login result, new message signal).
   *
   * Returns an unsubscribe function — **must be called on unmount** to avoid
   * the channel callback memory leak (tauri#13133).
   */
  onEvent(handler: (event: IpcEvent) => void): () => void;

  /** Discord webpack module proxy (Vencord pattern). */
  webpack: WebpackProxy;

  /** IPC protocol version — must match `IPC_PROTOCOL_VERSION` in Rust. */
  readonly protocolVersion: 1;
}

declare global {
  interface Window {
    /**
     * Discriminated-union message envelope between JS and Rust IPC channel.
     * Discriminators:
     * - `kind === 'invoke'` → JS → Rust pull-based command.
     * - `kind === 'response'` → Rust → JS response to a previous `invoke`.
     * - `kind === 'event'` → Rust → JS push event (small only).
     */
    ipc?: {
      postMessage(message: string): void;
      onmessage?: (event: MessageEvent<string>) => void;
    };
    /** Viscos global API (this file). */
    viscos?: ViscosApi;
  }
}

interface PendingRequest {
  resolve: (value: unknown) => void;
  reject: (reason: unknown) => void;
}

interface ViscosInternals {
  api: ViscosApi | null;
  pending: Map<number, PendingRequest>;
  nextId: number;
}

const internals: ViscosInternals = {
  api: null,
  pending: new Map(),
  nextId: 0,
};

function handleIncomingMessage(event: MessageEvent<string>): void {
  let parsed: unknown;
  try {
    parsed = JSON.parse(event.data);
  } catch {
    console.warn('viscos: invalid IPC message (not JSON)');
    return;
  }

  if (typeof parsed !== 'object' || parsed === null) {
    return;
  }

  const message = parsed as {
    kind?: string;
    id?: number;
    result?: unknown;
    error?: unknown;
    payload?: unknown;
  };

  if (message.kind === 'response' && typeof message.id === 'number') {
    const pending = internals.pending.get(message.id);
    if (!pending) {
      return; // Late response or unknown id — silently ignore.
    }
    internals.pending.delete(message.id);
    if (message.error !== undefined && message.error !== null) {
      pending.reject(message.error);
    } else {
      pending.resolve(message.result);
    }
    return;
  }

  if (message.kind === 'event') {
    if (!internals.api) {
      return;
    }
    // Event handlers are managed by `onEvent` — broadcast via custom mechanism.
    // We dispatch to all registered subscribers (set on the api).
    const handlers = (internals.api as unknown as { __eventHandlers?: Set<(e: IpcEvent) => void> })
      .__eventHandlers;
    if (handlers) {
      for (const handler of handlers) {
        try {
          handler(message.payload as IpcEvent);
        } catch (err) {
          console.error('viscos: event handler threw', err);
        }
      }
    }
  }
}

function ensureIpcBridge(): NonNullable<Window['ipc']> {
  if (!window.ipc) {
    window.ipc = {
      postMessage: (msg: string) => {
        console.warn('viscos: window.ipc.postMessage called but no Rust IPC bridge attached', msg);
      },
    };
  }
  // Attach the incoming-message listener exactly once.
  if (!window.ipc.onmessage) {
    const listener = handleIncomingMessage;
    window.ipc.onmessage = listener;
  }
  return window.ipc;
}

function createWebpackProxy(): WebpackProxy {
  return {
    findByProps: () => null,
    findByCode: () => null,
    waitFor: () => Promise.reject(new Error('waitFor not implemented in Faz 1.0')),
  };
}

function createApi(): ViscosApi {
  ensureIpcBridge();
  const eventHandlers = new Set<(e: IpcEvent) => void>();
  const api: ViscosApi = {
    invoke<T = unknown>(cmd: IpcCommand): Promise<T> {
      return new Promise<T>((resolve, reject) => {
        const id = ++internals.nextId;
        internals.pending.set(id, {
          resolve: resolve as (value: unknown) => void,
          reject,
        });
        try {
          window.ipc?.postMessage(JSON.stringify({ kind: 'invoke', id, cmd }));
        } catch (err) {
          internals.pending.delete(id);
          reject(err);
        }
      });
    },

    onEvent(handler: (event: IpcEvent) => void): () => void {
      eventHandlers.add(handler);
      // Return cleanup function — **must be called on unmount** to avoid
      // the channel callback leak (tauri#13133).
      return () => {
        eventHandlers.delete(handler);
      };
    },

    webpack: createWebpackProxy(),

    protocolVersion: 1,
  };

  // Stash handlers on the api for `handleIncomingMessage` to access.
  // Done after creation so `internals.api` is already the new object.
  internals.api = api;
  (api as unknown as { __eventHandlers: Set<(e: IpcEvent) => void> }).__eventHandlers =
    eventHandlers;
  return api;
}

/**
 * Install the viscos global. Idempotent — calling twice returns the existing
 * API rather than overwriting (subsequent `invoke` registrations would leak).
 *
 * The first call lazily creates the API and attaches an `onmessage` listener to
 * `window.ipc`. Call this *after* any test mocks have been installed, otherwise
 * the bridge binds to whatever `window.ipc` is at module-load time.
 */
export function installViscosGlobal(): ViscosApi {
  if (!internals.api) {
    internals.api = createApi();
  }
  if (!window.viscos) {
    window.viscos = internals.api;
  }
  return window.viscos;
}

// Auto-install on script load unless a test host sets the flag below.
// Vitest sets `globalThis.__VISCOS_BRIDGE_AUTORUN__ = false` via tests/setup.ts
// so the bridge is only attached when tests explicitly call installViscosGlobal().
type AutoRunGlobal = typeof globalThis & { __VISCOS_BRIDGE_AUTORUN__?: boolean };
const autoRun = (globalThis as AutoRunGlobal).__VISCOS_BRIDGE_AUTORUN__;
if (autoRun !== false) {
  installViscosGlobal();
}

/**
 * Test-only hook: clear pending requests and reset the global. Vitest only.
 */
export function __resetForTests(): void {
  internals.pending.clear();
  internals.nextId = 0;
  internals.api = null;
  delete window.viscos;
  delete window.ipc;
}
