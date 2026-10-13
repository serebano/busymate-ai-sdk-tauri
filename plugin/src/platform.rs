//! Renderer (web content process) recovery for chat windows.
//!
//! - Windows: WebView2 `ProcessFailed` (render process exited or unresponsive).
//! - Linux: WebKitGTK `web-process-terminated`.
//! - macOS: WebKit's termination callback is app-level in Tauri; forward it
//!   with [`crate::web_content_process_terminated`] from
//!   `Builder::on_web_content_process_terminate`.
//!
//! In every case the chat reloads and receives `restored` after its next `hello`.

use tauri::{Runtime, WebviewWindow};

use crate::BusymateAi;

#[cfg(windows)]
pub(crate) fn install_renderer_recovery<R: Runtime>(window: &WebviewWindow<R>, sdk: BusymateAi<R>) {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
        COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE,
    };
    use webview2_com::ProcessFailedEventHandler;

    let label = window.label().to_string();
    let _ = window.with_webview(move |platform| unsafe {
        let Ok(core) = platform.controller().CoreWebView2() else { return };
        let mut token = Default::default();
        let handler = ProcessFailedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else { return Ok(()) };
            let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
            args.ProcessFailedKind(&mut kind)?;
            if kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
                || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE
            {
                // Reloading from inside the WebView2 callback is what WebView2 documents;
                // the plugin's reload is dispatched through the event loop.
                sdk.renderer_gone(&label);
            }
            Ok(())
        }));
        let _ = core.add_ProcessFailed(&handler, &mut token);
    });
}

#[cfg(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
pub(crate) fn install_renderer_recovery<R: Runtime>(window: &WebviewWindow<R>, sdk: BusymateAi<R>) {
    use webkit2gtk::WebViewExt;

    let label = window.label().to_string();
    let _ = window.with_webview(move |platform| {
        platform.inner().connect_web_process_terminated(move |_webview, _reason| {
            sdk.renderer_gone(&label);
        });
    });
}

#[cfg(not(any(
    windows,
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
pub(crate) fn install_renderer_recovery<R: Runtime>(_window: &WebviewWindow<R>, _sdk: BusymateAi<R>) {}
