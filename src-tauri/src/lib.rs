mod badge;
mod persist;
mod popup;
mod scheduler;
mod settings;
mod shortcuts;
mod tray;

use chrono::Local;
use scheduler::{Action, Answer, Phase, Settings, State};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::Shortcut;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const TICK: Duration = Duration::from_secs(1);

/// Tells the settings window whether a reminder is on screen, so it can
/// disable recording shortcuts.
const REMINDER_SHOWING_EVENT: &str = "reminder-showing";

/// Debug builds only: `HYDRATION_INTERVAL_SECS` shortens the interval for
/// testing. Held in memory, never saved.
fn interval_override() -> Option<u32> {
    if cfg!(debug_assertions) {
        std::env::var("HYDRATION_INTERVAL_SECS").ok()?.parse().ok()
    } else {
        None
    }
}

/// Everything the scheduler loop and the popup commands share. One lock, so
/// ticks and button presses apply in order and their effects (tray, popup,
/// save) are issued in the same order.
struct Core<R: Runtime>(Mutex<Inner<R>>);

struct Inner<R: Runtime> {
    state: State,
    saved: Option<State>,
    path: PathBuf,
    tray: tray::Tray<R>,
}

/// The shortcuts in use: the stored ones, or the defaults if they're invalid.
fn current_shortcuts(s: &Settings) -> (Shortcut, Shortcut) {
    shortcuts::stored_or_defaults(&s.drank_shortcut, &s.snooze_shortcut)
}

fn tray_view(s: &State) -> (u32, bool) {
    (s.count(), s.phase == Phase::Prompting)
}

/// The popup change a state change calls for: `Some(true)` = show it,
/// `Some(false)` = hide it, `None` = leave it as it is.
fn popup_change(prev: &State, next: &State) -> Option<bool> {
    let (was, is) = (
        prev.phase == Phase::Prompting,
        next.phase == Phase::Prompting,
    );
    (was != is).then_some(is)
}

/// What `apply` must do after a state change. Pure, so it's unit-tested.
#[derive(Debug, PartialEq, Eq)]
struct Effects {
    render_tray: bool,
    /// See `popup_change`.
    popup: Option<bool>,
    save: bool,
}

fn effects(prev: &State, next: &State, saved: Option<&State>) -> Effects {
    Effects {
        render_tray: tray_view(prev) != tray_view(next),
        popup: popup_change(prev, next),
        save: saved != Some(next),
    }
}

impl<R: Runtime> Inner<R> {
    fn render_tray(&self) {
        let (count, prompting) = tray_view(&self.state);
        self.tray.render(count, prompting);
    }
}

/// The single path for every state change. Effects follow the phase change,
/// not the action: entering Prompting shows the popup and registers the
/// shortcuts, leaving it hides the popup and releases them.
///
/// The lock is held while effects are issued so they keep their order. Popup
/// calls go through `run_on_main_thread`: queued from other threads, but run
/// inline when `apply` is already on the main thread (setup). So `popup::show`
/// and `popup::hide` (and `shortcuts::register` / `release`) must never take
/// the `Core` lock. Outside setup the main thread never takes it: commands are
/// async, so they run off the main thread, and the tray menu hands Log and
/// Undo to a worker thread. This matters because tray updates wait for the
/// main thread while the lock is held.
fn apply<R: Runtime>(app: &AppHandle<R>, f: impl FnOnce(State) -> (State, Action)) {
    let core = app.state::<Core<R>>();
    let mut g = core.0.lock().unwrap_or_else(|e| e.into_inner());
    let prev = g.state.clone();
    let (next, action) = f(prev.clone());
    log_action(action, &next);

    let fx = effects(&prev, &next, g.saved.as_ref());
    g.state = next;
    if fx.render_tray {
        g.render_tray();
    }
    if let Some(is) = fx.popup {
        let handle = app.clone();
        let (drank, snooze) = current_shortcuts(&g.state.settings);
        let queued = app.run_on_main_thread(move || {
            if is {
                // Hints first, so the popup never shows stale ones; then again
                // without any combo that failed to register.
                let labels = shortcuts::Labels::new(&drank, &snooze);
                shortcuts::send_labels(&handle, &labels);
                popup::show(&handle);
                let live = shortcuts::register(&handle, drank, snooze);
                if live != labels {
                    shortcuts::send_labels(&handle, &live);
                }
            } else {
                popup::hide(&handle);
                shortcuts::release(&handle);
            }
            if let Err(e) = handle.emit_to(settings::LABEL, REMINDER_SHOWING_EVENT, is) {
                log::warn!("reminder state not sent to settings: {e}");
            }
        });
        if let Err(e) = queued {
            log::warn!("popup update failed: {e}");
        }
    }

    if fx.save {
        match persist::save(&g.path, &g.state) {
            Ok(()) => g.saved = Some(g.state.clone()),
            Err(e) => log::warn!("save failed: {e}"),
        }
    }
}

