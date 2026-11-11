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

pub struct Tray<R: Runtime> {
    icon: TrayIcon<R>,
    header: MenuItem<R>,
}

impl<R: Runtime> Tray<R> {
    pub fn build(app: &AppHandle<R>) -> tauri::Result<Self> {
        let header = MenuItem::with_id(app, "header", header_text(0, false), false, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&header, &PredefinedMenuItem::separator(app)?, &quit])?;

        let builder = TrayIconBuilder::with_id("main")
            .menu(&menu)
            .show_menu_on_left_click(true)
            .on_menu_event(|app, event| {
                if event.id().as_ref() == "quit" {
                    app.exit(0);
                }
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
        })
    }

    pub fn render(&self, count: u32, prompting: bool) {
        let result = (|| -> tauri::Result<()> {
            self.header.set_text(header_text(count, prompting))?;
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
            eprintln!("[hydration] warning: tray update failed: {e}");
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
}
