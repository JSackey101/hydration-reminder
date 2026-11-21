//! Global shortcuts for Drank and Snooze (CAP-15). They're registered only
//! while a reminder shows, so the key combos reach other apps the rest of the
//! time.
//!
//! Threading: `register` and `release` run on the main thread, from `apply`'s
//! popup task, and never take the `Core` lock. Only single `on_shortcut` /
//! `unregister` calls are used: the plugin's `register_multiple`,
//! `on_shortcuts` and `unregister_all` hold its lock while waiting on the main
//! thread, which deadlocks against a key press dispatched there.

use crate::scheduler::{Answer, DEFAULT_DRANK_SHORTCUT, DEFAULT_SNOOZE_SHORTCUT};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// What the popup is told when it loads and each time the keys are registered.
pub const LABELS_EVENT: &str = "shortcut-labels";

/// The shortcuts registered for the reminder on screen. Main thread only.
#[derive(Default)]
pub struct Registered(Mutex<Vec<Shortcut>>);

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Labels {
    pub drank: String,
    pub snooze: String,
}

impl Labels {
    pub fn new(drank: &Shortcut, snooze: &Shortcut) -> Self {
        let macos = cfg!(target_os = "macos");
        Self {
            drank: label(drank, macos),
            snooze: label(snooze, macos),
        }
    }
}

/// The stored shortcuts if they pass `validate`, else both defaults (with a
/// warning). Validating, not just parsing, keeps a hand-edited bare key, or two
/// equal combos, from ever being registered.
pub fn stored_or_defaults(drank: &str, snooze: &str) -> (Shortcut, Shortcut) {
    let (d, s) = validate(drank, snooze).unwrap_or_else(|e| {
        log::warn!("stored shortcuts {drank:?} / {snooze:?} are invalid ({e}); using the defaults");
        (
            DEFAULT_DRANK_SHORTCUT.into(),
            DEFAULT_SNOOZE_SHORTCUT.into(),
        )
    });
    (
        d.parse().expect("validated shortcut parses"),
        s.parse().expect("validated shortcut parses"),
    )
}

/// Which answer each shortcut gives.
pub fn bindings(drank: Shortcut, snooze: Shortcut) -> [(Shortcut, Answer); 2] {
    [(drank, Answer::Drank), (snooze, Answer::Snooze)]
}

#[cfg(target_os = "macos")]
const NEEDS: &str = "needs ⌃ or ⌘";
#[cfg(not(target_os = "macos"))]
const NEEDS: &str = "needs Ctrl or Win";

fn check(s: &str, what: &str) -> Result<Shortcut, String> {
    let sc: Shortcut = s
        .parse()
        .map_err(|_| format!("{what}: that key can't be used in a shortcut."))?;
    // Alt and Shift alone are refused: Shift-only would block typing, and
    // macOS 15 refuses Option-only combos.
    if !sc.mods.intersects(Modifiers::CONTROL | Modifiers::SUPER) {
        return Err(format!("{what}: a shortcut {NEEDS}."));
    }
    Ok(sc)
}

/// Both recorded combos, checked: the canonical strings to store, or the
/// message the settings page shows.
pub fn validate(drank: &str, snooze: &str) -> Result<(String, String), String> {
    let d = check(drank, "Drank")?;
    let s = check(snooze, "Snooze")?;
    if d.id() == s.id() {
        return Err("Drank and Snooze need different shortcuts.".into());
    }
    Ok((d.into_string(), s.into_string()))
}

/// How a shortcut is shown: "⌃⌥⇧⌘D" on macOS, "Ctrl+Alt+Shift+Win+D" elsewhere.
pub fn label(sc: &Shortcut, macos: bool) -> String {
    let order = [
        (Modifiers::CONTROL, "⌃", "Ctrl"),
        (Modifiers::ALT, "⌥", "Alt"),
        (Modifiers::SHIFT, "⇧", "Shift"),
        (Modifiers::SUPER, "⌘", "Win"),
    ];
    let mut parts: Vec<String> = order
        .iter()
        .filter(|(m, ..)| sc.mods.contains(*m))
        .map(|(_, mac, win)| if macos { *mac } else { *win }.to_string())
        .collect();
    parts.push(key_name(&sc.key.to_string()));
    parts.join(if macos { "" } else { "+" })
}

