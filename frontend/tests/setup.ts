// Vitest setup — disable bridge auto-install so tests can install a mock
// window.ipc *before* the bridge attaches its onmessage listener.
(globalThis as { __VISCOS_BRIDGE_AUTORUN__?: boolean }).__VISCOS_BRIDGE_AUTORUN__ = false;
