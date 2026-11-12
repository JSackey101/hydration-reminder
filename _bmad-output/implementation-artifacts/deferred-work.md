- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Extract the scheduler loop's render-on-change and save-on-change decisions into a pure helper and unit-test them.
  evidence: maybe-false, medium if real; run_scheduler in src-tauri/src/lib.rs is untested glue, so a regression there (stale tray, missed save) would pass all tests. Settle by mutation-testing the comparison.
  home: story 3 (#3) — issue #9. Story 2 already tests the popup show/hide decision (`popup_change`); the tray-render and save decisions remain.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Add a log file (e.g. tauri-plugin-log) so warnings are visible in Windows release builds.
  evidence: main.rs sets windows_subsystem = "windows" in release, which drops stderr; corrupt-file resets and save failures leave no trace.
  home: story 4 (#4, packaging) — issue #10.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Check the Windows tray badge is legible at 16 px (100% DPI) and add a 16 px variant if not.
  evidence: maybe-false, medium if real; badge.rs draws 32 px with 2 px strokes, and Windows downscales tray icons at 100% DPI. Settle at the story 2 Windows checkpoint.
  home: story 2 done checkpoint (Windows test) — issue #11.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/2-reminder-popup.md`
  summary: Add a test that `capabilities/default.json` lists `popup::LABEL`, so a rename can't silently break the popup buttons.
  evidence: Both are the literal "popup" today; nothing ties them together (review V2).
  home: story 3 (#3), which changes the same capability file — issue #12.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/2-reminder-popup.md`
  summary: Check whether clicking the popup on Windows takes focus through the WebView2 child window, despite WS_EX_NOACTIVATE.
  evidence: Unverified (review B5); settled by the user's Windows test at the story 2 done checkpoint.
  home: story 2 done checkpoint (Windows test) — issue #13.