/// Tick first, so a press just after midnight lands on the new day. `answer`
/// is a no-op unless still Prompting, so a press racing a click, a timeout or
/// a tray Log changes nothing. Returns whether it answered a reminder.
fn answer_reminder<R: Runtime>(app: &AppHandle<R>, answer: Answer) -> bool {
    let mut answered = false;
    apply(app, |s| {
        let now = Local::now();
        let (s, action) = scheduler::tick(now, s);
        answered = s.phase == Phase::Prompting;
        (scheduler::answer(now, s, answer), action)
    });
    answered
}

fn answer_popup<R: Runtime>(app: &AppHandle<R>, answer: Answer) {
    log::info!("answer: {answer:?}");
    answer_reminder(app, answer);
    if let Err(e) = app.run_on_main_thread(popup::restore_focus) {
        log::warn!("focus restore failed: {e}");
    }
}

/// A global shortcut (CAP-15), run off the main thread. Focus never left the
/// app in use, so unlike a click there's nothing to restore.
pub(crate) fn answer_by_shortcut<R: Runtime>(app: &AppHandle<R>, answer: Answer) {
    if answer_reminder(app, answer) {
        log::info!("answer by shortcut: {answer:?}");
    } else {
        log::info!("shortcut {answer:?} ignored: no reminder showing");
    }
}

/// Tray "Log a drink" and "Undo last drink". The tray menu handler runs these
/// off the main thread (see `apply`). Tick first, as for the popup buttons.
pub(crate) fn tray_log_drink<R: Runtime>(app: &AppHandle<R>) {
    log::info!("tray: log a drink");
    apply(app, |s| {
        let now = Local::now();
        let (s, action) = scheduler::tick(now, s);
        // A tick that just made the reminder due is answered by this log at
        // once, so no popup shows: don't log it as due.
        let action = match action {
            Action::ShowPopup => Action::None,
            a => a,
        };
        (scheduler::log_drink(now, s), action)
    });
}

pub(crate) fn tray_undo<R: Runtime>(app: &AppHandle<R>) {
    log::info!("tray: undo last drink");
    apply(app, |s| {
        let (s, action) = scheduler::tick(Local::now(), s);
        (scheduler::undo(s), action)
    });
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    interval_min: u32,
    popup_timeout_min: Option<u32>,
    drank_shortcut: String,
    snooze_shortcut: String,
    drank_label: String,
    snooze_label: String,
    reminder_showing: bool,
}

impl From<&State> for SettingsView {
    fn from(s: &State) -> Self {
        let (drank, snooze) = current_shortcuts(&s.settings);
        let labels = shortcuts::Labels::new(&drank, &snooze);
        Self {
            interval_min: s.settings.interval_min,
            popup_timeout_min: s.settings.popup_timeout_min,
            drank_shortcut: drank.into_string(),
            snooze_shortcut: snooze.into_string(),
            drank_label: labels.drank,
            snooze_label: labels.snooze,
            reminder_showing: s.phase == Phase::Prompting,
        }
    }
}

#[tauri::command]
async fn get_settings(app: AppHandle) -> SettingsView {
    let core = app.state::<Core<tauri::Wry>>();
    let g = core.0.lock().unwrap_or_else(|e| e.into_inner());
    SettingsView::from(&g.state)
}

/// For the popup when it loads; later changes arrive as `shortcut-labels`.
#[tauri::command]
async fn get_shortcut_labels(app: AppHandle) -> shortcuts::Labels {
    let core = app.state::<Core<tauri::Wry>>();
    let g = core.0.lock().unwrap_or_else(|e| e.into_inner());
    let (drank, snooze) = current_shortcuts(&g.state.settings);
    shortcuts::Labels::new(&drank, &snooze)
}

