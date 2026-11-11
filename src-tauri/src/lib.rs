mod badge;
mod persist;
mod scheduler;
mod tray;

use chrono::Local;
use scheduler::{Action, Phase, State};
use std::time::Duration;
use tauri::Manager;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // A second launch (e.g. autostart plus a manual start) exits at once,
        // so two schedulers never share state.json.
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
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

            // Tick once before the first render, so a new day never shows
            // yesterday's count, even briefly.
            let (state, action) = scheduler::tick(Local::now(), state);
            log_action(action, &state);

            let tray = tray::Tray::build(app.handle())?;
            tray.render(state.count(), state.phase == Phase::Prompting);

            std::thread::spawn(move || run_scheduler(path, state, tray));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn log_action(action: Action, state: &State) {
    match action {
        Action::ShowPopup => eprintln!("[hydration] reminder due"),
        Action::ResetDay => eprintln!("[hydration] new day {}: count reset", state.day),
        Action::None => {}
    }
}

/// Ticks once a second. Due times are absolute wall-clock instants, so this
/// also catches up straight after a wake from sleep.
fn run_scheduler<R: tauri::Runtime>(
    path: std::path::PathBuf,
    mut state: State,
    tray: tray::Tray<R>,
) {
    let mut saved: Option<State> = None;
    loop {
        let (next, action) = scheduler::tick(Local::now(), state.clone());
        log_action(action, &next);
        let shown = |s: &State| (s.count(), s.phase == Phase::Prompting);
        if shown(&next) != shown(&state) {
            tray.render(next.count(), next.phase == Phase::Prompting);
        }
        state = next;
        if saved.as_ref() != Some(&state) {
            match persist::save(&path, &state) {
                Ok(()) => saved = Some(state.clone()),
                Err(e) => eprintln!("[hydration] warning: save failed: {e}"),
            }
        }
        std::thread::sleep(TICK);
    }
}
