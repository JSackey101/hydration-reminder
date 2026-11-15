//! Tray icon showing today's drink count (CAP-1).
//! macOS: menu-bar title "💧 N". Windows: drop icon with the count drawn on as a
//! badge (see `badge`), plus tooltip.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Runtime};

#[cfg(not(target_os = "macos"))]
use crate::badge::{ICON_SIZE, badge_rgba};

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn title_text(count: u32, prompting: bool) -> String {
    if prompting {
        format!("💧 {count} ⏰")
    } else {
        format!("💧 {count}")
    }
}

pub fn header_text(count: u32, prompting: bool) -> String {
    let drinks = if count == 1 { "drink" } else { "drinks" };
    if prompting {
        format!("{count} {drinks} today · reminder due")
    } else {
        format!("{count} {drinks} today")
    }
}

/// "Undo last drink" is offered only when today has a drink to undo (CAP-13).
pub fn undo_enabled(count: u32) -> bool {
    count > 0
}

pub struct Tray<R: Runtime> {
    icon: TrayIcon<R>,
    header: MenuItem<R>,
    undo: MenuItem<R>,
}

impl<R: Runtime> Tray<R> {
    pub fn build(app: &AppHandle<R>) -> tauri::Result<Self> {
        let header = MenuItem::with_id(app, "header", header_text(0, false), false, None::<&str>)?;
        let log = MenuItem::with_id(app, "log", "Log a drink", true, None::<&str>)?;
        let undo = MenuItem::with_id(app, "undo", "Undo last drink", false, None::<&str>)?;
        let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &header,
                &PredefinedMenuItem::separator(app)?,
                &log,
                &undo,
                &PredefinedMenuItem::separator(app)?,
                &settings,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;

        let builder = TrayIconBuilder::with_id("main")
            .menu(&menu)
            .show_menu_on_left_click(true)
            // Menu events arrive on the main thread. Log and Undo take the
            // state lock, and tray updates made under that lock wait for the
            // main thread, so they must run elsewhere or the app deadlocks.
            .on_menu_event(|app, event| match event.id().as_ref() {
                "log" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn_blocking(move || crate::tray_log_drink(&app));
                }
                "undo" => {
                    let app = app.clone();
                    tauri::async_runtime::spawn_blocking(move || crate::tray_undo(&app));
                }
                "settings" => crate::settings::open(app),
                "quit" => app.exit(0),
                _ => {}
            });
        #[cfg(target_os = "macos")]
        let builder = builder.title(title_text(0, false));
        #[cfg(not(target_os = "macos"))]
        let builder = builder
            .icon(tauri::image::Image::new_owned(
                badge_rgba(0, false),
                ICON_SIZE,
                ICON_SIZE,
            ))
            .tooltip(format!("💧 {}", header_text(0, false)));

        Ok(Self {
            icon: builder.build(app)?,
            header,
            undo,
        })
    }

    pub fn render(&self, count: u32, prompting: bool) {
        let result = (|| -> tauri::Result<()> {
            self.header.set_text(header_text(count, prompting))?;
            // Undo covers today's drinks only (CAP-13).
            self.undo.set_enabled(undo_enabled(count))?;
            #[cfg(target_os = "macos")]
            self.icon.set_title(Some(title_text(count, prompting)))?;
            #[cfg(not(target_os = "macos"))]
            {
                self.icon.set_icon(Some(tauri::image::Image::new_owned(
                    badge_rgba(count, prompting),
                    ICON_SIZE,
                    ICON_SIZE,
                )))?;
                self.icon
                    .set_tooltip(Some(format!("💧 {}", header_text(count, prompting))))?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            log::warn!("tray update failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts() {
        assert_eq!(title_text(5, false), "💧 5");
        assert_eq!(title_text(5, true), "💧 5 ⏰");
        assert_eq!(header_text(1, false), "1 drink today");
        assert_eq!(header_text(0, true), "0 drinks today · reminder due");
    }

    #[test]
    fn undo_only_with_a_drink_today() {
        assert!(!undo_enabled(0));
        assert!(undo_enabled(1));
    }
}
