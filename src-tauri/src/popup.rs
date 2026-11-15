//! The reminder popup (CAP-2, CAP-3): a small always-on-top window that never
//! takes keyboard focus. Built once, hidden, at startup; shown next to the tray
//! on the display holding the cursor.

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "popup";
/// Logical size of the popup.
const SIZE: (f64, f64) = (340.0, 150.0);
/// Logical gap between the popup and the work-area edge.
const MARGIN: f64 = 12.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // each OS uses one corner
pub enum Corner {
    /// Under the macOS menu bar, where the tray title sits.
    TopRight,
    /// Above the Windows taskbar, next to the tray.
    BottomRight,
}

#[cfg(target_os = "macos")]
const CORNER: Corner = Corner::TopRight;
#[cfg(not(target_os = "macos"))]
const CORNER: Corner = Corner::BottomRight;

/// Physical top-left position for the popup inside a monitor's work area.
pub fn popup_position(
    area_pos: (i32, i32),
    area_size: (u32, u32),
    size: (f64, f64),
    scale: f64,
    corner: Corner,
) -> (i32, i32) {
    let (w, h) = (size.0 * scale, size.1 * scale);
    let margin = MARGIN * scale;
    let x = area_pos.0 as f64 + area_size.0 as f64 - w - margin;
    let y = match corner {
        Corner::TopRight => area_pos.1 as f64 + margin,
        Corner::BottomRight => area_pos.1 as f64 + area_size.1 as f64 - h - margin,
    };
    (x.round() as i32, y.round() as i32)
}

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let _win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popup.html".into()))
        .title("Hydration Reminder")
        .inner_size(SIZE.0, SIZE.1)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        // macOS: can't become key window. Windows: WS_EX_NOACTIVATE.
        .focusable(false)
        // The first click on the inactive popup presses the button.
        .accept_first_mouse(true)
        .visible_on_all_workspaces(true)
        .build()?;
    #[cfg(target_os = "macos")]
    {
        mac::show_over_full_screen(&_win);
        mac::watch_frontmost();
    }
    Ok(())
}

/// Shows the popup without activating the app. Call on the main thread.
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window(LABEL) else {
        eprintln!("[hydration] warning: popup window missing");
        return;
    };
    if let Err(e) = place(app, &win) {
        eprintln!("[hydration] warning: popup placement failed: {e}");
    }
    let shown = {
        #[cfg(windows)]
        {
            win32::show_no_activate(&win)
        }
        #[cfg(not(windows))]
        {
            win.show()
        }
    };
    if let Err(e) = shown {
        eprintln!("[hydration] warning: popup show failed: {e}");
    }
}

/// Hides the popup. Call on the main thread.
pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    let hidden = {
        #[cfg(windows)]
        {
            win32::hide(&win)
        }
        #[cfg(not(windows))]
        {
            win.hide()
        }
    };
    if let Err(e) = hidden {
        eprintln!("[hydration] warning: popup hide failed: {e}");
    }
}

/// After a button press: if the click activated our app, give focus back to
/// the app the user was in. Call on the main thread.
pub fn restore_focus() {
    #[cfg(target_os = "macos")]
    mac::restore_frontmost();
}

fn place<R: Runtime>(app: &AppHandle<R>, win: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
    // The cursor's display; any failure falls back to the primary display.
    let under_cursor = app
        .cursor_position()
        .ok()
        .and_then(|c| app.monitor_from_point(c.x, c.y).ok().flatten());
    let monitor = match under_cursor {
        Some(m) => Some(m),
        None => app.primary_monitor()?,
    };
    let Some(monitor) = monitor else {
        return Ok(()); // no monitor info: leave it where it is
    };
    let area = monitor.work_area();
    let (x, y) = popup_position(
        (area.position.x, area.position.y),
        (area.size.width, area.size.height),
        SIZE,
        monitor.scale_factor(),
        CORNER,
    );
    // macOS converts a physical position with the window's *current* scale,
    // which is wrong when moving between displays of different scales, so give
    // it points in the target display's scale. Windows uses global pixels.
    #[cfg(target_os = "macos")]
    let pos: tauri::Position = {
        let scale = monitor.scale_factor();
        tauri::LogicalPosition::new(x as f64 / scale, y as f64 / scale).into()
    };
    // Windows puts an invisible resize border around the window, so
    // `set_position` (outer frame) would leave the visible popup too close to
    // the edge. Shift by the outer-to-content offset so the content lands at x, y.
    #[cfg(not(target_os = "macos"))]
    let pos: tauri::Position = {
        let (dx, dy) = match (win.inner_position(), win.outer_position()) {
            (Ok(i), Ok(o)) => (i.x - o.x, i.y - o.y),
            _ => (0, 0),
        };
        tauri::PhysicalPosition::new(x - dx, y - dy).into()
    };
    win.set_position(pos)
}

