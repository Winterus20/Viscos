//! `viscos-core` — domain types, traits, events.
//!
//! **I/O yapmaz.** Sadece std + serde.
//! Faz 1+'ta doldurulacak: events, traits (Backend), state.

pub mod events;
pub mod traits;
pub mod types;

pub use events::{AppEvent, CoreEvent};
pub use traits::Backend;
pub use types::AppContext;

/// Viscos build bilgisi — runtime'da `env!` macro'larıyla doldurulur.
pub const VISCOS_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const VISCOS_NAME: &str = env!("CARGO_PKG_NAME");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_metadata_is_set() {
        // CARGO_PKG_VERSION her zaman workspace.package.version'dan gelir.
        assert!(!VISCOS_VERSION.is_empty());
        assert_eq!(VISCOS_NAME, "viscos-core");
    }

    #[test]
    fn app_context_default() {
        let ctx = AppContext::default();
        assert_eq!(ctx.name, "Viscos");
    }
}
