---
title: 'Story 3: Tray menu actions and interval setting'
type: 'feature'
created: '2026-11-15'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '2e7185b60104d24ca6992905b56f9bcdc30a990d'
context:
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/state-machines.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** You can only log a drink by answering a popup, you can't take back a wrong press, and the interval and popup timeout can only be changed by editing `state.json`. Covers CAP-4, CAP-5 and CAP-13. The Windows tray count is also too small to read at 16 px (#11).

**Approach:** Add "Log a drink", "Undo last drink" and "Settings…" to the tray menu. All three change state as pure scheduler functions run through the existing `apply` path. "Settings…" opens a small, normal settings window for the interval and the popup timeout. Redraw the Windows tray icon as a big white number filling the drop, with the drop turning red when a reminder is due (user decision on #11).

## Boundaries & Constraints

**Always:** Rust owns the state; the settings page only reads and sends values. Every state change goes through `apply`. Tray menu handlers run on the main thread and must hand their work to another thread before taking the lock, because tray setters block on the main thread (deadlock otherwise). Settings live in the existing `State.settings`; the `state.json` format is unchanged. Interval: whole minutes, 1–240. Timeout: Off or whole minutes, 1–60. Settings apply on change (user decision): each field saves as soon as it holds a valid value; there are no Save or Cancel buttons. Invalid input shows an inline message and is not saved. "Undo last drink" is disabled when today has no drinks.

**Never:** No change to the 10-minute snooze. No launch-at-login (story 4). No OS notifications. The settings window never opens on its own.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Log while waiting | Waiting, 3 drinks | 4 drinks; `next_due = now + interval`; Waiting | N/A |
| Log while popup open | Prompting | counts as Drank; popup hides | N/A |
| Log while snoozed | Snoozed | 1 more drink; `next_due = now + interval`; Waiting | N/A |
| Undo after a log | Waiting, last drink set the due time | 1 fewer drink; `next_due` = that drink's `prev_next_due_at`; if that time has passed, the next tick prompts | N/A |
| Undo while popup open or snoozed | Prompting / Snoozed | 1 fewer drink; phase and due time unchanged (a reminder has fired since that drink) | N/A |
| Undo with no drinks | 0 drinks today | menu item disabled; command is a no-op | N/A |
| Shorter interval | Waiting, due in 40 min, 45 → 15 | due time clamped to now + 15 min | N/A |
| Longer interval | Waiting, due in 10 min, 45 → 90 | due time unchanged; the new interval applies from the next drink | N/A |
| Timeout set while popup open | Prompting for 5 min, timeout → 2 | popup times out on the next tick | N/A |
| Settings opened twice | window already open | existing window comes to the front | N/A |

</frozen-after-approval>

## Code Map

- `src-tauri/src/scheduler.rs` -- add pure `log_drink(now, s)`, `undo(s)` and `set_settings(s, Settings) -> Result<State, String>` (range checks). `undo` restores the due time only when the phase is Waiting (a timed-out popup also lands in Waiting, so undo then re-prompts on the next tick; acceptable). Reuse `answer(.., Drank)` for Log while Prompting. `tick`'s clamp (step 2) already handles a shorter interval. Keep `on_load` and `tick` order unchanged.
- `src-tauri/src/lib.rs` -- `apply`, `Core`/`Inner`, `tray_view`, `popup_change`. Pull the render/popup/save decisions into a pure `effects(prev, next, saved)` and unit-test it (#9). New `async` commands `get_settings` and `save_settings` go through `apply`. Register them in `generate_handler!`.
- `src-tauri/src/tray.rs` -- the menu is currently header / separator / Quit. Add Log a drink, Undo last drink, Settings…. `render` also sets Undo's enabled state from the count. Menu handlers: `quit` stays inline; the others `tauri::async_runtime::spawn_blocking` before calling into `lib.rs`.
- `src-tauri/src/tray.rs` / `tauri` 2.12.5 `menu/mod.rs:26` -- `run_item_main_thread!` waits on a channel for the main thread. That's why handlers must not take the lock on the main thread.
- New `src-tauri/src/settings.rs` -- `LABEL = "settings"`; `open(app)`: focus the window if it exists, otherwise build `settings.html` (about 360×240, centred, decorations, not resizable, focused). On macOS (Accessory app), activate the app so the window comes to the front.
- `src-tauri/src/badge.rs` -- currently a small badge in the bottom-right corner. Becomes the drop itself coloured blue (or red when due) with centred white digits plus a 1 px dark outline; largest integer glyph scale that fits a 22 px wide box (one or two digits → scale 3, "99+" → 2). Update its tests.
- `src-tauri/capabilities/default.json` -- `windows: ["popup", "settings"]`. Add a test that reads this file and checks it lists `popup::LABEL` and `settings::LABEL` (#12).
- `vite.config.ts` -- add `settings: "settings.html"` to `build.rollupOptions.input`.
- New `settings.html`, `src/settings.ts`, `src/settings.css` -- reuse the light/dark CSS variables from `src/popup.css`.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/scheduler.rs` -- `log_drink`, `undo`, `set_settings` + one test per matrix row.
- [x] `src-tauri/src/lib.rs` -- pure `effects` + tests (#9); `log_drink`/`undo_drink` entry points used by the tray; `get_settings`/`save_settings` commands.
- [x] `src-tauri/src/tray.rs` -- new menu items, Undo enabled state, off-main-thread handlers.
- [x] `src-tauri/src/settings.rs` -- open-or-focus the settings window.
- [x] `settings.html`, `src/settings.ts`, `src/settings.css`, `vite.config.ts` -- settings page: interval (minutes) and popup timeout (Off or minutes), inline errors.
- [x] `src-tauri/capabilities/default.json` + capability test (#12).
- [x] `src-tauri/src/badge.rs` -- big-number drop icon, red when due (#11) + tests.

**Acceptance Criteria:**
- Given the app running, when the interval is changed in Settings and the app is quit and relaunched, then Settings shows the new interval and it's used for the next due time.
- Given the popup open, when "Log a drink" is chosen from the tray, then the count goes up by 1 within 1 s and the popup closes.
- Given a reminder due on Windows, when the tray icon is viewed at 100% scaling, then the drop is red and the count can be read.
- Given any menu action while the scheduler is ticking, then the app never hangs.

## Implementation Notes

- Scheduler: `log_drink`, `undo`, `set_settings` (+ `INTERVAL_RANGE_MIN`, `TIMEOUT_RANGE_MIN`); 12 new tests, one or more per matrix row. "Settings opened twice" is window behaviour (`settings::open`), checked in the Mac UI test.
- `lib.rs`: `apply` now uses pure `effects(prev, next, saved)` (#9, 3 tests). `tray_log_drink` / `tray_undo` tick first, like the popup buttons. `save_settings` runs `set_settings` inside `apply` and returns its error to the page.
- Tray: Log a drink, Undo last drink (enabled when count > 0, set in `render`), Settings…. Log and Undo go through `tauri::async_runtime::spawn_blocking`; Settings opens on the main thread (it takes no lock).
- `settings.rs`: window built on demand and destroyed on close (420×220, centred). macOS activates the app so the window comes to the front of an Accessory app.
- Badge (#11): drop 1 px bigger (r 11), blue or red; centred white digits, scale 3 for 1–2 digits, 2 for "99+", 1 px dark outline. Preview checked at 32 px and a 16 px downscale on light and dark backgrounds: single digits and "12" read clearly.
- `src/theme.css` holds the light/dark tokens, now imported by `popup.css` and `settings.css`.
- Capability test (#12) reads `capabilities/default.json` via `include_str!`.
- **Mac UI test** (TextEdit with hands-off/done lines, `state.json` backed up and restored, 15 s interval): menu shows header, Log a drink, Undo (disabled at 0), Settings…, Quit. Log with the popup open counted 1 and closed the popup. Undo with the popup open took the count to 0 and kept the popup. Settings opened twice gave one window. Interval 20, timeout on (filled in 2), then 3 all saved on change. 500 was refused with "Interval must be 1–240 minutes." and nothing saved. Closing Settings (⌘W) left the app running (rules out B10). The timeout closed the open popup exactly 3 min after it showed. After quit and relaunch, Settings showed the saved values.
- **Found in the Mac test, then fixed:** a newly built settings window opened behind the app in use, because `focused(true)` plus app activation isn't enough on macOS. Reopening an existing window (`set_focus`) did come forward, so the new-window path now calls `set_focus` too. Re-tested on Mac: the first open now comes to the front with TextEdit active before.
- Test tooling note: System Events can't open a Tauri tray menu (AXPress does nothing), and an Accessibility query just before a real click stops the menu opening. Click the tray item with CGEvent at a fixed position instead.
- Review patches: see the Review Triage Log. After them: `cargo test` 70 passed;
- Verified before review: `cargo test` 65 passed; clippy clean for macOS and `x86_64-pc-windows-msvc`; `npm run build` passes.

## Spec Change Log

## Review Triage Log

Reviewers: B = Blind Hunter, E = Edge Case Hunter, V = Verification Gap.

| # | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| B1/E1 | Undo in Waiting after a timed-out popup restores a passed due time and re-prompts at once | low | Confirmed: undo checked only the phase; a timeout also lands in Waiting | patch: restore only if the drink set the current due time (`next_due == drink.at + interval`); tests for timeout (kept) and popup Drank (popup comes back, as intended) |
| E2 | Log then Undo run out of order on separate workers | low | Needs two menu clicks milliseconds apart; the menu has to be reopened between them | rejected |
| E3/B6 | Overlapping saves can apply out of order; "Saved." never clears | low | Change events fire on blur/Enter; overlap needs sub-ms edits. Stale "Saved." is cosmetic | rejected |
| E4 | Negative or huge number shows a serde error, not the range message | low | Confirmed: u32 args fail to deserialize. Easy to type | patch: command takes i64; pure `change_settings` maps out-of-range to the range message; tested |
| B5/E5/E6 | Inputs editable before load, or after a failed load | low | Confirmed; fix is two attributes | patch: inputs start `disabled`, enabled after a successful load |
| B7/E7 | Tray log records "reminder due" when the same step answers it | low | Confirmed: log line only | patch: ShowPopup → None in `tray_log_drink` |
| B9/E8 | macOS activates the app after the window failed to open | low | Confirmed | patch: activate only on Ok |
| V1 | Undo enabled rule untested | low | Confirmed: `count > 0` inline in `render` | patch: pure `tray::undo_enabled` + test |
| V2/B13 | Settings command contract (camelCase keys, error path) untested | medium | Confirmed: dropping `rename_all` would load "undefined" with all tests green | patch: `settings_view_uses_the_page_field_names`, `change_settings_saves_valid_values_and_refuses_others` |
| V3 | `settings.html` could drop out of the build with CI green | medium | Confirmed: dev serves it regardless; CI never opens settings | patch: Windows workflow checks `dist/popup.html` and `dist/settings.html` |
| V4 | No front-end tests for `settings.ts` | low | Confirmed: no JS runner; Rust validates every value | defer: issue #15 |
| B2 | "Saved." shown when the disk write failed | low | Real but pre-existing pattern (`apply` logs save failures); needs a full disk | rejected (logging: #10) |
| B3 | Field keeps a rejected value | low | The inline error is shown next to it; normal form behaviour | rejected |
| B4 | Ranges in both HTML and Rust | low | Rust is the authority; HTML min/max only affect the stepper | rejected |
| B8 | Tray menu stale at startup | false | Setup force-renders the tray (`render_tray`) after the first `apply` | rejected |
| B10 | Closing settings quits the app | false | The hidden popup window always exists, so the last window never closes; checked in the Mac test | rejected |
| B11 | Both windows share one capability | low | Both pages are local, bundled content | rejected |
| B12 | Outline test overclaims | false | With no text pixel on the edge, every text neighbour is in bounds and gets outlined | rejected |
| E9 | AC "used for the next due time" vs longer interval | false | Frozen matrix: a longer interval applies from the next drink | rejected |
| E10 | Code Map names `undo_drink`, code has `tray_undo` | low | Naming in the spec only | rejected (spec edit) |

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- expected: all pass
- `cd src-tauri && cargo clippy -- -D warnings` and `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings` -- expected: clean (llvm-rc stub on PATH)
- `npm run build` -- expected: tsc and vite pass

**Manual checks:**
- macOS UI test (hands-off signals, `state.json` backed up and restored): Log a drink, Undo, Log with the popup open, change interval and timeout, quit and relaunch.
- Windows CI: popup test still passes; the tray crop shows the new icon.
