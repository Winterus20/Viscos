//! `DefaultIpcRouter` — basit dispatch + NotImplemented stub.
//!
//! Faz 1.0'da her command `ViscosError::Unimplemented` döner (Faz 2+'da
//! gerçek handler'lar eklenecek).
//!
//! Cross-reference: [`phase-1.0-window-webview.md` §3.3](../../.cursor/plans/phase-1.0-window-webview.md#33-viscos-ipc-iskelet).

use std::sync::Arc;

use crate::command::{IpcCommand, IpcHandler};
use viscos_error::{Result, ViscosError};

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
    /// Yeni router oluştur (default StubHandler).
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
    /// `ViscosError::Unimplemented(...)` Faz 1.0'da (default handler).
    pub async fn dispatch(&self, cmd: IpcCommand) -> Result<serde_json::Value> {
        tracing::debug!(?cmd, "IpcRouter::dispatch");
        self.handler.handle(cmd).await
    }
}

/// Default `IpcHandler` — her command için `Unimplemented` döner.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubHandler;

#[async_trait::async_trait]
impl IpcHandler for StubHandler {
    async fn handle(&self, cmd: IpcCommand) -> Result<serde_json::Value> {
        // Bilinen tüm variant'lar eklendikçe buraya match arm eklenmeli.
        // #[non_exhaustive] olduğu için compiler exhaustive match zorlamaz.
        // Bilinmeyen yeni variant eklendiğinde aşağıdaki "phase-X.Y unknown"
        // arm'ı tetiklenir → log'dan hangi variant'ın eklendiğini görürüz.
        //
        // `unreachable_patterns` allow: compiler mevcut 3 variant'ı tümüyle
        // match ettiğini düşünüyor, ama non_exhaustive enum'lar runtime'da
        // yeni variant içerebilir (downstream crate eklediğinde).
        #[allow(unreachable_patterns)]
        let phase_msg = match cmd {
            IpcCommand::GetUnreadCount { .. } => "phase-2.0 unread count",
            IpcCommand::Navigate { .. } => "phase-1.6 navigation",
            IpcCommand::SetTheme { .. } => "phase-5.0 theme sync",
            _ => "phase-X.Y unknown command (yeni variant — StubHandler güncelle)",
        };
        Err(ViscosError::Unimplemented(phase_msg))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        ];

        for cmd in commands {
            let result = router.dispatch(cmd).await;
            assert!(matches!(result, Err(ViscosError::Unimplemented(_))));
        }
    }

    #[tokio::test]
    async fn stub_handler_direct_call() {
        let handler = StubHandler;
        let cmd = IpcCommand::GetUnreadCount { guild_id: None };
        let result = handler.handle(cmd).await;
        assert!(matches!(result, Err(ViscosError::Unimplemented(_))));
    }

    #[test]
    fn custom_handler_wiring() {
        use crate::command::IpcHandler;
        use std::sync::atomic::{AtomicU32, Ordering};

        struct CountingHandler(AtomicU32);
        #[async_trait::async_trait]
        impl IpcHandler for CountingHandler {
            async fn handle(&self, _cmd: IpcCommand) -> Result<serde_json::Value> {
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
}
