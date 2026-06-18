/**
 * Discord webpack module proxy (Vencord pattern).
 *
 * Faz 1.0: stub returning `null`. Real implementation intercepts
 * `Function.prototype.m` setter to capture Discord's webpack chunk factory.
 *
 * See [`crates/viscos-webview/BRIDGE-RESILIENCE.md`](../../crates/viscos-webview/BRIDGE-RESILIENCE.md) §3.
 */

import type { WebpackProxy } from './bridge.js';

export function createWebpackProxy(): WebpackProxy {
  // Faz 1.0 stub — full implementation lives in a follow-up PR (Faz 5+).
  return {
    findByProps: () => null,
    findByCode: () => null,
    waitFor: () => Promise.reject(new Error('waitFor not implemented in Faz 1.0')),
  };
}
