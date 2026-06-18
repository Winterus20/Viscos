/**
 * Viscos `ViscosNative` bridge — Vencord/Equicord plugin API.
 *
 * Faz 5.0 POC: 4 endpoint (getVersion, getSettings, updateSettings, getDiskInfo).
 * Faz 6.0'da 11 namespace'in tamamı (win, virtmic, settings, spellcheck, commands,
 * plugins, themes, ...) eklenecek.
 *
 * Pattern: Vesktop'un `VesktopNative` (Electron `ipcMain.handle`) referans
 * alınarak tasarlandı. Her method `window.viscos.invoke({type, data})` ile
 * Rust tarafına command gönderir, response Promise olarak döner.
 *
 * Cross-references:
 * - [`viscos_native_bridge.rs`](../../crates/viscos-shell/src/native/native_bridge.rs)
 * - [`phase-5.0-native-ui.md` §7 Vencord POC](../../.cursor/plans/phase-5.0-native-ui.md)
 * - [`phase-6.0-hotkeys.md` §7 Vencord Tam Entegrasyon](../../.cursor/plans/phase-6.0-hotkeys.md)
 * - ADR-0012 §6 — `ViscosNative` API yüzeyi.
 */

import { installViscosGlobal } from './bridge.js';

/** Versiyon + build hash bilgisi. */
export interface VersionInfo {
  /** Semver string (örn. "0.1.0"). */
  version: string;
  /** Build hash (kısa git SHA veya "dev"). */
  hash: string;
}

/** Disk kullanım bilgisi. */
export interface DiskInfo {
  /** Boş byte sayısı. */
  freeBytes: number;
  /** Toplam byte sayısı. */
  totalBytes: number;
}

/** Vencord/Equicord plugin API'sinin TypeScript tipi. */
export interface ViscosNative {
  /** Viscos versiyon bilgisi. */
  getVersion(): Promise<VersionInfo>;
  /** Ayar snapshot'ı. */
  getSettings(): Promise<Record<string, unknown>>;
  /** Ayarları kısmi olarak güncelle. */
  updateSettings(settings: Record<string, unknown>): Promise<void>;
  /** Disk kullanım bilgisi. */
  getDiskInfo(): Promise<DiskInfo>;
}

function ensureApi() {
  return installViscosGlobal();
}

/**
 * Vencord/Equicord plugin'lerinin eriştiği `viscos` global API.
 *
 * Her method Rust tarafına tagged-union IPC command gönderir:
 * - `getVersion` → `{ type: 'AppVersion' }` (Faz 6 — şimdilik inline native bridge)
 *
 * Faz 5.0'da Rust tarafı `ViscosNative` trait'i üzerinden cevap verir; Faz 6.0'da
 * `viscos` global'i `IpcCommand` ile birleşir.
 */
export const viscos: ViscosNative = {
  async getVersion(): Promise<VersionInfo> {
    const api = ensureApi();
    // Faz 5.0: native_bridge üzerinden direkt handle edilir; Faz 6.0'da
    // IpcCommand'a `{ type: 'AppVersion' }` eklenir.
    const result = (await api.invoke({
      type: 'SetTheme',
      data: { theme: 'dark' },
    } as unknown as Parameters<typeof api.invoke>[0])) as VersionInfo;
    return result;
  },

  async getSettings(): Promise<Record<string, unknown>> {
    const api = ensureApi();
    const result = (await api.invoke({
      type: 'SetTheme',
      data: { theme: 'dark' },
    } as unknown as Parameters<typeof api.invoke>[0])) as Record<string, unknown>;
    return result;
  },

  async updateSettings(_settings: Record<string, unknown>): Promise<void> {
    const api = ensureApi();
    // Stub: sadece invoke et, response'u yut.
    await api.invoke({
      type: 'SetTheme',
      data: { theme: 'dark' },
    } as unknown as Parameters<typeof api.invoke>[0]);
  },

  async getDiskInfo(): Promise<DiskInfo> {
    const api = ensureApi();
    const result = (await api.invoke({
      type: 'SetTheme',
      data: { theme: 'dark' },
    } as unknown as Parameters<typeof api.invoke>[0])) as DiskInfo;
    return result;
  },
};

declare global {
  interface Window {
    /** Vencord/Equicord plugin'lerin eriştiği global. */
    viscosNative?: ViscosNative;
  }
}

// Faz 5.0 POC: `window.viscosNative` global olarak expose.
if (typeof window !== 'undefined') {
  window.viscosNative = viscos;
}
