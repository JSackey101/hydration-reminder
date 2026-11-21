---
title: 'Story 5: Keyboard shortcuts for Drank and Snooze'
type: 'feature'
created: '2026-11-21'
status: 'ready-for-dev'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '00111354301ea217ca095e99ee1c231a516e967f'
context:
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/state-machines.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** Answering a reminder means reaching for the mouse and clicking the popup, which breaks the flow of typing (CAP-15).

**Approach:** Add global shortcuts through `tauri-plugin-global-shortcut`. They're registered when a reminder shows and released when it goes. Each one triggers the same Drank or Snooze as the buttons. They're stored in settings and recorded in the settings window, and the popup shows them as hints.

**Decisions (user, 2026-11-21; see the memlog):**
- Shortcuts are registered only while a reminder is showing.
- The defaults are Ctrl+Alt+D (Drank) and Ctrl+Alt+S (Snooze), shown as ⌃⌥D and ⌃⌥S on macOS.
- The defaults stay despite the AltGr clash on some Windows layouts; the README notes it.
- The same combo is refused for both actions.
- The popup shows the current keys as small hints under its buttons.
- While a reminder is showing, Settings' Record buttons are disabled, with the message "Answer the reminder first".

## Boundaries & Constraints

**Always:**
- **Threading.** Code on the main thread never takes the `Core` lock.
  - Registering and releasing happen in the `fx.popup` main-thread task: register after `popup::show`, release after `popup::hide`. In setup this runs inline under the lock, which is harmless.
  - Only single `on_shortcut` / `unregister` calls are made. Never call `register_multiple`, `on_shortcuts` or `unregister_all`: they hold the plugin's lock while waiting on the main thread.
- **Answering by key.**
  - Each shortcut is registered with `on_shortcut`, which captures its `Answer`. Only `Pressed` counts; on Windows, Released comes from another thread, so ignore it.
  - The handler hands off with `spawn_blocking` and then answers through `apply`. That ticks first and only answers if the phase is still Prompting, so a key press racing a click, a timeout or a tray Log is a no-op.
  - A shortcut answer never moves focus: no `restore_focus` and no app activation.
- **Release** unregisters exactly what was registered, kept in a small managed list that only the main thread touches. A failed register or unregister is logged as a warning, and the buttons still work.
- **Recording** is allowed only while no reminder is showing. `save_shortcuts` runs inside `apply` and checks under the lock:
  - it refuses if the phase is Prompting;
  - it validates both combos;
  - it test-registers each one (`on_shortcut`, then `unregister`) and saves only if all pass.
- **Validation:**
  - Each combo needs Ctrl or ⌘/Win; Alt and Shift may be added. This rules out bare keys, Shift-only, and the Option-only combos macOS 15 refuses.
  - The two must differ, compared by `Shortcut::id()`.
  - Store the canonical form `Shortcut::into_string()`.
- **"Can't register"** is only detectable on Windows. On macOS a combo owned by another app registers without error and the other app wins; the README says so.
- **Old files.** An old `state.json` loads with the defaults. A stored string that won't parse falls back to its default, with a warning.

