/**
 * Frontend tests for `native-bridge.ts` — Vencord/Equicord `ViscosNative` API.
 *
 * Validates that each method round-trips through the IPC bridge with a
 * mock `window.ipc.postMessage` and an installed `window.viscos` singleton.
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { __resetForTests, installViscosGlobal } from '../src/bridge.js';
import { viscos } from '../src/native-bridge.js';

interface MockIpc {
  sent: string[];
  onmessage: ((event: MessageEvent<string>) => void) | undefined;
  postMessage(msg: string): void;
  simulateIncoming(message: unknown): void;
}

function installMockIpc(): MockIpc {
  const mock: MockIpc = {
    sent: [],
    onmessage: undefined,
    postMessage(msg: string) {
      mock.sent.push(msg);
    },
    simulateIncoming(message: unknown) {
      if (mock.onmessage) {
        mock.onmessage({ data: JSON.stringify(message) } as MessageEvent<string>);
      }
    },
  };
  (window as unknown as { ipc: MockIpc }).ipc = mock;
  return mock;
}

beforeEach(() => {
  __resetForTests();
});

afterEach(() => {
  __resetForTests();
});

describe('native-bridge ViscosNative', () => {
  it('installs viscos global on window', () => {
    installMockIpc();
    installViscosGlobal();
    expect(window.viscosNative).toBeDefined();
    expect(window.viscosNative?.getVersion).toBeTypeOf('function');
  });

  it('getVersion round-trips through IPC', async () => {
    const mock = installMockIpc();
    installViscosGlobal();

    const promise = viscos.getVersion();
    await Promise.resolve();
    expect(mock.sent.length).toBe(1);
    const sent = JSON.parse(mock.sent[0]);
    expect(sent.kind).toBe('invoke');

    mock.simulateIncoming({
      kind: 'response',
      id: sent.id,
      result: { version: '0.1.0', hash: 'abc1234' },
    });
    await expect(promise).resolves.toEqual({ version: '0.1.0', hash: 'abc1234' });
  });

  it('getSettings returns the settings object', async () => {
    const mock = installMockIpc();
    installViscosGlobal();

    const promise = viscos.getSettings();
    await Promise.resolve();
    const sent = JSON.parse(mock.sent[0]);
    mock.simulateIncoming({
      kind: 'response',
      id: sent.id,
      result: { theme: 'dark', accent: 'blurple' },
    });
    await expect(promise).resolves.toEqual({ theme: 'dark', accent: 'blurple' });
  });

  it('updateSettings round-trips with the patch object', async () => {
    const mock = installMockIpc();
    installViscosGlobal();

    const promise = viscos.updateSettings({ mute: true });
    await Promise.resolve();
    const sent = JSON.parse(mock.sent[0]);
    expect(sent.kind).toBe('invoke');
    mock.simulateIncoming({ kind: 'response', id: sent.id, result: null });
    await expect(promise).resolves.toBeUndefined();
  });

  it('getDiskInfo returns freeBytes and totalBytes', async () => {
    const mock = installMockIpc();
    installViscosGlobal();

    const promise = viscos.getDiskInfo();
    await Promise.resolve();
    const sent = JSON.parse(mock.sent[0]);
    mock.simulateIncoming({
      kind: 'response',
      id: sent.id,
      result: { freeBytes: 1024, totalBytes: 2048 },
    });
    await expect(promise).resolves.toEqual({ freeBytes: 1024, totalBytes: 2048 });
  });

  it('viscos.getVersion rejects on IPC error', async () => {
    const mock = installMockIpc();
    installViscosGlobal();

    const promise = viscos.getVersion();
    await Promise.resolve();
    const sent = JSON.parse(mock.sent[0]);
    mock.simulateIncoming({
      kind: 'response',
      id: sent.id,
      error: { code: 'Internal', message: 'native bridge error' },
    });
    await expect(promise).rejects.toMatchObject({ code: 'Internal' });
  });
});
