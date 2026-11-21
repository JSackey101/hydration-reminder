---
title: 'Story 6: Small drop reminder style'
type: 'feature'
created: '2026-11-21'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '3f38d2634b37165290799b3e6a3233e849d7dfed'
context:
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/state-machines.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The popup is a lot to have pop up for someone deep in work. Some users want a quieter cue that's still on screen (CAP-16).

**Approach:**
- Add a "Reminder style: Popup / Small drop" setting, with Popup as the default.
- With Small drop, entering Prompting shows a small, unfocusable drop window in the popup's corner instead of the popup.
- Clicking the drop swaps it for the normal popup while staying in Prompting.
- The story 5 shortcuts and the timeout work exactly as they do now.

**Decisions (user, 2026-11-21):**
- **Look:** the drop is a round bubble, about 56 px, showing 💧. It pulses gently once when it appears, then stays still. Honour `prefers-reduced-motion`: no pulse.
- **Spec size:** keep the full spec (about 2,200 tokens after the review). It's one goal, and the Windows CI drop run stays in.
- **`expanded` lives in memory only** (`#[serde(skip)]`), so a drop click never writes `state.json`.
- **The debug override `HYDRATION_REMINDER_STYLE=drop`** is held in memory only, like `HYDRATION_INTERVAL_SECS`. It's never saved.

## Boundaries & Constraints

**Always:**
- **What shows** is a pure function of the state, `view(&State)`:
  - not Prompting → nothing (`expanded` is ignored);
  - Prompting, effective style Drop, not expanded → the drop;
  - otherwise → the popup.
  - The effective style is the debug override if set, otherwise the setting.
- **`expanded`** is reset in only two places: the `tick` branch that enters Prompting, and `on_load`. `expand(s)` sets it only while Prompting, so a stale click is a no-op. Nothing else in `tick` or `answer` changes, so the CAP-14 timeout still counts from `popup_shown_at`, which is when the drop appeared.
- **`effects` keeps two signals apart:**
  - `prompting: Option<bool>`, from the phase change, alone registers and releases the shortcuts and emits `REMINDER_SHOWING_EVENT`, exactly as today.
  - `view: Option<View>`, from the view change, only swaps windows.
  - Expanding the drop or changing the style mid-reminder changes `view` only.
  - A swap shows the new window, then hides the old one. On entering Prompting the order stays labels → show → register.
- **The drop window copies the popup's no-focus build:**
  - `focusable(false)`, always on top, `skip_taskbar`, no decorations, `accept_first_mouse`, all workspaces;
  - on macOS, `FullScreenAuxiliary`;
  - on Windows, the same `SW_SHOWNOACTIVATE` / `HWND_TOPMOST` show and hide path.
  - Its title is "Hydration Reminder Drop".
  - `restore_focus` runs after a drop click.
- **Transparency:**
  - The `macos-private-api` Cargo feature and `app.macOSPrivateApi: true` must agree, or tauri-build fails.
  - The window is `transparent(true)` and `shadow(false)`.
  - `drop.css` forces `html, body { background: transparent }`, overriding theme.css's `--bg`.
- **Placement:** `popup_position` with the drop's size, in the same corner and margin, on the cursor's display.
- **Threading rules from story 5 still apply:** the main thread never takes the `Core` lock, and global-shortcut calls are only single `on_shortcut` / `unregister` calls.
- Old `state.json` files load with style Popup.

**Never:**
- No other change to `tick` or `answer` timing or transitions.
- No OS notification.
- No new JS plugin permissions.
- No drag, dismiss or count on the drop.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|---|---|---|---|
| Drop due | style Drop, due | drop shows; shortcuts registered; focus unchanged | N/A |
| Click drop | Prompting, drop | popup shows, then the drop hides; still Prompting; shortcuts untouched; focus restored | stale click → no-op |
| Shortcut on drop | Prompting, drop | Drank/Snooze as usual; drop hides; shortcuts released | N/A |
| Timeout | 2 min, drop at 9:45, expanded at 9:46 | times out at 9:47 | N/A |
| Tray Log | drop showing | answered; drop hides | N/A |
| Style switch mid-reminder | Prompting, not expanded | the view follows the new style; shortcuts untouched | N/A |
| Next reminder after an expand | expanded, answered, due again | drop again (expanded reset) | N/A |
| Restart while Prompting | style Drop | Waiting, not expanded | N/A |
| Old file | no `reminder_style` key | Popup | N/A |

