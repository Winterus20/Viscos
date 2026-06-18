/**
 * Vencord/Equicord plugin loader (Faz 6.0 stub).
 *
 * Faz 5.0'da sadece tip + discovery API iskeleti. Faz 6.0'da:
 * - `%APPDATA%/Viscos/plugins/` dizininden plugin manifest'lerini oku
 * - Vencord'un `definePlugin` API'si ile dinamik yükle
 * - Plugin sandbox + permission check
 *
 * Cross-references:
 * - [`phase-6.0-hotkeys.md` §7 Vencord/Equicord Tam Entegrasyon](../../.cursor/plans/phase-6.0-hotkeys.md)
 * - ADR-0012 §6 — Plugin API surface.
 */

/** Plugin manifest formatı. */
export interface PluginManifest {
  /** Unique plugin ID (örn. "vencord-betterdiscord-themes"). */
  id: string;
  /** İnsan-okunabilir ad. */
  name: string;
  /** Semver versiyon. */
  version: string;
  /** Plugin yazarı. */
  author: string;
  /** Kısa açıklama. */
  description: string;
  /** Entry point (örn. "index.js"). */
  entry: string;
  /** Minimum Vencord uyumluluk versiyonu. */
  vencordCompat?: string;
  /** İstenen izinler. */
  permissions?: string[];
}

/** Yüklü plugin runtime state. */
export interface LoadedPlugin {
  manifest: PluginManifest;
  enabled: boolean;
  loadPath: string;
}

/** Plugin loader API. */
export interface PluginLoader {
  /**
   * Verilen path'ten plugin'leri keşfet (discovery).
   *
   * Faz 6.0'da `%APPDATA%/Viscos/plugins/` recursive scan + `manifest.json` parse.
   * Faz 5.0 stub: boş array döner.
   */
  discover(rootPath: string): Promise<LoadedPlugin[]>;
  /**
   * Plugin'i yükle (sandbox + permission check sonrası).
   *
   * Faz 5.0 stub: sadece discovery sonucunu wrap eder.
   */
  load(plugin: LoadedPlugin): Promise<void>;
  /**
   * Plugin'i etkinleştir.
   */
  enable(pluginId: string): Promise<void>;
  /**
   * Plugin'i devre dışı bırak.
   */
  disable(pluginId: string): Promise<void>;
  /** Yüklü plugin'leri listele. */
  list(): LoadedPlugin[];
}

/** Default plugin loader (Faz 5.0 stub). */
export class DefaultPluginLoader implements PluginLoader {
  private loaded: Map<string, LoadedPlugin> = new Map();

  async discover(_rootPath: string): Promise<LoadedPlugin[]> {
    // Faz 5.0 stub: gerçek filesystem scan Faz 6.0'da.
    return [];
  }

  async load(plugin: LoadedPlugin): Promise<void> {
    this.loaded.set(plugin.manifest.id, plugin);
  }

  async enable(pluginId: string): Promise<void> {
    const plugin = this.loaded.get(pluginId);
    if (plugin) {
      plugin.enabled = true;
    }
  }

  async disable(pluginId: string): Promise<void> {
    const plugin = this.loaded.get(pluginId);
    if (plugin) {
      plugin.enabled = false;
    }
  }

  list(): LoadedPlugin[] {
    return Array.from(this.loaded.values());
  }
}

/** Default plugin loader singleton. */
export const pluginLoader: PluginLoader = new DefaultPluginLoader();
