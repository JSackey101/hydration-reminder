mod badge;
mod persist;
mod popup;
mod scheduler;
mod tray;

use chrono::Local;
use scheduler::{Action, Answer, Phase, State};
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
/// thread never takes it: commands are async, so they run off the main thread.
fn apply<R: Runtime>(app: &AppHandle<R>, f: impl FnOnce(State) -> (State, Action)) {
    let core = app.state::<Core<R>>();
    let mut g = core.0.lock().unwrap_or_else(|e| e.into_inner());
    let prev = g.state.clone();
    let (next, action) = f(prev.clone());
    log_action(action, &next);

    let tray_changed = tray_view(&next) != tray_view(&prev);
    let popup = popup_change(&prev, &next);
    g.state = next;
    if tray_changed {
        g.render_tray();
    }
    if let Some(is) = popup {
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

    if g.saved.as_ref() != Some(&g.state) {
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
        .invoke_handler(tauri::generate_handler![drank, snooze])
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
}