/// The settings page's values applied to the state. Takes i64 so a negative
/// or huge number gets the range message, not a serde error. On error the
/// state is returned unchanged.
fn change_settings(
    s: State,
    interval_min: i64,
    popup_timeout_min: Option<i64>,
) -> (State, Result<(), String>) {
    // Out of u32 range → 0, which `set_settings` rejects with its message.
    let to_u32 = |n: i64| u32::try_from(n).unwrap_or(0);
    match scheduler::set_settings(
        s.clone(),
        to_u32(interval_min),
        popup_timeout_min.map(to_u32),
    ) {
        Ok(next) => (next, Ok(())),
        Err(e) => (s, Err(e)),
    }
}

/// Applies on change (no Save button). An invalid value is refused with the
/// message the page shows, and nothing is saved.
#[tauri::command]
async fn save_settings(
    app: AppHandle,
    interval_min: i64,
    popup_timeout_min: Option<i64>,
) -> Result<(), String> {
    let mut result = Ok(());
    apply(&app, |s| {
        let (next, r) = change_settings(s, interval_min, popup_timeout_min);
        result = r;
        (next, Action::None)
    });
    if result.is_ok() {
        log::info!("settings: interval {interval_min} min, timeout {popup_timeout_min:?}");
    }
    result
}

/// Recorded shortcuts applied to the state: refused while a reminder shows,
/// then validated, then each changed one test-registered with `try_register`
/// (an unchanged one that has since been taken mustn't block fixing the
/// other). On error the state is returned unchanged.
fn change_shortcuts(
    s: State,
    drank: &str,
    snooze: &str,
    mut try_register: impl FnMut(Shortcut) -> Result<(), String>,
) -> (State, Result<(), String>) {
    if s.phase == Phase::Prompting {
        return (s, Err("Answer the reminder first.".into()));
    }
    let (drank, snooze) = match shortcuts::validate(drank, snooze) {
        Ok(v) => v,
        Err(e) => return (s, Err(e)),
    };
    let old = (
        s.settings.drank_shortcut.clone(),
        s.settings.snooze_shortcut.clone(),
    );
    for (what, combo, old) in [("Drank", &drank, &old.0), ("Snooze", &snooze, &old.1)] {
        if combo == old {
            continue;
        }
        let sc: Shortcut = combo.parse().expect("validated shortcut parses");
        if let Err(e) = try_register(sc) {
            log::warn!("{what} shortcut {combo} can't be registered: {e}");
            let label = shortcuts::label(&sc, cfg!(target_os = "macos"));
            return (
                s,
                Err(format!(
                    "{what}: {label} can't be registered; another app may be using it."
                )),
            );
        }
    }
    let mut next = s;
    next.settings.drank_shortcut = drank;
    next.settings.snooze_shortcut = snooze;
    (next, Ok(()))
}

/// Runs inside `apply`, so the Prompting check and the save happen under one
/// lock and no reminder can appear in between. The test registration waits on
/// the main thread, which never takes the lock (see `apply`).
#[tauri::command]
async fn save_shortcuts(app: AppHandle, drank: String, snooze: String) -> Result<(), String> {
    let mut result = Ok(());
    apply(&app, |s| {
        let (next, r) =
            change_shortcuts(s, &drank, &snooze, |sc| shortcuts::try_register(&app, sc));
        result = r;
        (next, Action::None)
    });
    match &result {
        Ok(()) => log::info!("settings: shortcuts {drank} / {snooze}"),
        Err(e) => log::info!("shortcuts refused: {e}"),
    }
    result
}

/// Marker written once the first launch of a release build has turned on
/// launch at login, so the default is applied once and a later "off" sticks.
const AUTOSTART_MARKER: &str = "autostart-initialised";

/// What a launch does about launch at login. Release builds only: a debug run
/// never registers on its own (the Settings checkbox still works there).
#[derive(Debug, PartialEq)]
enum AutostartStep {
    Nothing,
    /// First launch: turn it on, then write the marker.
    EnableAndMark,
    /// Already on: enable again, so the login entry points at this launch's
    /// executable. Fixes an entry saved while the app ran from the mounted
    /// .dmg or a quarantine (App Translocation) path once it runs from
    /// Applications.
    Refresh,
}