</frozen-after-approval>

## Code Map

- `src-tauri/src/scheduler.rs`:
  - `Settings` gets `reminder_style: ReminderStyle` (`Popup` | `Drop`, serde lowercase, default `Popup`).
  - `State` gets `expanded` and `style_override: Option<ReminderStyle>`, both `#[serde(skip)]` (next to `interval_override_secs`, ~line 69).
  - Reset `expanded` in the enter-Prompting branch of `tick` (~274) and in `on_load` (~162).
  - New `expand(s)`.
  - New `set_reminder_style(s, style)`, which leaves `expanded` alone.
  - New `State::reminder_style()`, which returns the override, otherwise the setting.
- `src-tauri/src/lib.rs`:
  - Replace `popup_change` and `Effects.popup` (~56–81) with `prompting` + `view`.
  - Split the main-thread task (~114–139) to match.
  - `SettingsView` gets `reminderStyle`.
  - New commands `open_reminder` (tick, `expand`, then `restore_focus` on the main thread) and `save_reminder_style`.
  - Read `HYDRATION_REMINDER_STYLE` beside `interval_override`, debug builds only.
- `src-tauri/src/popup.rs`:
  - Generalise `build`, `show`, `hide` and `place` over (label, url, title, size).
  - Add `DROP_LABEL` and `DROP_SIZE` (56×56).
  - Call `watch_frontmost` once.
  - Reuse the `win32` and `mac` modules unchanged.
- `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`: turn on the private API, as above.
- `src-tauri/capabilities/default.json`: add `"drop"` to `windows`. `core:default` already covers app commands.
- New `drop.html`, `src/drop.ts` and `src/drop.css`. The page is a `<button aria-label="Time for a drink. Open the reminder">` that invokes `open_reminder`. Add it to the `vite.config.ts` inputs.
- `settings.html`, `src/settings.ts`: a "Reminder style" radio pair that saves on change. `settings.rs`: make the window about 460 tall.
- `.github/workflows/windows-popup-test.yml` (~31): add `drop.html` to the page check.
- `scripts/windows-popup-test.ps1`:
  - `FindTop` matches the exact title.
  - The final Snooze leaves `state.json` Snoozed for 10 minutes, and `snooze()` ignores the override.
  - Both runs append to one log file, and the "exactly once" check is at ~216.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/src/scheduler.rs`: add the style, `expanded`, the override and the functions above. Unit-test the timeout counting from when the drop appeared, a stale expand, an expand reset on the next due, `on_load`, an old file, and that `expanded` isn't serialised.
- [x] `src-tauri/src/lib.rs`: add the `effects` split, the window task, the commands, `SettingsView` and the env override. Unit-test `effects` for entering Prompting (drop), expand (`prompting: None`), a style switch (`prompting: None`), and leaving Prompting from the drop.
- [x] `src-tauri/src/popup.rs`, `Cargo.toml`, `tauri.conf.json`, `capabilities/default.json`: build the drop window.
- [x] `drop.html`, `src/drop.ts`, `src/drop.css`, `vite.config.ts`: the drop page, with the single pulse and the reduced-motion rule.
- [x] `settings.html`, `src/settings.ts`, `src-tauri/src/settings.rs`: the style setting.
- [x] `scripts/windows-popup-test.ps1` and the workflow. After run 1:
  - kill the app, note the log length, delete `%APPDATA%\com.jeffsackey.hydration-reminder\state.json`, and relaunch with `HYDRATION_REMINDER_STYLE=drop`;
  - find "Hydration Reminder Drop" and check that focus stays in Notepad, the no-activate and topmost bits are set, it sits bottom-right, and its content rect is 56 DIP square;
  - click its centre, then check the popup shows, the drop hides and focus is back; click Drank;
  - on the next drop, Ctrl+Alt+S hides it;
  - check the second run's log lines only.
- [x] `README.md`, `SPEC.md`: document the reminder style and mark CAP-16 as done.

**Acceptance Criteria:**
- Given style Popup, when a reminder is due, then behaviour is unchanged from story 5.
- Given style Drop on macOS or Windows, when the drop shows, then keyboard focus stays in the app in use, and the drop is visible over a full-screen app on macOS.
- Given the drop is showing, when it's clicked, then the popup shows, it's still Prompting, and focus goes back to the previous app.
- Given the drop is showing, when ⌃⌥D / Ctrl+Alt+D is pressed, then a drink is counted and the drop hides.
- Given a timeout is set, when the drop is expanded part-way through, then the reminder still times out counted from when the drop appeared.
- Given Settings, when Small drop is picked, then it's saved and survives a restart, and a debug env override never changes the saved value.

## Implementation Notes

- **Saving.** `#[serde(skip)]` fields still count in the derived `PartialEq`, and `effects` saved on `saved != next`. Added `State::same_on_disk`, which compares persisted fields only. It destructures every field, so a new field won't compile until it's listed. A drop click and the debug override never write `state.json`.
- **Pulse.** It's driven by a `drop-shown` event that Rust emits from `show_view` each time the drop shows. The drop window is built once and hidden between reminders, so a CSS on-insert animation would pulse only once per launch, and `visibilitychange` isn't reliable across WKWebView and WebView2. The bubble is 48 px inside the 56 px window, which leaves room for the 1.12× pulse. `prefers-reduced-motion` turns the pulse off.
- **The view diff.** `popup_change` was renamed `prompting_change` and is unchanged in meaning. `show_view` shows the new window before hiding the other. `effects` stays pure.
- **Debug override.** `HYDRATION_REMINDER_STYLE` accepts `drop` or `popup`; any other value is ignored with a warning.
- **SPEC.md isn't changed.** It has no "done" markers (CAP-15 wasn't marked either). Done status lives in the story spec's `status` and on the board. CAP-16 is documented in the README.
- **Not run locally:** the PowerShell script (no pwsh on this Mac). CI is its first run.
- **Files:**
  - Rust: `scheduler.rs`, `lib.rs`, `popup.rs`, `settings.rs`, `shortcuts.rs` (label constant);
  - config: `Cargo.toml`, `tauri.conf.json`, `capabilities/default.json`, `vite.config.ts`;
  - frontend: `drop.html`, `src/drop.ts`, `src/drop.css`, `settings.html`, `src/settings.ts`, `src/settings.css`;
  - CI and docs: `scripts/windows-popup-test.ps1`, `.github/workflows/windows-popup-test.yml`, `README.md`.
