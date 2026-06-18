/**
 * Frontend tests for `viscos://` deep-link URL parser (TypeScript side).
 *
 * Mirrors the Rust `parse_viscos_url` in
 * `crates/viscos-shell/src/integration/deep_link.rs`. Used by Vencord/Equicord
 * plugins that need to construct deep links.
 */

import { describe, it, expect } from 'vitest';

export type DeepLinkAction =
  | { type: 'openChannel'; guildId: number | null; channelId: number }
  | { type: 'openInvite'; code: string }
  | { type: 'openPlugin'; id: string }
  | { type: 'unknown'; url: string };

export function parseViscosUrl(url: string): DeepLinkAction | null {
  const stripped = url.replace(/^viscos:\/\//, '');
  const parts = stripped.split('/').filter((s) => s.length > 0);
  const head = parts[0];

  if (head === 'channel') {
    if (parts.length === 2) {
      const channelId = Number(parts[1]);
      if (Number.isNaN(channelId)) return { type: 'unknown', url };
      return { type: 'openChannel', guildId: null, channelId };
    }
    if (parts.length === 3) {
      const guildId = Number(parts[1]);
      const channelId = Number(parts[2]);
      if (Number.isNaN(guildId) || Number.isNaN(channelId)) return null;
      return { type: 'openChannel', guildId, channelId };
    }
    return { type: 'unknown', url };
  }
  if (head === 'invite' && parts.length === 2) {
    return { type: 'openInvite', code: parts[1] };
  }
  if (head === 'plugin' && parts.length === 2) {
    return { type: 'openPlugin', id: parts[1] };
  }
  if (url === '' || url === 'viscos://') return null;
  return { type: 'unknown', url };
}

describe('parseViscosUrl', () => {
  it('parses channel/guild/channel', () => {
    const result = parseViscosUrl('viscos://channel/123/456');
    expect(result).toEqual({ type: 'openChannel', guildId: 123, channelId: 456 });
  });

  it('parses channel/dm', () => {
    const result = parseViscosUrl('viscos://channel/789');
    expect(result).toEqual({ type: 'openChannel', guildId: null, channelId: 789 });
  });

  it('parses invite/code', () => {
    const result = parseViscosUrl('viscos://invite/xyz');
    expect(result).toEqual({ type: 'openInvite', code: 'xyz' });
  });

  it('parses plugin/id', () => {
    const result = parseViscosUrl('viscos://plugin/my-plugin');
    expect(result).toEqual({ type: 'openPlugin', id: 'my-plugin' });
  });

  it('returns unknown for non-viscos scheme', () => {
    // TS parser doesn't recognize the scheme — falls through to unknown.
    expect(parseViscosUrl('https://example.com')).toEqual({
      type: 'unknown',
      url: 'https://example.com',
    });
  });

  it('returns null for empty url', () => {
    expect(parseViscosUrl('')).toBeNull();
    expect(parseViscosUrl('viscos://')).toBeNull();
  });

  it('returns unknown for invalid channel id', () => {
    // Invalid numeric id → unknown (not null)
    expect(parseViscosUrl('viscos://channel/abc')).toEqual({
      type: 'unknown',
      url: 'viscos://channel/abc',
    });
  });

  it('returns unknown for unrecognised routes', () => {
    expect(parseViscosUrl('viscos://other/foo')).toEqual({
      type: 'unknown',
      url: 'viscos://other/foo',
    });
  });

  it('returns unknown for too many path segments', () => {
    expect(parseViscosUrl('viscos://channel/1/2/3/4')).toEqual({
      type: 'unknown',
      url: 'viscos://channel/1/2/3/4',
    });
  });
});
