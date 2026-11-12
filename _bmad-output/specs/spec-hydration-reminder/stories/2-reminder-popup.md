---
title: 'Story 2: Reminder popup'
type: 'feature'
created: '2026-11-11'
status: 'in-review'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '7824eefbd4a5939fe491b9f5ffcbea9251a4302d'
context:
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/stack.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/state-machines.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** A due reminder only shows as "⏰" in the tray, which is easy to miss, and there's no way to answer it. Covers the popup half of CAP-2, CAP-3 and CAP-14.

**Approach:** Add a small always-on-top popup window that never takes keyboard focus. It appears on the display holding the cursor and shows the mouthfuls hint plus **Drank** and **Snooze 10 min** buttons. The answers and the optional timeout are pure scheduler functions. The popup's visibility follows the scheduler's phase: Prompting = shown, anything else = hidden.

## Boundaries & Constraints

**Always:** Rust owns the state; the popup only renders and sends `drank` / `snooze`. One shared, locked `State`: the 1 s loop and the commands both go through one apply path that re-renders the tray, shows or hides the popup on phase changes, and saves on change. Showing the popup never activates our app: macOS uses a non-focusable window; Windows shows it with `SW_SHOWNOACTIVATE` + `SWP_NOACTIVATE`. On macOS, after a button press the app you were in gets focus back: the app re-activates the last other app that was active (user decision; replaces `AppHandle::hide`). The popup appears on all macOS Spaces. Placement (user decision): next to the tray, in the work area of the cursor's display with a 12 px margin: top-right under the menu bar on macOS, bottom-right above the taskbar on Windows. Drank pushes `Drink { at: now, prev_next_due_at }` so story 3's undo works. A timeout below 1 min is treated as 1 min. The `state.json` format is unchanged.

**Never:** No OS notifications. No tray "Log a drink", undo or settings UI (story 3). No new saved fields. No popup sounds or animations. No in-memory countdown for the timeout: it's `popup_shown_at + timeout`, checked by `tick`.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Due | Waiting, `now >= next_due_at` | popup shown on the cursor's display; focus stays in the app being typed in | no monitor found → primary monitor |
| Drank | Prompting, 3 drinks | 4 drinks; `next_due = now + interval`; Waiting; popup hidden; tray updated within 1 s | N/A |
| Snooze | Prompting | count unchanged; `next_due = now + 10 min`; Snoozed; popup hidden | N/A |
| Snooze runs out | Snoozed, `now >= next_due_at` | popup shown again | N/A |
| Stale or double click | Drank/Snooze while not Prompting | ignored, state unchanged | N/A |
| Timeout off (default) | Prompting for 3 h | popup stays | N/A |
| Timeout 2 min | Prompting, `now >= popup_shown_at + 2 min` | popup hidden; count unchanged; `next_due = now + interval`; Waiting | N/A |
| Timeout 0 (hand-edited file) | `popup_timeout_min: 0` | treated as 1 min | N/A |
| Asleep through the timeout | Prompting, wake 1 h later, timeout 2 min | timed out on the first tick; no new popup until one interval later | N/A |
| Restart while prompting | stored Prompting | popup shown again within 1 s of launch | N/A |
| Midnight while prompting | Prompting across midnight | count resets; popup stays; Drank then counts 1 for the new day | N/A |

</frozen-after-approval>

## Code Map