fn key_name(code: &str) -> String {
    if let Some(c) = code.strip_prefix("Key").filter(|c| c.len() == 1) {
        return c.into();
    }
    if let Some(d) = code.strip_prefix("Digit").filter(|d| d.len() == 1) {
        return d.into();
    }
    let punct = match code {
        "Backquote" => "`",
        "Backslash" => "\\",
        "BracketLeft" => "[",
        "BracketRight" => "]",
        "Comma" => ",",
        "Equal" => "=",
        "Minus" => "-",
        "Period" => ".",
        "Quote" => "'",
        "Semicolon" => ";",
        "Slash" => "/",
        // F1–F24 and every other key show their code name.
        other => other,
    };
    punct.into()
}

/// Sends the popup its key hints.
pub fn send_labels<R: Runtime>(app: &AppHandle<R>, labels: &Labels) {
    if let Err(e) = app.emit_to(crate::popup::POPUP.label, LABELS_EVENT, labels) {
        log::warn!("shortcut labels not sent: {e}");
    }
}

/// Registers both shortcuts for the reminder now on screen. Main thread only.
/// A combo that won't register is logged; the popup buttons still work.
/// Returns the labels of what works: empty for a combo that didn't register,
/// so the popup doesn't advertise a dead key.
pub fn register<R: Runtime>(app: &AppHandle<R>, drank: Shortcut, snooze: Shortcut) -> Labels {
    let plugin = app.global_shortcut();
    let registered = app.state::<Registered>();
    let mut list = registered.0.lock().unwrap_or_else(|e| e.into_inner());
    let mut live = Labels::new(&drank, &snooze);
    for (sc, answer) in bindings(drank, snooze) {
        let result = plugin.on_shortcut(sc, move |app, _, event| {
            // Windows sends Released from another thread; only Pressed counts.
            if event.state == ShortcutState::Pressed {
                let app = app.clone();
                // Off the main thread: `apply` takes the `Core` lock.
                tauri::async_runtime::spawn_blocking(move || {
                    crate::answer_by_shortcut(&app, answer)
                });
            }
        });
        match result {
            Ok(()) => list.push(sc),
            Err(e) => {
                log::warn!("can't register {answer:?} shortcut {sc}: {e}");
                match answer {
                    Answer::Drank => live.drank.clear(),
                    Answer::Snooze => live.snooze.clear(),
                }
            }
        }
    }
    live
}

/// Releases exactly what `register` registered. Main thread only.
pub fn release<R: Runtime>(app: &AppHandle<R>) {
    let registered = app.state::<Registered>();
    let list = std::mem::take(&mut *registered.0.lock().unwrap_or_else(|e| e.into_inner()));
    for sc in list {
        if let Err(e) = app.global_shortcut().unregister(sc) {
            log::warn!("can't release shortcut {sc}: {e}");
        }
    }
}

