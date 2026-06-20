//! `tao::EventLoop::run` blocking call'u + Ctrl-C listener.
//!
//! Bu modül `shell.rs`'ten ayrıldı çünkü Faz 1.6 Dalga 1b ile
//! event loop implementasyonu 100+ satır ekledi; `.cursorrules`
//! Bölüm 2 "400 satır soft limit" gereği `shell.rs`'i sade tutmak
//! için extraction yapıldı. Tek sorumluluk: ana thread'in event
//! loop'unu çalıştırmak ve Ctrl-C'yi OS sinyali olarak handle etmek.
//!
//! ## Neden iki thread (main + viscos-ctrlc)?
//!
//! `tao::EventLoop::run()` Windows'ta **main thread'de blocking** çağrılmalı
//! (tao::Window ve WebView2 COM nesneleri main-thread affine). Async
//! `tokio::signal::ctrl_c().await`'ı main thread'de poll edemeyiz çünkü
//! loop blokluyor. Bu yüzden Ctrl-C'yi dinlemek için ayrı bir OS thread'i
//! kurup `current_thread` tokio runtime ile async signal'i bekliyoruz;
//! sinyal geldiğinde process-global `AtomicBool` flag'i set ediyoruz.
//! Ana thread'in event loop callback'i her event'te bu flag'i kontrol
//! edip `ControlFlow::Exit` ile loop'u sonlandırır.
//!
//! `'static` sınırlaması: `event_loop.run` callback'i `FnMut + 'static`
//! istiyor; bu yüzden `&self` referansları closure'a capture edemiyoruz.
//! `Box<dyn WebViewWindow>` ise `Send + Sync` (unsafe impl webview2.rs /
//! cef.rs'te) — bu yüzden move ile transfer edilebilir ve closure
//! içinde tutulabilir.

use std::sync::atomic::{AtomicBool, Ordering};

use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use viscos_webview::WebViewWindow;

/// Process-global Ctrl-C flag.
///
/// Main thread'in event loop callback'i her event'te bu flag'i kontrol
/// eder; `true` olduğunda `ControlFlow::Exit` ile loop'tan çıkar.
/// `Ordering::Relaxed` yeterli: write bir kere yapılıyor ve sadece
/// exit tetiklemek için okunuyor (memory order gerektirmeyen bir
/// boolean flag pattern'i).
static CTRL_C_RECEIVED: AtomicBool = AtomicBool::new(false);

/// Ctrl-C listener thread'i spawn et (process-global flag ile haberleşir).
///
/// `tao::EventLoop::run()` main thread'i blokladığı için async
/// `tokio::signal::ctrl_c().await`'ı ayrı bir OS thread'inde çalıştırıyoruz.
/// Sinyal geldiğinde `CTRL_C_RECEIVED` flag'ini set eder; ana thread'in
/// event loop callback'i bu flag'i kontrol eder ve `ControlFlow::Exit`
/// ile loop'u sonlandırır.
///
/// Thread leak: bu thread uygulama ömrü boyunca yaşar (Ctrl-C'yi süresiz
/// dinler). Process exit'te OS tarafından reclaim edilir — sorun değil.
pub(crate) fn spawn_ctrl_c_listener() {
    use std::thread;

    thread::Builder::new()
        .name("viscos-ctrlc".into())
        .spawn(|| {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(err) => {
                    tracing::error!(?err, "ctrl-c tokio runtime kurulamadı");
                    return;
                }
            };

            runtime.block_on(async {
                if let Err(err) = tokio::signal::ctrl_c().await {
                    tracing::error!(?err, "ctrl-c handler başarısız");
                    return;
                }
                tracing::warn!("Ctrl-C received (Faz 1.6 event loop path)");
                CTRL_C_RECEIVED.store(true, Ordering::SeqCst);
            });
        })
        .expect("viscos-ctrlc thread spawn başarısız");
}

/// `tao::EventLoop::run` blocking call'unu çalıştır.
///
/// Ayrı fonksiyon olarak tutmamızın sebebi: closure içinde `Box<dyn WebViewWindow>`
/// ve diğer non-`'static` referansları değil, sadece `'static` uyumlu
/// handle'ları kullanabiliyoruz. Fonksiyon argümanları `move` ile
/// transfer edildiğinden borrow checker sorunu yok.
pub(crate) fn run_loop(event_loop: EventLoop<()>, window: Box<dyn WebViewWindow>) {
    let webview_id = window.id();
    event_loop.run(
        move |event, _target: &EventLoopWindowTarget<()>, control: &mut ControlFlow| {
            // Ctrl-C polling — her event'te kontrol et.
            if CTRL_C_RECEIVED.load(Ordering::Relaxed) {
                tracing::info!(window_id = webview_id, "Ctrl-C: event loop exit");
                *control = ControlFlow::Exit;
                return;
            }

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    tracing::info!(window_id = webview_id, "window close requested");
                    // `Box<dyn WebViewWindow>`'un `close()` methodu WebView
                    // dispose eder + tao::Window'u yok eder; gerçek kapatma
                    // `Drop` ile olur. Burada sadece event loop'a exit sinyali
                    // veriyoruz.
                    if let Err(err) = window.close() {
                        tracing::warn!(?err, "WebViewWindow::close hata (devam ediliyor)");
                    }
                    *control = ControlFlow::Exit;
                }
                Event::WindowEvent {
                    event: WindowEvent::Destroyed,
                    ..
                } => {
                    tracing::debug!(window_id = webview_id, "window destroyed");
                    // Pencere destroy edildi → loop'tan çık. (Normal close
                    // path'te CloseRequested önce gelir; bu sadece OOM/crash
                    // gibi beklenmedik durumlar için.)
                    *control = ControlFlow::Exit;
                }
                _ => {
                    // Diğer event'ler (mouse, keyboard, resize, scale_factor
                    // changed, vb.) Faz 5.0 native UI ve Faz 6.0 hotkey'lerde
                    // handle edilecek. Şimdilik consume edip ignore.
                }
            }
        },
    );
}