- `src-tauri/src/scheduler.rs` -- `State`, `Phase`, `Settings.popup_timeout_min`, `popup_shown_at` and `tick` already exist; add the answers and the timeout here. Keep `tick`'s order: rollover → clamp → due check.
- `src-tauri/src/lib.rs` -- `run_scheduler` owns `State` on a thread and diffs (count, prompting) to re-render. Becomes the shared apply path.
- `src-tauri/src/tray.rs` -- `Tray::render(count, prompting)`; reuse as is.
- `src-tauri/tauri.conf.json` -- `"windows": []`, `csp: null`; `capabilities/default.json` names window `main`.
- `index.html`, `src/main.ts` -- empty placeholders for the tray-only app; replace them with the popup page.
- Tauri 2.12.5 / tao 0.37.2: `focusable(false)` → macOS `canBecomeKeyWindow = false`, Windows `WS_EX_NOACTIVATE`. tao's `show()` on Windows uses `SW_SHOW` (activates the window) after creation, hence the raw Win32 call. `accept_first_mouse(true)` lets the first click on the inactive popup press the button. Window builder also has `cursor_position`, `monitor_from_point` and `Monitor::work_area`.
- `windows` 0.62.2 is already in Cargo.lock (via Tauri); add it as a Windows-only dependency with the same version.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/scheduler.rs` -- add `Answer { Drank, Snooze }` and pure `answer(now, state, Answer) -> State`, a no-op unless Prompting. Add a timeout step to `tick` after the due check: it returns a new `Action::TimedOut`. Add `State::popup_timeout()`, floored to 1 min -- pure and testable.
- [x] `src-tauri/src/scheduler.rs` (tests) -- one test per state-level matrix row: Drank, Snooze, snooze runs out, stale click, timeout off/2 min/0, asleep through the timeout, midnight while prompting -- the timing edges are the risk.
- [x] `src-tauri/src/popup.rs` -- build the hidden popup at startup (label `popup`, `popup.html`, about 340×150, no decorations, always on top, skip taskbar, not resizable, not focusable, `accept_first_mouse`, on all workspaces). `show` picks the cursor's monitor, falling back to the primary, then shows the popup without activating it. `hide` hides it. Pure `popup_position(work_area, size, scale, corner)` (top-right on macOS, bottom-right on Windows) with unit tests.
- [x] `src-tauri/src/lib.rs` -- `Mutex<State>` in Tauri managed state; one `apply(app, f)` path for ticks and commands that renders the tray, shows or hides the popup and saves; `drank` / `snooze` commands -- single source of truth.
- [x] `src-tauri/Cargo.toml` -- `[target.'cfg(windows)'.dependencies] windows = { version = "0.62", features = [Win32_UI_WindowsAndMessaging, Win32_Foundation] }`.
- [x] `popup.html`, `src/popup.ts`, `src/popup.css`; delete `index.html` and `src/main.ts`; `vite.config.ts` input = `popup.html` -- content: "💧 Time for a drink", "A few good mouthfuls, not just a sip.", buttons Drank and Snooze 10 min; follows the system light/dark theme.
- [x] `src-tauri/tauri.conf.json`, `capabilities/default.json` -- capability window `popup`; CSP `default-src 'self'; connect-src ipc: http://ipc.localhost` -- closes the story 1 B13 item.

**Acceptance Criteria:**
- Given you're typing in another app on macOS, when the popup appears, then your keystrokes keep going to that app and the popup is visible on the current Space.
- Given the same on Windows, when the popup appears, then the foreground window keeps focus and the popup has no taskbar button.
- Given two displays, when a reminder is due, then the popup appears on the display holding the cursor.
- Given the popup on macOS, when you click Drank, then the popup hides and the app you were in has focus again.

## Implementation Notes