**Never:**
- Don't register shortcuts while no reminder is showing.
- Don't give JS any global-shortcut permission.
- Don't change the scheduler (`tick`/`answer`).
- No small drop (that's story 6).

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Drank by key | Prompting, typing in another app, presses ⌃⌥D | as clicking Drank; popup hides; focus unchanged; no "d" typed | N/A |
| Snooze by key | Prompting, presses ⌃⌥S | as Snooze | N/A |
| No reminder | Waiting/Snoozed, presses ⌃⌥D | not registered; other app gets the keys | N/A |
| Ended another way | Prompting → button / tray Log / timeout | shortcuts released | unregister error → warn |
| Key races a click | press arrives after the click answered | no-op (phase no longer Prompting) | N/A |
| Combo taken (Windows) | another app owns Ctrl+Alt+D | reminder shows; that key doesn't work | warn; buttons work |
| Record valid | no reminder; record ⌃⌥⇧W for Drank | saved; used from the next reminder | N/A |
| Record refused | bare key, Shift-only, Alt-only, same as other, can't register (Windows) | inline error naming the reason; old combo kept | N/A |
| Record during reminder | Prompting | Record disabled, "Answer the reminder first"; command refuses too | N/A |
| Old or bad state | no shortcut fields, or unparsable string | defaults | warn on unparsable |

</frozen-after-approval>

## Code Map

- `src-tauri/src/scheduler.rs` `Settings` (`#[serde(default)]`, lines 9-25) -- add `drank_shortcut` and `snooze_shortcut` (String). `Default` gives `"control+alt+KeyD"` and `"control+alt+KeyS"`.
- New `src-tauri/src/shortcuts.rs`:
  - pure `parse_or_default(s, default) -> Shortcut`;
  - pure `validate(drank, snooze) -> Result<(String, String), String>`: canonical strings, or a message;
  - pure `label(&Shortcut, macos: bool) -> String`: macOS "⌃⌥⇧⌘D", Windows "Ctrl+Alt+Shift+Win+D". `KeyX`→X, `DigitN`→N, `F1`–`F24`, and punctuation codes become their character; anything else shows its code name.
  - `register(app, drank, snooze)` and `release(app)`: main thread only, using the managed `Registered(Mutex<Vec<Shortcut>>)`. `register` emits `shortcut-labels` to the popup.
- `src-tauri/src/lib.rs`:
  - Add the plugin with `tauri_plugin_global_shortcut::Builder::new().build()`, with no global handler, registered after single-instance.
  - In `apply`'s popup task (lines 96-110), copy the two parsed shortcuts into the closure.
  - New `answer_by_shortcut` is like `answer_popup` (line 122) but skips `restore_focus`.
  - Add `drankShortcut`, `snoozeShortcut`, `drankLabel`, `snoozeLabel` and `reminderShowing` to `SettingsView`, and update `settings_view_uses_the_page_field_names` (around line 544).
  - New commands: `save_shortcuts(drank, snooze) -> Result<(), String>` and `get_shortcut_labels`, for the popup on load.
  - Emit `reminder-showing` (bool) to the settings window in the popup task.
- `tauri-plugin-global-shortcut` 2.4.1 / `global-hotkey` 0.8.0:
  - `register`/`unregister` block on the main thread (`run_main_thread!`, lib.rs:111).
  - Pressed events are dispatched on the main thread, without the plugin lock held during the handler (lib.rs:71-90).
  - Windows registers the same id twice → `AlreadyRegistered`.
  - macOS uses non-exclusive Carbon registration (macos/mod.rs:116).
- `settings.html`, `src/settings.ts`, `settings.rs` (size):
  - Add a "Shortcuts" section: for Drank and for Snooze, the label plus a **Record** button.
  - While recording, ignore lone modifier keydowns (`Shift/Control/Alt/Meta` + `Left/Right`) and `e.repeat`. On the first other key, `preventDefault`, build the combo and send it. Escape cancels.
  - The key comes from `e.code`, except on Windows, where letters and digits come from `e.key`, uppercased, because Windows registers by layout key.
  - Disable Record while `reminderShowing`, updated by the event.
- `popup.html`, `src/popup.ts`, `src/popup.css`, `popup.rs` `SIZE` (340×150):
  - Add a hint line under each button, filled from `get_shortcut_labels` on load and from `shortcut-labels` events.
  - Make the popup about 18 px taller, and update `$y` in the Windows test to match (now `35*s`, ps1:139).
- `scripts/windows-popup-test.ps1`:
  - popup 1: click Drank (as now);
  - popup 2: `SendKeys "^%d"` → check the popup hides, focus stays in Notepad and no stray "d" appears;
  - popup 3, one interval later: click Snooze (moved from popup 2).
- `README.md` -- add the shortcuts to "Using it", plus notes on AltGr (Windows) and on another app owning a combo (macOS: no error, the other app wins).

## Tasks & Acceptance

**Execution:**
- [ ] `scheduler.rs`, `shortcuts.rs` -- fields, `parse_or_default`, `validate`, `label` + tests (every refused case, id-based duplicate, label forms, old/bad JSON)
- [ ] `lib.rs` -- plugin, register/release in the popup task, `answer_by_shortcut`, `save_shortcuts`, `get_shortcut_labels`, view + test update
- [ ] `settings.html`, `src/settings.ts`, `settings.rs` -- record UI, disabled during reminders
- [ ] `popup.html`, `src/popup.ts`, `src/popup.css`, `popup.rs` -- key hints
- [ ] `scripts/windows-popup-test.ps1` -- shortcut on popup 2, Snooze on popup 3, new `$y`
- [ ] `README.md`

**Acceptance Criteria:**
- Given a reminder on macOS while typing in TextEdit, when ⌃⌥D is pressed, then the count rises, the popup hides, TextEdit keeps focus and no "d" is typed.
- Given no reminder, when ⌃⌥D is pressed in TextEdit, then nothing is logged.
- Given no reminder, when ⌃⌥⇧W is recorded for Drank, then Settings shows it, the next popup's hint reads ⌃⌥⇧W, and pressing it answers Drank.
- Given each refused case, when it's recorded, then an inline error names the reason and the old combo stays.
- Given a reminder showing, when Settings is open, then both Record buttons are disabled with "Answer the reminder first".
- Given a `state.json` from v0.1.0, when the app starts, then the defaults are used and the hints read ⌃⌥D / ⌃⌥S.
- Given Windows CI, when Ctrl+Alt+D is sent on popup 2, then the popup hides, focus stays in Notepad and no "d" appears.

## Implementation Notes

## Spec Change Log

## Review Triage Log

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- expected: all pass.
- `cargo clippy --all-targets -- -D warnings` and the Windows-target clippy -- expected: clean.
- `npm run build` -- expected: success.

**Manual checks (if no CLI):**
- Mac UI test: the acceptance criteria above.
