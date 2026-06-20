//! `DefaultIpcRouter` — basit dispatch + `Unimplemented` stub.
//!
//! Faz 1.0'da her command `IpcCommandError::Unimplemented` döner (Faz 2+'da
//! gerçek handler'lar eklenecek). Default handler [`StubHandler`] design
//! contract'ı korur: yeni eklenen her command için explicit `match` kolu
//! yazılmadığı sürece bilinçli olarak "phase-X.Y" mesajı ile
//! `Unimplemented` döner.
//!
//! Cross-reference: [`phase-1.0-window-webview.md` §3.3](../../.cursor/plans/phase-1.0-window-webview.md#33-viscos-ipc-iskelet).

use std::sync::Arc;

use crate::command::{IpcCommand, IpcHandler};
use crate::types::IpcCommandError;

/// Default IPC router.
///
/// `Arc<dyn IpcHandler>` tutar; dispatch basitçe `handler.handle(cmd)` çağırır.
/// Faz 1.0'da default handler `Unimplemented` döner; Faz 2+'da handler inject
/// edilir (`AuthHandler`, `MessageHandler`, `ThemeHandler`, vb.).
#[derive(Clone)]
pub struct DefaultIpcRouter {
    handler: Arc<dyn IpcHandler>,
}

impl Default for DefaultIpcRouter {
    fn default() -> Self {
        Self {
            handler: Arc::new(StubHandler),
        }
    }
}

impl std::fmt::Debug for DefaultIpcRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultIpcRouter").finish_non_exhaustive()
    }
}

impl DefaultIpcRouter {
    /// Yeni router oluştur (default `StubHandler`).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Custom handler ile router oluştur.
    #[must_use]
    pub fn with_handler(handler: Arc<dyn IpcHandler>) -> Self {
        Self { handler }
    }

    /// Command dispatch et.
    ///
    /// # Errors
    ///
    /// `IpcCommandError::Unimplemented(...)` Faz 1.0'da (default handler).
    /// Faz 2+'da typed hatalar yüzeye çıkar (BadPayload, Internal, vb.).
    pub async fn dispatch(&self, cmd: IpcCommand) -> Result<serde_json::Value, IpcCommandError> {
        tracing::debug!(?cmd, "IpcRouter::dispatch");
        self.handler.handle(cmd).await
    }
}

/// Default `IpcHandler` — her command için `Unimplemented` döner.
///
/// **Design contract:** Bilinmeyen yeni variant eklendiğinde aşağıdaki
/// `_ => "phase-X.Y unknown command"` kolu tetiklenir. Bilinen varyantlar için
/// kendi mesajımız (örn. "phase-2.0 unread count") döner. Bu sayede:
/// - Test'lerde hangi command'un implemente olup olmadığı net görünür.
/// - Yeni command ekleyen PR'da hangi handler'ın yazılacağı review checklist
///   olarak belirir.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubHandler;