fn autostart_step(is_debug: bool, marker_exists: bool, is_enabled: bool) -> AutostartStep {
    match (is_debug, marker_exists, is_enabled) {
        (true, _, _) => AutostartStep::Nothing,
        (false, false, _) => AutostartStep::EnableAndMark,
        (false, true, true) => AutostartStep::Refresh,
        (false, true, false) => AutostartStep::Nothing,
    }
}

/// Writes the marker: the first-launch default no longer applies, either
/// because it has been applied or because the user chose in Settings (#18).
fn settle_autostart_default(marker: &std::path::Path) {
    if let Err(e) = std::fs::write(marker, b"") {
        log::warn!("can't write {} ({e})", marker.display());
    }
}

/// Carries out `step`. The marker is written only after a successful enable,
/// so a failed first enable is retried on the next launch.
fn run_autostart_step<E: std::fmt::Display>(
    step: AutostartStep,
    marker: &std::path::Path,
    enable: impl FnOnce() -> Result<(), E>,
) {
    if step == AutostartStep::Nothing {
        return;
    }
    match enable() {
        Ok(()) if step == AutostartStep::EnableAndMark => {
            log::info!("launch at login turned on (first launch)");
            settle_autostart_default(marker);
        }
        Ok(()) => {}
        Err(e) => log::warn!("can't turn on launch at login: {e}"),
    }
}

fn default_autostart<R: Runtime>(app: &AppHandle<R>, dir: &std::path::Path) {
    use tauri_plugin_autostart::ManagerExt;
    let marker = dir.join(AUTOSTART_MARKER);
    let manager = app.autolaunch();
    let is_debug = cfg!(debug_assertions);
    // Only asked when it matters, so a debug run never touches the entry.
    let marker_exists = marker.exists();
    let is_enabled = !is_debug && marker_exists && manager.is_enabled().unwrap_or(false);
    run_autostart_step(
        autostart_step(is_debug, marker_exists, is_enabled),
        &marker,
        || manager.enable(),
    );
}

/// The settings window reads the OS login entry each time; nothing is cached.
#[tauri::command]
async fn get_launch_at_login(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| {
        log::warn!("can't read launch at login: {e}");
        format!("Couldn't read the login setting: {e}")
    })
}

#[tauri::command]
async fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    match result {
        Ok(()) => {
            log::info!(
                "settings: launch at login {}",
                if enabled { "on" } else { "off" }
            );
            // An explicit choice wins over the first-launch default, even if
            // that default failed and never wrote the marker (#18).
            match app.path().app_data_dir() {
                Ok(dir) => settle_autostart_default(&dir.join(AUTOSTART_MARKER)),
                Err(e) => log::warn!("no app data folder for the autostart marker: {e}"),
            }
            Ok(())
        }
        Err(e) => {
            log::warn!(
                "can't turn launch at login {}: {e}",
                if enabled { "on" } else { "off" }
            );
            Err(format!(
                "Couldn't turn {} start at login: {e}",
                if enabled { "on" } else { "off" }
            ))
        }
    }
}

// Async so they run off the main thread (see `apply`).
#[tauri::command]
async fn drank(app: AppHandle) {
    answer_popup(&app, Answer::Drank);
}

