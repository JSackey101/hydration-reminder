- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Extract the scheduler loop's render-on-change and save-on-change decisions into a pure helper and unit-test them.
  evidence: maybe-false, medium if real; run_scheduler in src-tauri/src/lib.rs is untested glue, so a regression there (stale tray, missed save) would pass all tests. Settle by mutation-testing the comparison.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Add a log file (e.g. tauri-plugin-log) so warnings are visible in Windows release builds.
  evidence: main.rs sets windows_subsystem = "windows" in release, which drops stderr; corrupt-file resets and save failures leave no trace.
- source_spec: `_bmad-output/specs/spec-hydration-reminder/stories/1-core-scheduler-tray-count-midnight-reset.md`
  summary: Check the Windows tray badge is legible at 16 px (100% DPI) and add a 16 px variant if not.
  evidence: maybe-false, medium if real; badge.rs draws 32 px with 2 px strokes, and Windows downscales tray icons at 100% DPI. Settle at the story 2 Windows checkpoint.
