/**
 * Typed IPC error enums — mirrors `crates/viscos-ipc/src/types.rs`.
 *
 * **Source of truth:** Rust crate. Run `cargo run -p codegen-ipc` to regenerate
 * when Rust types change (ADR-0012 §bridge.ts).
 *
 * `IpcCommandError` and `IpcEventError` are `#[non_exhaustive]` on the Rust
 * side; the `unknown` branch below matches the wildcard `_ =>` arm.
 *
 * Cross-references:
 * - `crates/viscos-ipc/src/types.rs` — Rust canonical source.
 * - ADR-0012 §3 — pull-based IPC pattern.
 * - ADR-0007 — error handling policy (typed at library boundary).
 */

// ---------------------------------------------------------------------------
// Command errors  (IpcCommandError in Rust)
// ---------------------------------------------------------------------------

/**
 * Typed error returned by `IpcCommand` handlers.
 *
 * Mirrors `crates/viscos-ipc/src/types.rs::IpcCommandError`.
 *
 * | Rust variant           | TS discriminant       |
 * |------------------------|-----------------------|
 * | `UnknownCommand(s)`    | `'UnknownCommand'`    |
 * | `BadPayload(e)`        | `'BadPayload'`        |
 * | `Internal(e)`          | `'Internal'`          |
 * | `Unimplemented(s)`     | `'Unimplemented'`     |
 * | (future / non_exhaustive) | `'Unknown'`        |
 */
export type IpcCommandError =
  | { type: 'UnknownCommand'; command: string }
  | { type: 'BadPayload'; message: string }
  | { type: 'Internal'; message: string }
  | { type: 'Unimplemented'; feature: string }
  | { type: 'Unknown'; raw: unknown };

// ---------------------------------------------------------------------------
// Event errors  (IpcEventError in Rust)
// ---------------------------------------------------------------------------

/**
 * Typed error emitted when a Rust → JS push event fails.
 *
 * Mirrors `crates/viscos-ipc/src/types.rs::IpcEventError`.
 *
 * | Rust variant           | TS discriminant    |
 * |------------------------|--------------------|
 * | `Serialize(e)`         | `'Serialize'`      |
 * | `ChannelClosed`        | `'ChannelClosed'`  |
 * | `Internal(e)`          | `'Internal'`       |
 * | (future / non_exhaustive) | `'Unknown'`     |
 */
export type IpcEventError =
  | { type: 'Serialize'; message: string }
  | { type: 'ChannelClosed' }
  | { type: 'Internal'; message: string }
  | { type: 'Unknown'; raw: unknown };

// ---------------------------------------------------------------------------
// Result wrappers
// ---------------------------------------------------------------------------

/**
 * Discriminated-union result for IPC commands — success or typed error.
 *
 * The Rust side uses `Result<T, IpcCommandError>`; the IPC channel serialises
 * it as `{ ok: true, value: T }` or `{ ok: false, error: IpcCommandError }`.
 */
export type IpcCommandResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: IpcCommandError };

/**
 * Discriminated-union result for IPC event emission.
 *
 * Mirrors `IpcEventResult<T>` in Rust.
 */
export type IpcEventResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: IpcEventError };
