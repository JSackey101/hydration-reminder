//! The settings window (CAP-5, CAP-7, CAP-14): an ordinary window for the
//! interval, the popup timeout and launch at login. Built when "Settings…" is
//! chosen and closed for real, so it costs nothing while it isn't open.

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "settings";

/// Opens the settings window, or brings it to the front if it's already open.
/// Takes no state lock, so it may run on the main thread (tray menu handler).
pub fn open<R: Runtime>(app: &AppHandle<R>) {
    let result = match app.get_webview_window(LABEL) {
        Some(win) => win
            .unminimize()
            .and_then(|_| win.show())
            .and_then(|_| win.set_focus()),
        None => WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings.html".into()))
            .title("Hydration Reminder Settings")
            .inner_size(420.0, 264.0)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .center()
            .focused(true)
            .build()
            // `focused(true)` alone leaves a new window behind the app in use
            // on macOS (found in the Mac test); `set_focus` brings it forward.
            .and_then(|win| win.set_focus()),
    };
    match result {
        Err(e) => log::warn!("settings window failed: {e}"),
        // A menu-bar-only (Accessory) app isn't activated by a new window, so
        // the window would open behind the app in use.
        #[cfg(target_os = "macos")]
        Ok(()) => {
            use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
            #[allow(deprecated)] // same call as popup::mac; `activate()` needs macOS 14
            NSRunningApplication::currentApplication()
                .activateWithOptions(NSApplicationActivationOptions::empty());
        }
        #[cfg(not(target_os = "macos"))]
        Ok(()) => {}
    }
}