#[tauri::command]
async fn snooze(app: AppHandle) {
    answer_popup(&app, Answer::Snooze);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Registered first, as the plugin requires: a second launch (e.g.
        // autostart plus a manual start) exits at once, so two schedulers
        // never share state.json.
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        // Next, so warnings from the other plugins and setup are logged. The
        // file matters in Windows release builds, which have no console (#10).
        .plugin(
            tauri_plugin_log::Builder::new()
                .clear_targets()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                ])
                .level(log::LevelFilter::Info)
                .timezone_strategy(TimezoneStrategy::UseLocal)
                .max_file_size(1_000_000)
                // The current file plus up to two rotated ones (about 3 MB).
                .rotation_strategy(RotationStrategy::KeepSome(2))
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        // No global handler: each shortcut gets its own, registered only
        // while a reminder shows (see `shortcuts`).
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            drank,
            snooze,
            get_settings,
            save_settings,
            save_shortcuts,
            get_shortcut_labels,
            get_launch_at_login,
            set_launch_at_login
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let path = dir.join("state.json");
            default_autostart(app.handle(), &dir);

            let mut state = scheduler::on_load(persist::load(&path, Local::now()));
            state.interval_override_secs = interval_override();
            if let Some(secs) = state.interval_override_secs {
                log::info!("debug build: interval overridden to {secs} s");
            }

            let tray = tray::Tray::build(app.handle())?;
            popup::build(app.handle())?;
            app.manage(shortcuts::Registered::default());
            app.manage(Core(Mutex::new(Inner {
                state,
                saved: None,
                path,
                tray,
            })));

            // Tick once before the first render, so a new day never shows
            // yesterday's count, even briefly.
            apply(app.handle(), |s| scheduler::tick(Local::now(), s));
            // The tray was built showing 0; draw the real state once.
            app.state::<Core<tauri::Wry>>()
                .0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .render_tray();

            let handle = app.handle().clone();
            std::thread::spawn(move || run_scheduler(handle));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn log_action(action: Action, state: &State) {
    match action {
        Action::ShowPopup => log::info!("reminder due"),
        Action::ResetDay => log::info!("new day {}: count reset", state.day),
        Action::TimedOut => log::info!("popup timed out"),
        Action::None => {}
    }
}

/// Ticks once a second. Due times are absolute wall-clock instants, so this
/// also catches up straight after a wake from sleep.
fn run_scheduler<R: Runtime>(app: AppHandle<R>) {
    loop {
        apply(&app, |s| scheduler::tick(Local::now(), s));
        std::thread::sleep(TICK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone, Utc};

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 1, h, m, 0).unwrap()
    }

    /// Waiting, due at 9:45.
    fn waiting() -> State {
        State::new(at(9, 0))
    }

    fn prompting() -> State {
        scheduler::tick(at(9, 45), waiting()).0
    }

    #[test]
    fn due_reminder_shows_popup() {
        let (next, _) = scheduler::tick(at(9, 45), waiting());
        assert_eq!(popup_change(&waiting(), &next), Some(true));
    }

    #[test]
    fn drank_snooze_and_timeout_hide_popup() {
        let prev = prompting();
        for answer in [Answer::Drank, Answer::Snooze] {
            let next = scheduler::answer(at(9, 46), prev.clone(), answer);
            assert_eq!(popup_change(&prev, &next), Some(false), "{answer:?}");
        }
        let mut timed = prev.clone();
        timed.settings.popup_timeout_min = Some(2);
        let (next, action) = scheduler::tick(at(9, 47), timed.clone());
        assert_eq!(action, Action::TimedOut);
        assert_eq!(popup_change(&timed, &next), Some(false));
    }

    #[test]
    fn no_popup_change_without_a_phase_change() {
        // Ordinary tick while waiting.
        let (next, _) = scheduler::tick(at(9, 10), waiting());
        assert_eq!(popup_change(&waiting(), &next), None);
        // Still prompting, a minute later.
        let (next, _) = scheduler::tick(at(9, 46), prompting());
        assert_eq!(popup_change(&prompting(), &next), None);
        // Day rollover while prompting keeps the popup.
        let (next, action) = scheduler::tick(at(9, 45) + chrono::Duration::days(1), prompting());
        assert_eq!(action, Action::ResetDay);
        assert_eq!(popup_change(&prompting(), &next), None);
        // Double click: the second press lands in Waiting and changes nothing.
        let after = scheduler::answer(at(9, 46), prompting(), Answer::Drank);
        let again = scheduler::answer(at(9, 46), after.clone(), Answer::Drank);
        assert_eq!(popup_change(&after, &again), None);
    }

    #[test]
    fn effects_render_tray_only_when_its_view_changes() {
        let w = waiting();
        // Same count, same phase: nothing to draw.
        let (later, _) = scheduler::tick(at(9, 10), w.clone());
        assert!(!effects(&w, &later, Some(&later)).render_tray);
        // A drink changes the count.
        let logged = scheduler::log_drink(at(9, 10), w.clone());
        assert!(effects(&w, &logged, Some(&w)).render_tray);
        // Undo changes it back.
        let undone = scheduler::undo(logged.clone());
        assert!(effects(&logged, &undone, Some(&logged)).render_tray);
        // Becoming due shows the due marker.
        assert!(effects(&w, &prompting(), Some(&w)).render_tray);
        // A settings change alone doesn't touch the tray.
        let set = scheduler::set_settings(w.clone(), 30, Some(2)).unwrap();
        assert!(!effects(&w, &set, Some(&w)).render_tray);
    }

    #[test]
    fn effects_save_whenever_state_differs_from_saved() {
        let w = waiting();
        // Never saved yet (first apply after launch): save.
        assert!(effects(&w, &w, None).save);
        // Unchanged since the last save: no write.
        assert!(!effects(&w, &w, Some(&w)).save);
        // A settings change is saved even though the tray doesn't change.
        let set = scheduler::set_settings(w.clone(), 30, None).unwrap();
        assert!(effects(&w, &set, Some(&w)).save);
        // A failed save leaves `saved` behind, so the next apply retries.
        let logged = scheduler::log_drink(at(9, 10), w.clone());
        assert!(effects(&logged, &logged, Some(&w)).save);
    }

    #[test]
    fn effects_hide_popup_when_logged_from_tray() {
        let p = prompting();
        let logged = scheduler::log_drink(at(9, 46), p.clone());
        assert_eq!(effects(&p, &logged, Some(&p)).popup, Some(false));
    }

    #[test]
    fn settings_view_uses_the_page_field_names() {
        let mut s = waiting();
        let (drank_label, snooze_label) = if cfg!(target_os = "macos") {
            ("⌃⌥D", "⌃⌥S")
        } else {
            ("Ctrl+Alt+D", "Ctrl+Alt+S")
        };
        assert_eq!(
            serde_json::to_value(SettingsView::from(&s)).unwrap(),
            serde_json::json!({
                "intervalMin": 45,
                "popupTimeoutMin": null,
                "drankShortcut": "control+alt+KeyD",
                "snoozeShortcut": "control+alt+KeyS",
                "drankLabel": drank_label,
                "snoozeLabel": snooze_label,
                "reminderShowing": false,
            })
        );
        s.settings.popup_timeout_min = Some(2);
        assert_eq!(
            serde_json::to_value(SettingsView::from(&s)).unwrap()["popupTimeoutMin"],
            2
        );
        let p = prompting();
        assert_eq!(
            serde_json::to_value(SettingsView::from(&p)).unwrap()["reminderShowing"],
            true
        );
    }

    #[test]
    fn settings_view_shows_defaults_for_an_unparsable_stored_shortcut() {
        let mut s = waiting();
        s.settings.drank_shortcut = "control+".into();
        let v = serde_json::to_value(SettingsView::from(&s)).unwrap();
        assert_eq!(v["drankShortcut"], "control+alt+KeyD");
    }

    #[test]
    fn change_shortcuts_saves_canonical_combos_after_test_registering_the_changed_one() {
        let mut tried = Vec::new();
        let (s, r) = change_shortcuts(waiting(), "Alt+Ctrl+Shift+W", "control+alt+KeyS", |sc| {
            tried.push(sc.into_string());
            Ok(())
        });
        assert_eq!(r, Ok(()));
        assert_eq!(s.settings.drank_shortcut, "shift+control+alt+KeyW");
        assert_eq!(s.settings.snooze_shortcut, "control+alt+KeyS");
        // Snooze is unchanged (the default), so only Drank is tried.
        assert_eq!(tried, ["shift+control+alt+KeyW"]);
    }

    #[test]
    fn change_shortcuts_refuses_during_a_reminder_without_registering() {
        let (s, r) = change_shortcuts(prompting(), "control+KeyW", "control+KeyE", |_| {
            panic!("must not register while a reminder shows")
        });
        assert_eq!(s, prompting());
        assert_eq!(r, Err("Answer the reminder first.".into()));
    }

    #[test]
    fn change_shortcuts_keeps_the_old_combos_when_refused() {
        // Invalid: nothing is registered.
        let (s, r) = change_shortcuts(waiting(), "KeyW", "control+KeyE", |_| {
            panic!("must not register an invalid combo")
        });
        assert_eq!(s, waiting());
        assert!(r.unwrap_err().contains("needs"));
        // Can't register (another app owns it, Windows).
        let (s, r) = change_shortcuts(waiting(), "control+KeyW", "control+KeyE", |sc| {
            if sc.into_string() == "control+KeyE" {
                Err("AlreadyRegistered".into())
            } else {
                Ok(())
            }
        });
        assert_eq!(s, waiting());
        let msg = r.unwrap_err();
        assert!(
            msg.starts_with("Snooze:") && msg.contains("can't be registered"),
            "{msg}"
        );
        assert!(!msg.contains("AlreadyRegistered"), "raw error shown: {msg}");
    }

    #[test]
    fn change_settings_saves_valid_values_and_refuses_others() {
        let (s, r) = change_settings(waiting(), 30, Some(5));
        assert_eq!(r, Ok(()));
        assert_eq!(
            (s.settings.interval_min, s.settings.popup_timeout_min),
            (30, Some(5))
        );

        for (i, t) in [
            (-5, None),
            (5_000_000_000, None),
            (0, None),
            (45, Some(-1)),
            (45, Some(61)),
        ] {
            let (s, r) = change_settings(waiting(), i, t);
            assert_eq!(s, waiting(), "{i} {t:?}");
            let msg = r.unwrap_err();
            assert!(msg.contains("minutes"), "{msg}");
        }
    }

    #[test]
    fn capability_lists_every_window_label() {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let windows: Vec<&str> = json["windows"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|w| w.as_str())
            .collect();
        for label in [popup::LABEL, settings::LABEL] {
            assert!(
                windows.contains(&label),
                "capability is missing window {label:?}"
            );
        }
    }

    #[test]
    fn autostart_step_defaults_on_once_and_refreshes_in_release_builds_only() {
        use AutostartStep::*;
        assert_eq!(
            autostart_step(false, false, false),
            EnableAndMark,
            "first launch"
        );
        assert_eq!(
            autostart_step(false, true, true),
            Refresh,
            "on: follow the app"
        );
        assert_eq!(
            autostart_step(false, true, false),
            Nothing,
            "turned off stays off"
        );
        for (marker, on) in [(false, false), (true, true), (true, false)] {
            assert_eq!(autostart_step(true, marker, on), Nothing, "debug build");
        }
    }

    fn marker_in(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "hydration-test-autostart-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(AUTOSTART_MARKER)
    }

    #[test]
    fn marker_is_written_only_after_a_successful_first_enable() {
        let marker = marker_in("ok");
        run_autostart_step(
            AutostartStep::EnableAndMark,
            &marker,
            || Ok::<_, String>(()),
        );
        assert!(marker.exists());

        let marker = marker_in("err");
        run_autostart_step(AutostartStep::EnableAndMark, &marker, || {
            Err::<(), _>("denied".to_string())
        });
        assert!(!marker.exists(), "a failed enable is retried next launch");
    }

    #[test]
    fn a_choice_in_settings_stops_the_default_after_a_failed_first_enable() {
        // #18: the first enable failed, so no marker; the user then turns it
        // on and off in Settings, which settles the default.
        let marker = marker_in("settled");
        run_autostart_step(AutostartStep::EnableAndMark, &marker, || {
            Err::<(), _>("denied".to_string())
        });
        assert_eq!(
            autostart_step(false, marker.exists(), false),
            AutostartStep::EnableAndMark
        );
        settle_autostart_default(&marker);
        assert_eq!(
            autostart_step(false, marker.exists(), false),
            AutostartStep::Nothing
        );
    }

    #[test]
    fn refresh_enables_without_writing_and_nothing_does_nothing() {
        let marker = marker_in("refresh");
        let mut called = false;
        run_autostart_step(AutostartStep::Refresh, &marker, || {
            called = true;
            Ok::<_, String>(())
        });
        assert!(called && !marker.exists());

        let mut called = false;
        run_autostart_step(AutostartStep::Nothing, &marker, || {
            called = true;
            Ok::<_, String>(())
        });
        assert!(!called, "enable must not run");
    }

    #[test]
    fn js_gets_no_plugin_permissions() {
        // Launch at login and logging go through our own commands, so the
        // pages need nothing beyond core.
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(json["permissions"], serde_json::json!(["core:default"]));
    }
}