#[cfg(windows)]
mod win32 {
    //! tao's `show()` uses `SW_SHOW`, which activates the window, and its
    //! visibility flag would then be out of step, so both show and hide go
    //! through Win32 directly.
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SetWindowPos, ShowWindow,
    };

    fn hwnd<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>) -> tauri::Result<HWND> {
        Ok(HWND(win.hwnd()?.0))
    }

    pub fn show_no_activate<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
        let h = hwnd(win)?;
        unsafe {
            let _ = ShowWindow(h, SW_SHOWNOACTIVATE);
            SetWindowPos(
                h,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )
            .map_err(|e| tauri::Error::Anyhow(e.into()))?;
        }
        Ok(())
    }

    pub fn hide<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>) -> tauri::Result<()> {
        let h = hwnd(win)?;
        unsafe {
            let _ = ShowWindow(h, SW_HIDE);
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod mac {
    //! Clicking a button in the popup activates our app, which would leave
    //! keyboard focus nowhere once the popup hides. So keep track of the last
    //! other app that was active, and hand focus back to it after the click.
    use block2::RcBlock;
    use objc2_app_kit::{
        NSApplicationActivationOptions, NSRunningApplication, NSWindow, NSWindowCollectionBehavior,
        NSWorkspace, NSWorkspaceDidActivateApplicationNotification,
    };
    use objc2_foundation::NSNotification;
    use std::ptr::NonNull;
    use std::sync::Mutex;

    static PREVIOUS: Mutex<Option<i32>> = Mutex::new(None);

    /// "All Spaces" alone doesn't cover another app's full-screen Space; the
    /// popup also needs to be a full-screen auxiliary window. Main thread only.
    pub fn show_over_full_screen<R: tauri::Runtime>(win: &tauri::WebviewWindow<R>) {
        let Ok(ptr) = win.ns_window() else {
            eprintln!("[hydration] warning: no NSWindow for the popup");
            return;
        };
        // SAFETY: Tauri returns the live NSWindow of this window, and we're on
        // the main thread (called from setup).
        let ns_window = unsafe { &*(ptr as *const NSWindow) };
        let behavior =
            ns_window.collectionBehavior() | NSWindowCollectionBehavior::FullScreenAuxiliary;
        ns_window.setCollectionBehavior(behavior);
    }

    fn record_frontmost() {
        let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
            return;
        };
        let pid = app.processIdentifier();
        if pid != std::process::id() as i32 {
            *PREVIOUS.lock().unwrap_or_else(|e| e.into_inner()) = Some(pid);
        }
    }

    /// Starts tracking app activations. Call once, on the main thread.
    pub fn watch_frontmost() {
        record_frontmost();
        let block = RcBlock::new(|_: NonNull<NSNotification>| record_frontmost());
        let center = NSWorkspace::sharedWorkspace().notificationCenter();
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(NSWorkspaceDidActivateApplicationNotification),
                None,
                None,
                &block,
            )
        };
        // The observer lives as long as the app.
        std::mem::forget(token);
    }

    pub fn restore_frontmost() {
        if !NSRunningApplication::currentApplication().isActive() {
            return; // focus never left the other app
        }
        let Some(pid) = *PREVIOUS.lock().unwrap_or_else(|e| e.into_inner()) else {
            return;
        };
        if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
            #[allow(deprecated)]
            app.activateWithOptions(NSApplicationActivationOptions::empty());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_right_sits_under_the_menu_bar() {
        // 1512x949 work area starting below a 33 px menu bar, Retina scale 2.
        let (x, y) = popup_position((0, 66), (3024, 1898), SIZE, 2.0, Corner::TopRight);
        assert_eq!((x, y), (3024 - 680 - 24, 66 + 24));
    }

    #[test]
    fn bottom_right_sits_above_the_taskbar() {
        // 1920x1080 display with a 48 px taskbar, scale 1.
        let (x, y) = popup_position((0, 0), (1920, 1032), SIZE, 1.0, Corner::BottomRight);
        assert_eq!((x, y), (1920 - 340 - 12, 1032 - 150 - 12));
    }

    #[test]
    fn second_monitor_offset_is_respected() {
        // Secondary display to the left of the primary, scale 1.5.
        let (x, y) = popup_position((-2560, 0), (2560, 1392), SIZE, 1.5, Corner::BottomRight);
        assert_eq!((x, y), (-2560 + 2560 - 510 - 18, 1392 - 225 - 18));
    }
}
