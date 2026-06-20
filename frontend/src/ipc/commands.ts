/**
 * IpcCommand tagged union — mirrors `crates/viscos-ipc/src/command.rs::IpcCommand`.
 *
 * **Source of truth:** Rust enum. Every variant must be mirrored here.
 * Run `cargo run -p codegen-ipc` to regenerate (ADR-0012 §bridge.ts).
 *
 * **Wire format** (Rust `#[serde(tag = "type", content = "data")]`):
 * ```json
 * { "type": "GetMessages", "data": { "channel_id": 123456789, "limit": 50 } }
 * ```
 *
 * All commands are pull-based (JS → Rust). Responses come back via
 * `window.ipc.onmessage` as `{ kind: "response", id, result }` envelopes.
 *
 * Cross-references:
 * - `crates/viscos-ipc/src/command.rs` — Rust canonical source.
 * - ADR-0012 §3 — pull-based IPC pattern.
 *
 * ## Rust → TypeScript mapping
 *
 * | Rust variant          | TS `type` discriminant  | Phase |
 * |-----------------------|-------------------------|-------|
 * | `LoginRequest`        | `'LoginRequest'`        | 2.0   |
 * | `Logout`              | `'Logout'`              | 2.0   |
 * | `GetGuildList`        | `'GetGuildList'`        | 3.0   |
 * | `GetChannelList`      | `'GetChannelList'`      | 3.0   |
 * | `GetMessages`         | `'GetMessages'`         | 3.0   |
 * | `SendMessage`         | `'SendMessage'`         | 3.0   |
 * | `TriggerTyping`       | `'TriggerTyping'`       | 3.0   |
 * | `SaveMessageDraft`    | `'SaveMessageDraft'`    | 5.0   |
 * | `CancelMessageDraft`  | `'CancelMessageDraft'`  | 5.0   |
 * | `MarkChannelRead`     | `'MarkChannelRead'`     | 3.0   |
 * | `GetUnreadCount`      | `'GetUnreadCount'`      | 1.0   |
 * | `Navigate`            | `'Navigate'`            | 1.0   |
 * | `SetTheme`            | `'SetTheme'`            | 1.0   |
 */

/**
 * Frontend → Backend pull-based command.
 *
 * `#[non_exhaustive]` on Rust side — new variants may be added without a
 * breaking change. Always use exhaustive narrowing where possible; fall back
 * with a `never`-checked default for future-proofing.
 *
 * **Note on snowflake IDs:** Discord snowflakes fit in `u64` on the Rust side.
 * JavaScript's `number` is safe up to 2^53−1 (~9×10^15). Real Discord IDs
 * approach 2^63 — Faz 4 will migrate these fields to `string` (bigint JSON
 * representation). For now `number` is kept for API surface consistency.
 */
export type IpcCommand =
  // -------------------------------------------------------------------------
  // Auth (Phase 2.0)
  // -------------------------------------------------------------------------
  /** Start login — validate token and trigger gateway connect. */
  | { type: 'LoginRequest'; data: { token: string | null } }
  /** End session — delete token from keyring, disconnect gateway. */
  | { type: 'Logout'; data: Record<string, never> }

  // -------------------------------------------------------------------------
  // Guild + channel metadata (Phase 3.0)
  // -------------------------------------------------------------------------
  /** Fetch user's guild list (REST + cache merge). */
  | { type: 'GetGuildList'; data: Record<string, never> }
  /** Fetch channel list for a specific guild. */
  | { type: 'GetChannelList'; data: { guild_id: number } }

  // -------------------------------------------------------------------------
  // Messages (Phase 3.0)
  // -------------------------------------------------------------------------
  /** Fetch recent messages for a channel (cache-first, REST fallback). */
  | { type: 'GetMessages'; data: { channel_id: number; limit: number } }
  /** Send a new message (REST POST /channels/{id}/messages). */
  | { type: 'SendMessage'; data: { channel_id: number; content: string } }
  /** Trigger typing indicator (REST POST /channels/{id}/typing). */
  | { type: 'TriggerTyping'; data: { channel_id: number } }
  /** Autosave message draft (periodic, watchdog-triggered). */
  | { type: 'SaveMessageDraft'; data: { channel_id: number; content: string } }
  /** Discard saved draft. */
  | { type: 'CancelMessageDraft'; data: { channel_id: number } }
  /** Mark channel as read (reset mention badge). */
  | { type: 'MarkChannelRead'; data: { channel_id: number } }

  // -------------------------------------------------------------------------
  // Phase-1 skeleton commands (kept for backwards compat)
  // -------------------------------------------------------------------------
  /** Unread message count. `guild_id: null` = aggregate across all guilds. */
  | { type: 'GetUnreadCount'; data: { guild_id: number | null } }
  /** Navigate to a URL (Discord channel deep-link). */
  | { type: 'Navigate'; data: { url: string } }
  /** Change theme (`"dark"` | `"light"`). */
  | { type: 'SetTheme'; data: { theme: string } };