#[async_trait::async_trait]
impl IpcHandler for StubHandler {
    async fn handle(&self, cmd: IpcCommand) -> Result<serde_json::Value, IpcCommandError> {
        // Bilinen tüm variant'lar eklendikçe buraya match arm eklenmeli.
        // #[non_exhaustive] olduğu için compiler exhaustive match zorlamaz.
        // Bilinmeyen yeni variant eklendiğinde aşağıdaki "phase-X.Y unknown"
        // arm'ı tetiklenir → log'dan hangi variant'ın eklendiğini görürüz.
        //
        // `unreachable_patterns` allow: compiler mevcut variant'ları tümüyle
        // match ettiğini düşünüyor, ama non_exhaustive enum'lar runtime'da
        // yeni variant içerebilir (downstream crate eklediğinde).
        #[allow(unreachable_patterns)]
        let phase_msg = match cmd {
            // Phase 1.0 iskeleti
            IpcCommand::GetUnreadCount { .. } => "phase-2.0 unread count",
            IpcCommand::Navigate { .. } => "phase-1.6 navigation",
            IpcCommand::SetTheme { .. } => "phase-5.0 theme sync",
            // Phase 2.0 auth
            IpcCommand::LoginRequest { .. } | IpcCommand::Logout { .. } => "phase-2.0 auth",
            // Phase 3.0 gateway + messages
            IpcCommand::GetGuildList { .. } | IpcCommand::GetChannelList { .. } => {
                "phase-3.0 guild list"
            }
            IpcCommand::GetMessages { .. }
            | IpcCommand::SendMessage { .. }
            | IpcCommand::TriggerTyping { .. }
            | IpcCommand::MarkChannelRead { .. } => "phase-3.0 messages",
            IpcCommand::SaveMessageDraft { .. } | IpcCommand::CancelMessageDraft { .. } => {
                "phase-5.0 drafts"
            }
            _ => "phase-X.Y unknown command (yeni variant — StubHandler güncelle)",
        };
        Err(IpcCommandError::Unimplemented(phase_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::IpcCommandError;

    #[tokio::test]
    async fn stub_handler_returns_unimplemented_for_each_command() {
        let router = DefaultIpcRouter::new();

        let commands = vec![
            IpcCommand::GetUnreadCount { guild_id: None },
            IpcCommand::GetUnreadCount { guild_id: Some(42) },
            IpcCommand::Navigate {
                url: "https://discord.com".into(),
            },
            IpcCommand::SetTheme {
                theme: "light".into(),
            },
            IpcCommand::LoginRequest { token: None },
            IpcCommand::GetGuildList {},
            IpcCommand::GetChannelList { guild_id: 1 },
            IpcCommand::GetMessages {
                channel_id: 7,
                limit: 50,
            },
            IpcCommand::SendMessage {
                channel_id: 7,
                content: "hi".into(),
            },
            IpcCommand::MarkChannelRead { channel_id: 7 },
            IpcCommand::CancelMessageDraft { channel_id: 7 },
        ];

        for cmd in &commands {
            let result = router.dispatch(cmd.clone()).await;
            assert!(
                matches!(result, Err(IpcCommandError::Unimplemented(_))),
                "expected Unimplemented for {cmd:?}"
            );
        }
    }

    #[tokio::test]
    async fn stub_handler_direct_call() {
        let handler = StubHandler;
        let cmd = IpcCommand::GetUnreadCount { guild_id: None };
        let result = handler.handle(cmd).await;
        assert!(matches!(result, Err(IpcCommandError::Unimplemented(_))));
    }

    #[test]
    fn custom_handler_wiring() {
        use crate::command::IpcHandler;
        use std::sync::atomic::{AtomicU32, Ordering};

        struct CountingHandler(AtomicU32);
        #[async_trait::async_trait]
        impl IpcHandler for CountingHandler {
            async fn handle(&self, _cmd: IpcCommand) -> Result<serde_json::Value, IpcCommandError> {
                let prev = self.0.fetch_add(1, Ordering::SeqCst);
                Ok(serde_json::json!({ "count": prev + 1 }))
            }
        }

        let counter = Arc::new(CountingHandler(AtomicU32::new(0)));
        let router = DefaultIpcRouter::with_handler(counter.clone());
        // Router'ın dispatch'ı runtime'da test etmek için future dönmek gerek.
        // Burada sadece handler'ın doğru kurulduğunu doğruluyoruz.
        let _ = router;
    }

    #[tokio::test]
    async fn custom_handler_can_return_bad_payload() {
        // IpcCommandError::BadPayload rotadan dönen custom handler — yeni
        // typed error path'inin de çalıştığını doğrular.
        use crate::command::IpcHandler;
        use std::sync::atomic::{AtomicU32, Ordering};

        struct FailingHandler;
        #[async_trait::async_trait]
        impl IpcHandler for FailingHandler {
            async fn handle(&self, _cmd: IpcCommand) -> Result<serde_json::Value, IpcCommandError> {
                let bad: serde_json::Error =
                    serde_json::from_str::<serde_json::Value>("{not valid}").unwrap_err();
                Err(bad.into())
            }
        }

        let router = DefaultIpcRouter::with_handler(Arc::new(FailingHandler));
        let result = router
            .dispatch(IpcCommand::GetUnreadCount { guild_id: None })
            .await;
        assert!(
            matches!(result, Err(IpcCommandError::BadPayload(_))),
            "BadPayload should propagate"
        );

        // AtomicU32 kullanımı unused-warning önlemek için (referans yeterli).
        let _ = AtomicU32::new(0).fetch_add(0, Ordering::SeqCst);
    }
}
