- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Extract the scheduler loop's render-on-change and save-on-change decisions into a pure helper and unit-test them.
  evidence: maybe-false, medium if real; run_scheduler in src-tauri/src/lib.rs is untested glue, so a regression there (stale tray, missed save) would pass all tests. Settle by mutation-testing the comparison.
  home: story 3 (#3) — issue #9. RESOLVED in story 3: pure `effects()` tested; issue closed.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Add a log file (e.g. tauri-plugin-log) so warnings are visible in Windows release builds.
  evidence: main.rs sets windows_subsystem = "windows" in release, which drops stderr; corrupt-file resets and save failures leave no trace.
  home: story 4 (#4, packaging) — issue #10. RESOLVED in story 4: tauri-plugin-log writes to the OS log folder; every eprintln! is now a log macro.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Check the Windows tray badge is legible at 16 px (100% DPI) and add a 16 px variant if not.
  evidence: maybe-false, medium if real; badge.rs draws 32 px with 2 px strokes, and Windows downscales tray icons at 100% DPI. Settle at the story 2 Windows checkpoint.
  home: story 3 (#3) — issue #11. CI crop at 100% DPI shows the digit is ~5 px tall; user to choose: big number on the drop, number only, or keep. RESOLVED in story 3: big number, red when due; issue closed.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/2-reminder-popup.md`
  summary: Add a test that `capabilities/default.json` lists `popup::LABEL`, so a rename can't silently break the popup buttons.
  evidence: Both are the literal "popup" today; nothing ties them together (review V2).
  home: story 3 (#3), which changes the same capability file — issue #12. RESOLVED in story 3: `capability_lists_every_window_label`; issue closed.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/2-reminder-popup.md`
  summary: Check whether clicking the popup on Windows takes focus through the WebView2 child window, despite WS_EX_NOACTIVATE.
  evidence: Unverified (review B5); settled by the user's Windows test at the story 2 done checkpoint.
  home: story 2 done checkpoint (Windows test) — issue #13. RESOLVED: Windows CI focus test passed (run 43612648267); issue closed.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/2-reminder-popup.md`
  summary: When CAP-8 (hold reminders during full-screen apps) is built, stop the macOS popup showing over full-screen apps (`FullScreenAuxiliary` in popup.rs) or hold the reminder before it is shown.
  evidence: Sourcery S6 on PR #14; story 2 added FullScreenAuxiliary on purpose (review B6) because CAP-2 has no full-screen exception today.
  home: phase 2 — issue #5 (CAP-8), comment added.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/3-tray-menu-actions-and-interval-setting.md`
  summary: Add front-end tests for the settings page logic in `src/settings.ts` (timeout Off sends null, empty timeout fills in 2, non-whole interval refused).
  evidence: Review V4; there is no JS/TS test runner. Rust validates every value (`change_settings`, tested), so the risk is limited to the UI not sending the intended value.
  home: issue #15 (tech-debt, Todo on the board).
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/4-launch-at-login-and-packaging.md`
  summary: Front-end tests for the "Start at login" checkbox in `src/settings.ts` (a failed change reverts the box and shows the error; a failed load leaves it disabled while the other settings stay usable).
  evidence: Review V2 (story 4); there is no JS/TS test runner. Checked by hand in the story 4 Mac test (enable failure).
  home: issue #15 (tech-debt), extended with these cases.