- **Checks:**
  - `cargo test`: 99 passed (87 before).
  - clippy `-D warnings` on macOS and on x86_64-pc-windows-msvc: clean.
  - `cargo fmt --check`: clean.
  - `npm run build`: three pages built.
- **Mac UI test (2026-11-21, `HYDRATION_INTERVAL_SECS=15`), all passing:**
  - Picking Small drop in Settings while the popup was showing swapped it for the drop at once; `state.json` had `"reminder_style": "drop"`.
  - The drop sits at (1444, 45), 56×56, in the popup's corner, as a round bubble with transparent corners.
  - Clicking the drop from TextEdit showed the popup ("drop opened"), and TextEdit kept focus.
  - Drank on the opened popup worked. The next reminder was the drop again, and ⌃⌥D answered it directly.
  - After a restart the style was still Small drop.
  - The drop shows over a full-screen TextEdit, keeps focus there, and its pulse shows in the frame captures.
  - Two observations:
    - A freshly launched debug app took focus. That's tao's activate-on-launch, the same for the popup and not caused by the drop. A drop that showed later never took focus.
    - Once, the first drop after TextEdit entered full screen appeared about 1.5 s late. It didn't happen again in 2 more full-screen cycles, so it's likely the Space transition.

## Spec Change Log

## Review Triage Log

Pass 1 (2026-11-21). Reviewers: Blind Hunter (B), Edge Case Hunter (E) and Verification Gap (V). Routes: patch, defer or reject. There were no intent_gap or bad_spec findings, so no loopback.

| # | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| E1 | An unknown `reminder_style` value in `state.json` fails the whole decode | medium | `persist::load` treats a decode error as corrupt and starts from defaults, so drinks and history are lost. Fixed: `#[serde(other)]` on `Popup` (now the last variant). Test: `unknown_style_loads_as_popup_and_keeps_the_rest` | patch |
| B13 | A doc comment in `apply` runs past the line width | low | The earlier rewrap only half applied (lib.rs ~163). Rewrapped | patch |
| B2 | "drop opened" is logged before `apply`, so a stale click logs it too and the CI check can pass falsely | low | Logged first in `open_reminder`. Now logged after, only if the popup actually opened; otherwise "drop click ignored" | patch |
| B3, E9 | The CI warning check misses the new warnings | low | The regex didn't match `not sent`, `update failed` or `restore failed`. Widened, and it also fails on "click ignored" | patch |
| V-other, E8 | The CI popup checks in run 2 pass vacuously if the popup handle is 0 | low | `IsWindowVisible(0)` returns false. Added "Popup window found in the drop run" | patch |
| B5, E4 | The first drop after startup can miss its pulse | low | `apply` in setup runs before `drop.ts` listens; common on a login start with an overdue reminder. `drop.ts` now also pulses on load | patch |
| B6 | Clicks just outside the bubble do nothing (Windows) | low | Only the button had a handler, but the whole transparent window takes clicks on WebView2. The click handler is now on `document` | patch |
| B7, E7 | The shadow is clipped at the pulse peak | low | 24 px × 1.12 + 5 px of shadow is more than the 28 px half-window. Now a 44 px bubble, a 1.08 pulse and a 3 px blur (23.8 + 4 < 28) | patch |
| B1, E6 | After a failed style save, the radio shows a style that wasn't saved | low | On rejection `saveStyle` showed the error but left the radio. It now re-checks the saved style. Returning a `Result` isn't needed: nothing in `set_reminder_style` can fail | patch |
| B12 | Missing unit tests: a style change while Waiting, and a timeout while the drop shows | low | Added `style_change_while_waiting_touches_no_window` and `timeout_while_the_drop_shows_hides_it_and_releases` | patch |
| V1 | The Settings radio isn't tested end to end | medium | No JS runner; CI uses the env override. Same gap as #15 | defer → #15 |
| V2 | macOS drop focus, full screen and transparency are checked only by hand | low | Same accepted limit as the popup: GitHub runners can't drive macOS focus or Spaces. Covered by this story's Mac UI test | reject |
| B4 | CI doesn't press Ctrl+Alt+D on the drop | low | Shortcut handling doesn't depend on the view (`answer_by_shortcut`). Run 1 covers Ctrl+Alt+D, run 2 Ctrl+Alt+S on the drop, and unit tests cover answering from the drop | reject |
| B8 | The drop doesn't show the shortcut keys | low | The spec chose a minimal drop (no text). The README documents the keys. Adding them would mean sending labels to a second window | reject |
| B9 | The `macos-private-api` trade-off isn't documented | low | App-store distribution is a SPEC non-goal, and the trade-off is recorded in this spec's Boundaries | reject |
| B10, E11 | The task line says SPEC.md was updated, but it isn't in the diff | low | Real, but the fix edits this build's spec; Implementation Notes explain it | reject |
| B11 | Settings doesn't show when the debug override is active | low | Debug builds only; the override is logged at startup | reject |
| E2 | A stale drop click landing after the next reminder starts opens the popup | false | Needs Drank plus a whole interval (at least 1 min) between the click and its handling, which takes milliseconds | reject |
| E3 | Expanding while the style is Popup keeps the popup after switching back to Drop | low | Needs a click on the drop to land in the same instant as a style switch. Negligible, and the fix adds a branch | reject |
| E5 | A failed drop-window build stops the app starting | maybe-false | Transparency is supported on both target OSes, and the popup build fails the same way. Would need a platform where `transparent` errors | reject |
| E10 | Log rotation between CI runs breaks the run 2 slice | false | Rotation happens at 1 MB, and the test log is a few kB | reject |

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- expected: all pass, 87 + new.
- `cargo clippy --all-targets -- -D warnings`, plus the Windows target via the `llvm-rc` stub -- expected: clean.
- `npm run build` -- expected: tsc and vite build three pages.
- CI `windows-popup-test` -- expected: all checks pass, including the drop run.

**Manual checks:**
- Mac UI test with `HYDRATION_INTERVAL_SECS=15` and style Small drop, run against the ACs above while typing in TextEdit.
