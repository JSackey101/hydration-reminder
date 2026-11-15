mod badge;
mod persist;
mod popup;
mod scheduler;
mod settings;
mod tray;

use chrono::Local;
use scheduler::{Action, Answer, Phase, Settings, State};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};

const TICK: Duration = Duration::from_secs(1);

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
/// not the action: entering Prompting shows the popup, leaving it hides it.
///
/// The lock is held while effects are issued so they keep their order. Popup
/// calls go through `run_on_main_thread`: queued from other threads, but run
/// inline when `apply` is already on the main thread (setup). So `popup::show`
/// and `popup::hide` must never take the `Core` lock. Outside setup the main
/// thread never takes it: commands are async, so they run off the main thread,
/// and the tray menu hands Log and Undo to a worker thread. This matters
/// because tray updates wait for the main thread while the lock is held.
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
        let queued = app.run_on_main_thread(move || {
            if is {
                popup::show(&handle)
            } else {
                popup::hide(&handle)
            }
        });
        if let Err(e) = queued {
            eprintln!("[hydration] warning: popup update failed: {e}");
        }
    }

    if fx.save {
        match persist::save(&g.path, &g.state) {
            Ok(()) => g.saved = Some(g.state.clone()),
            Err(e) => eprintln!("[hydration] warning: save failed: {e}"),
        }
    }
}

fn answer_popup<R: Runtime>(app: &AppHandle<R>, answer: Answer) {
    eprintln!("[hydration] answer: {answer:?}");
    // Tick first, so a press just after midnight lands on the new day.
    apply(app, |s| {
        let now = Local::now();
        let (s, action) = scheduler::tick(now, s);
        (scheduler::answer(now, s, answer), action)
    });
    if let Err(e) = app.run_on_main_thread(popup::restore_focus) {
        eprintln!("[hydration] warning: focus restore failed: {e}");
    }
}

/// Tray "Log a drink" and "Undo last drink". The tray menu handler runs these
/// off the main thread (see `apply`). Tick first, as for the popup buttons.
pub(crate) fn tray_log_drink<R: Runtime>(app: &AppHandle<R>) {
    eprintln!("[hydration] tray: log a drink");
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
    eprintln!("[hydration] tray: undo last drink");
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
}

impl From<&Settings> for SettingsView {
    fn from(s: &Settings) -> Self {
        Self {
            interval_min: s.interval_min,
            popup_timeout_min: s.popup_timeout_min,
        }
    }
}

#[tauri::command]
async fn get_settings(app: AppHandle) -> SettingsView {
    let core = app.state::<Core<tauri::Wry>>();
    let g = core.0.lock().unwrap_or_else(|e| e.into_inner());
    SettingsView::from(&g.state.settings)
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
        eprintln!(
            "[hydration] settings: interval {interval_min} min, timeout {popup_timeout_min:?}"
        );
    }
    result
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
        // A second launch (e.g. autostart plus a manual start) exits at once,
        // so two schedulers never share state.json.
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .invoke_handler(tauri::generate_handler![
            drank,
            snooze,
            get_settings,
            save_settings
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let path = dir.join("state.json");

            let mut state = scheduler::on_load(persist::load(&path, Local::now()));
            state.interval_override_secs = interval_override();
            if let Some(secs) = state.interval_override_secs {
                eprintln!("[hydration] debug: interval overridden to {secs} s");
            }

            let tray = tray::Tray::build(app.handle())?;
            popup::build(app.handle())?;
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
        Action::ShowPopup => eprintln!("[hydration] reminder due"),
        Action::ResetDay => eprintln!("[hydration] new day {}: count reset", state.day),
        Action::TimedOut => eprintln!("[hydration] popup timed out"),
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
        let mut s = Settings::default();
        assert_eq!(
            serde_json::to_value(SettingsView::from(&s)).unwrap(),
            serde_json::json!({ "intervalMin": 45, "popupTimeoutMin": null })
        );
        s.popup_timeout_min = Some(2);
        assert_eq!(
            serde_json::to_value(SettingsView::from(&s)).unwrap()["popupTimeoutMin"],
            2
        );
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
}
