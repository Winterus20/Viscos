/**
 * IpcEvent tagged union — mirrors `crates/viscos-ipc/src/event.rs::IpcEvent`.
 *
 * **Source of truth:** Rust enum. Every variant must be mirrored here.
 * Run `cargo run -p codegen-ipc` to regenerate (ADR-0012 §bridge.ts).
 *
 * **Wire format** (Rust `#[serde(tag = "kind", content = "payload")]`):
 * ```json
 * { "kind": "MessageCreated", "payload": { "channel_id": 123, "message_id": 456 } }
 * ```
 *
 * Push events are **small and real-time only** (mention badge, notification,
 * watchdog alert, new message signal). Large state transfers use pull-based
 * `IpcCommand` (ADR-0012 §3).
 *
 * Cross-references:
 * - `crates/viscos-ipc/src/event.rs` — Rust canonical source.
 * - ADR-0012 §3 — pull-based IPC pattern.
 *
 * ## Rust → TypeScript mapping
 *
 * | Rust variant           | TS `kind` discriminant   | Phase |
 * |------------------------|--------------------------|-------|
 * | `LoginSuccess`         | `'LoginSuccess'`         | 2.0   |
 * | `LoginFailure`         | `'LoginFailure'`         | 2.0   |
 * | `MessageCreated`       | `'MessageCreated'`       | 3.0   |
 * | `MessageEdited`        | `'MessageEdited'`        | 3.0   |
 * | `DraftSaved`           | `'DraftSaved'`           | 5.0   |
 * | `UnreadCountChanged`   | `'UnreadCountChanged'`   | 1.0   |
 * | `ThemeChanged`         | `'ThemeChanged'`         | 1.0   |
 * | `WatchdogAlert`        | `'WatchdogAlert'`        | 1.0   |
 */

// ---------------------------------------------------------------------------
// WatchdogKind  (snake_case per Rust `#[serde(rename_all = "snake_case")]`)
// ---------------------------------------------------------------------------

/**
 * Watchdog alert category.
 *
 * Mirrors `crates/viscos-ipc/src/event.rs::WatchdogKind` with
 * `#[serde(rename_all = "snake_case")]`.
 *
 * | Rust variant          | Wire value              |
 * |-----------------------|-------------------------|
 * | `GdiLeakWarning`      | `"gdi_leak_warning"`    |
 * | `GdiLeakCritical`     | `"gdi_leak_critical"`   |
 * | `IpcBufferWarning`    | `"ipc_buffer_warning"`  |
 * | `IpcBufferCritical`   | `"ipc_buffer_critical"` |
 */
export type WatchdogKind =
  | 'gdi_leak_warning'
  | 'gdi_leak_critical'
  | 'ipc_buffer_warning'
  | 'ipc_buffer_critical';

// ---------------------------------------------------------------------------
// IpcEvent
// ---------------------------------------------------------------------------

/**
 * Backend → Frontend small push event.
 *
 * `#[non_exhaustive]` on Rust side — new variants may arrive in future phases.
 * Always handle the `default` / `else` branch when exhaustively switching.
 */
export type IpcEvent =
  // -------------------------------------------------------------------------
  // Auth (Phase 2.0)
  // -------------------------------------------------------------------------
  /** Login succeeded: token validated, gateway connect triggered. */
  | { kind: 'LoginSuccess'; payload: { user_id: number } }
  /** Login failed (invalid token, MFA required, etc.). */
  | { kind: 'LoginFailure'; payload: { code: string; message: string } }

  // -------------------------------------------------------------------------
  // Messages (Phase 3.0 — gateway MESSAGE_CREATE push)
  // -------------------------------------------------------------------------
  /** New message received (gateway `MESSAGE_CREATE` or `SendMessage` ack). */
  | { kind: 'MessageCreated'; payload: { channel_id: number; message_id: number } }
  /** Existing message edited (gateway `MESSAGE_UPDATE`). */
  | { kind: 'MessageEdited'; payload: { channel_id: number; message_id: number } }

  // -------------------------------------------------------------------------
  // Drafts (Phase 5.0)
  // -------------------------------------------------------------------------
  /** Draft autosaved (periodic watchdog callback). */
  | { kind: 'DraftSaved'; payload: { channel_id: number; content_len: number } }

  // -------------------------------------------------------------------------
  // Mentions / counts
  // -------------------------------------------------------------------------
  /** Mention/unread count changed — update tray badge. */
  | { kind: 'UnreadCountChanged'; payload: { count: number } }

  // -------------------------------------------------------------------------
  // Theme + watchdog (Phase 1.0)
  // -------------------------------------------------------------------------
  /** Theme changed (OS or user). */
  | { kind: 'ThemeChanged'; payload: { theme: string } }
  /** Watchdog alert (GDI leak / IPC buffer overflow / etc.). */
  | { kind: 'WatchdogAlert'; payload: { kind: WatchdogKind; message: string } };