/// Checks a combo can be registered by registering and releasing it at once.
/// Only Windows reports a combo owned by another app; macOS accepts it and the
/// other app wins. Must not run on the main thread while it is blocked.
pub fn try_register<R: Runtime>(app: &AppHandle<R>, sc: Shortcut) -> Result<(), String> {
    let plugin = app.global_shortcut();
    plugin
        .on_shortcut(sc, |_, _, _| {})
        .map_err(|e| e.to_string())?;
    // Refuse rather than save if it can't be released: a stuck test
    // registration would take the key from other apps.
    plugin
        .unregister(sc)
        .map_err(|e| format!("can't release test registration: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::Settings;

    fn sc(s: &str) -> Shortcut {
        s.parse().unwrap()
    }

    #[test]
    fn defaults_parse_and_are_canonical() {
        for d in [DEFAULT_DRANK_SHORTCUT, DEFAULT_SNOOZE_SHORTCUT] {
            assert_eq!(sc(d).into_string(), d);
        }
    }

    #[test]
    fn validate_accepts_and_canonicalises() {
        assert_eq!(
            validate("Alt+Control+Shift+KeyW", "super+KeyS"),
            Ok(("shift+control+alt+KeyW".into(), "super+KeyS".into()))
        );
        assert_eq!(
            validate(DEFAULT_DRANK_SHORTCUT, DEFAULT_SNOOZE_SHORTCUT),
            Ok((
                DEFAULT_DRANK_SHORTCUT.into(),
                DEFAULT_SNOOZE_SHORTCUT.into()
            ))
        );
    }

    #[test]
    fn validate_refuses_each_bad_case_with_a_reason() {
        let ok = DEFAULT_SNOOZE_SHORTCUT;
        for (drank, why) in [
            ("KeyD", "needs"),
            ("shift+KeyD", "needs"),
            ("alt+KeyD", "needs"),
            ("alt+shift+KeyD", "needs"),
            ("control+NoSuchKey", "can't be used"),
            ("", "can't be used"),
        ] {
            let msg = validate(drank, ok).unwrap_err();
            assert!(
                msg.starts_with("Drank:") && msg.contains(why),
                "{drank:?}: {msg}"
            );
        }
        let msg = validate(DEFAULT_DRANK_SHORTCUT, "shift+KeyS").unwrap_err();
        assert!(msg.starts_with("Snooze:"), "{msg}");
    }

    #[test]
    fn validate_refuses_the_same_combo_by_id_not_spelling() {
        let msg = validate("control+alt+KeyD", "Alt+Ctrl+D").unwrap_err();
        assert!(msg.contains("different"), "{msg}");
    }

    #[test]
    fn labels_for_both_systems() {
        let all = sc("shift+control+alt+super+KeyD");
        assert_eq!(label(&all, true), "⌃⌥⇧⌘D");
        assert_eq!(label(&all, false), "Ctrl+Alt+Shift+Win+D");
        assert_eq!(label(&sc(DEFAULT_DRANK_SHORTCUT), true), "⌃⌥D");
        assert_eq!(label(&sc(DEFAULT_SNOOZE_SHORTCUT), false), "Ctrl+Alt+S");
        assert_eq!(label(&sc("control+Digit7"), false), "Ctrl+7");
        assert_eq!(label(&sc("control+F12"), true), "⌃F12");
        assert_eq!(label(&sc("control+Slash"), true), "⌃/");
        assert_eq!(label(&sc("control+BracketLeft"), false), "Ctrl+[");
        assert_eq!(label(&sc("control+Space"), false), "Ctrl+Space");
        assert_eq!(label(&sc("control+ArrowUp"), true), "⌃ArrowUp");
    }

    #[test]
    fn bad_stored_values_fall_back_to_both_defaults() {
        let defaults = (sc(DEFAULT_DRANK_SHORTCUT), sc(DEFAULT_SNOOZE_SHORTCUT));
        for (d, s) in [
            ("control+", DEFAULT_SNOOZE_SHORTCUT), // unparsable
            ("KeyD", DEFAULT_SNOOZE_SHORTCUT),     // bare key (hand-edited)
            // Unparsable Drank whose default would equal the stored Snooze.
            ("control+", "control+alt+KeyD"),
            ("control+KeyW", "Ctrl+W"), // same combo
        ] {
            assert_eq!(stored_or_defaults(d, s), defaults, "{d:?} / {s:?}");
        }
        assert_eq!(
            stored_or_defaults("control+alt+KeyW", "super+KeyS"),
            (sc("control+alt+KeyW"), sc("super+KeyS"))
        );
    }

    #[test]
    fn each_shortcut_is_bound_to_its_own_answer() {
        let (d, s) = (sc(DEFAULT_DRANK_SHORTCUT), sc(DEFAULT_SNOOZE_SHORTCUT));
        assert_eq!(bindings(d, s), [(d, Answer::Drank), (s, Answer::Snooze)]);
    }

    #[test]
    fn settings_from_v0_1_0_load_the_default_shortcuts() {
        let old: Settings = serde_json::from_str(
            r#"{ "interval_min": 30, "snooze_min": 10, "popup_timeout_min": null }"#,
        )
        .unwrap();
        assert_eq!(old.interval_min, 30);
        assert_eq!(old.drank_shortcut, DEFAULT_DRANK_SHORTCUT);
        assert_eq!(old.snooze_shortcut, DEFAULT_SNOOZE_SHORTCUT);
    }
}