- **Focus on macOS (found in the Mac test, then fixed):** clicking the popup does activate our app, as the spec expected. The first version remembered the frontmost app *when the popup showed*, so a click handed focus to whichever app was in front then (System Settings in the test), not the one in use. `popup::mac` now keeps a running record of the last other app via `NSWorkspaceDidActivateApplicationNotification` and re-activates it after a button press, but only if our app is active.
- **Lock held during effects** (differs from the Design Notes' "don't hold the lock"). Holding it keeps tray, popup and save effects in the same order as the state changes. It's deadlock-free because the main thread never takes the lock after setup: `drank`/`snooze` are `async` commands, and popup show/hide are queued with `run_on_main_thread`, never awaited.
- **Windows show/hide** both go through Win32 (`SW_SHOWNOACTIVATE` + `SetWindowPos(HWND_TOPMOST, SWP_NOACTIVATE)`, then `SW_HIDE`). tao's own `show()` uses `SW_SHOW`, and mixing raw show with tao hide would leave tao's visibility flag out of step.
- Setup builds the tray and popup, then ticks through `apply` and force-renders the tray once (the tray is built showing 0).
- New macOS-only deps: `objc2-app-kit`, `objc2-foundation`, `block2` (versions already in Cargo.lock via Tauri). Windows-only: `windows` 0.62.
- `index.html` and `src/main.ts` replaced by `popup.html`, `src/popup.ts`, `src/popup.css`; Vite builds only `popup.html`.
- Manual Mac test (scripted clicks and typing in TextEdit, 15 s interval): typing reached TextEdit before, during and after the popup appeared; Drank counted and returned focus; Snooze hid the popup and returned focus; the popup sat top-right under the menu bar. Recording: 55 s screen capture kept outside the repo.
- Real `state.json` was backed up before the test and restored after.

## Spec Change Log

## Review Triage Log

Reviewers: B = Blind Hunter, E = Edge Case Hunter, V = Verification Gap.

| # | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| V1/B9 | `apply`'s popup show/hide decision is untested | medium | Only the scheduler was tested; swapping show/hide or keying off `Action` would pass | patch: pure `popup_change(prev, next)` + 3 tests (show; Drank/Snooze/TimedOut hide; no change for tick, rollover, double click) |
| V2 | Capability window label not tied to `popup::LABEL` | low | Both are literal `"popup"`; a rename would silently break the buttons | defer |
| V3/B11 | Setup re-renders the tray with duplicated logic and a hardcoded `Wry` | low | Second render path outside `apply` | patch: `Inner::render_tray` used by both |
| B1 | Snooze label hardcoded "10 min" | false | SPEC fixes snooze at 10 min; story 3 makes only the interval and timeout configurable | rejected |
| B2/E6 | Lock comment says popup calls are only queued | low | Confirmed: `send_user_message` runs inline on the main thread, so in setup `popup::show` runs under the lock. Harmless today | patch: comment states the rule (show/hide must never take the lock) |
| B3 | Drank/Snooze not logged | low | `answer_popup` passed `Action::None` | patch: log line |
| B4 | `restore_focus` after a stale click | false | A click on the popup is what activates our app, so the remembered app is the right target; with our app inactive it does nothing | rejected |
| B5 | Windows: WebView2 child may take focus on click | maybe-false (medium if real) | Can't run Windows here | defer; check at the Windows checkpoint |
| B6/E5 | Popup hidden behind full-screen apps on macOS | medium | Confirmed: tao sets only `CanJoinAllSpaces`; CAP-2 has no full-screen exception | patch: add `FullScreenAuxiliary`; verified on Mac with full-screen TextEdit |
| B7 | No screen-reader semantics | low | Markup had no role or labels | patch: `role="alertdialog"` + labelledby/describedby |
| B8a | No clamp for a work area smaller than the popup | low | Needs a display under ~370×175 points; fix adds a branch | rejected |
| B8b | Windows `WM_DPICHANGED` may shift the popup on mixed-DPI moves | maybe-false (low) | Would only be low; Windows positions are global pixels | rejected |
| B10a | Prompting with no `popup_shown_at` never times out | false | Every path into Prompting sets it; `on_load` clears both | rejected |
| B10b | Timeout racing a button press; Drank after TimedOut | false | The lock serialises them; an answer outside Prompting is ignored (`stale_or_double_click_is_ignored`) | rejected |
| B12 | popup.ts swallows errors; buttons stay enabled | low | Backend ignores repeats; errors need a broken IPC; fix adds UI state | rejected |
| B13 | `windows` crate compiled twice | false | Cargo.lock has a single `windows` 0.62.2, shared with Tauri | rejected |
| E1 | Failed show leaves Prompting with no popup | low | Needs the popup window missing or a dead event loop; tray still shows ⏰; fix adds a retry path | rejected |
| E2 | `cursor_position` error skips the primary-display fallback | low | `?` returned early, against the matrix row | patch: errors fall through to the primary display |
| E3 | Drank just after midnight counts for yesterday | low | Up to 1 s window before the rollover tick | patch: `answer_popup` ticks first |
| E4 | macOS mixed-scale displays misplace the popup | medium | Confirmed: tao converts a physical position with the window's current scale | patch: macOS passes a logical position in the target display's scale |
| E7 | CSP blocks Vite HMR in dev | maybe-false (low) | Dev-only; the popup page has no HMR need | rejected |
| E8 | Code returns focus by re-activating the last app, not `AppHandle::hide` as the frozen text says | low | Intent (focus goes back) is met and tested; `hide` would need an unhide that activates the app. Fix would edit the frozen spec | user reworded the frozen text to match the code |

## Design Notes

Effects follow the phase change, not the `Action`, so ticks and commands share one code path:

```rust
fn apply(app, f: impl FnOnce(State) -> State) {
    let mut s = lock(); let prev = s.clone(); *s = f(prev.clone());
    if tray_view(&prev) != tray_view(&s) { tray.render(..) }
    match (prev.phase == Prompting, s.phase == Prompting) {
        (false, true) => popup::show(app), (true, false) => popup::hide(app), _ => {}
    }
    if *s != saved { persist::save(..) }
}
```

Window calls run on the main thread (`run_on_main_thread`). Don't hold the lock across them.

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- expected: all pass, including the new scheduler and placement tests
- `cd src-tauri && cargo clippy -- -D warnings` -- expected: clean
- `cd src-tauri && cargo check --target x86_64-pc-windows-msvc` -- expected: compiles (llvm-rc stub on PATH)
- `npm run build` -- expected: tsc and vite pass

**Manual checks:**
- macOS: `HYDRATION_INTERVAL_SECS=5 npm run tauri dev`; take a screenshot of the popup; check that typing in another app isn't interrupted; check Drank and Snooze.
- Windows (user, done checkpoint): the same, plus the 16 px badge check (#11).
