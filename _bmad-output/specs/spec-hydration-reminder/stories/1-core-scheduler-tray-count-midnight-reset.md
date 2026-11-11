---
title: 'Story 1: Core scheduler, tray count and midnight reset'
type: 'feature'
created: '2026-11-10'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: 'NO_VCS'
context:
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/stack.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/state-machines.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** There is no app yet. The reminder needs a scheduler that tracks due times reliably across sleep, quit and midnight, plus a tray icon that shows today's drink count (CAP-1, CAP-6, and the due-time half of CAP-2).

**Approach:** Scaffold a tray-only Tauri v2 app (TS + Vite front end, no visible window). Put all scheduling in a pure Rust `tick(now, state)` with an injected clock, persist state to a local JSON file, and render the count in the tray. Until story 2 adds the popup, a due reminder shows as a log line plus a marker in the tray.

## Boundaries & Constraints

**Always:** Times are stored as absolute UTC instants; the local date comes from the injected `now`'s time zone. State is saved whenever it changes (not on every tick). Defaults: 45 min interval, 10 min snooze, popup timeout off. Missed reminders collapse into one prompt. On Windows the count is a number badge drawn onto the tray icon (tooltip shows it too), since Windows tray icons can't show text. A day rollover archives yesterday's count (`history: {date: count}`, kept for CAP-11) and then clears today's drinks. A new day starts fresh (user decision): on rollover, if `next_due_at` is already past and the phase isn't Prompting, set `next_due_at = now + interval` and phase → Waiting, so there's no leftover overdue reminder in the morning. A due time still ahead (working past midnight) and a popup already showing are left alone.

**Never:** No popup window, Drank/Snooze, "Log a drink", undo, settings UI or autostart (stories 2–4). No in-memory countdown timers for due times. No network calls. No OS notifications.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| First launch | no store file | `next_due_at = now + 45 min`, `day = today`, count 0 | N/A |
| Not yet due | `now < next_due_at` | action none, state unchanged | N/A |
| Due | Waiting, `now >= next_due_at` | action show_popup once; phase → Prompting | N/A |
| Already prompting | Prompting, still past due | action none (no repeat) | N/A |
| Slept past several due times | `now = next_due_at + 3h` | exactly one show_popup | N/A |
| Midnight while running/asleep/quit | `local_date(now) > day`, 5 drinks | history[day]=5, drinks cleared, `day` = new date, action reset_day | N/A |
| Rollover and overdue together | new day, Waiting/Snoozed, `next_due_at` past | reset; `next_due_at = now + interval`, phase → Waiting; no show_popup | N/A |
| Rollover, due still ahead | 23:50 → 00:01, due 00:15 | reset; `next_due_at` stays 00:15 | N/A |
| Rollover while prompting | Prompting at midnight | reset; stays Prompting | N/A |
| DST change | tick across a 23 h / 25 h local day | one reset, at local midnight; a 45 min interval is 45 real minutes | N/A |
| Clock set backwards past midnight | `local_date(now) < day` | no reset, no history change | N/A |
| Due time too far ahead (clock corrected backwards) | Waiting/Snoozed, `next_due_at > now + interval` (or `+ snooze` when Snoozed) | clamp `next_due_at` to that bound | N/A |
| Restart while prompting | stored phase Prompting, past due | on load phase → Waiting; the next tick gives show_popup | N/A |
| Corrupt/unreadable store | bad JSON | copy it to `state.json.bak`, log a warning, start from defaults | not fatal |

</frozen-after-approval>

## Code Map

Greenfield: the repo holds only `_bmad/`, `_bmad-output/` and `.claude/`. Not a git repo yet.

- `_bmad-output/specs/spec-hydration-reminder/` -- the contract; follow stack.md.
- `_bmad/`, `.claude/`, `_bmad-output/` -- don't modify; keep them tracked in git.

## Tasks & Acceptance

**Execution:**
- [x] repo root -- `git init`, plus `.gitignore` (node_modules, dist, src-tauri/target) -- story 4's CI needs a repo, and review needs diffs.
- [x] repo root -- scaffold with create-tauri-app (vanilla-ts, npm); remove the default main window from `tauri.conf.json`; keep `index.html` minimal -- tray-only app.
- [x] `src-tauri/src/scheduler.rs` -- `State` (serde), `Action {None, ShowPopup, ResetDay}`, `tick<Tz: TimeZone>(now: DateTime<Tz>, state) -> (State, Action)`; `State::new(now)` for first launch; `on_load(state) -> State` (Prompting → Waiting); no I/O -- pure and testable.
- [x] `src-tauri/src/scheduler.rs` (tests) -- one unit test per I/O matrix row, with a fake clock; use `chrono-tz` (dev-dependency) for DST -- the edge cases are the main risk.
- [x] `src-tauri/src/persist.rs` -- load/save `State` as `state.json` in the app data dir with `serde_json`; save atomically (write `state.json.tmp`, then rename); decoding is a pure `decode(json) -> Result<State>` with unit tests; on failure back up to `state.json.bak` and use defaults -- survives restarts and crashes mid-save.
- [x] `src-tauri/src/tray.rs` -- build the tray (drop icon, menu with a count header and Quit); `render(count, prompting)`: macOS title "💧 N" (plus "⏰" while prompting), Windows: icon with the count drawn on as a badge, plus tooltip. Badge drawing is a pure `badge_rgba(count) -> RGBA` (built-in digit bitmaps, "99+" cap) unit-tested on any OS; only applying it is `cfg(windows)` -- CAP-1.
- [x] `src-tauri/src/lib.rs` -- setup: load state, run a tick at startup, then every 1 s on a background thread (this also covers wake, since it compares wall-clock times); apply actions, save only if the state changed, re-render the tray; `HYDRATION_INTERVAL_SECS` (debug builds only) overrides the interval in memory and is never saved; macOS `ActivationPolicy::Accessory` (no Dock icon) -- wiring.

**Acceptance Criteria:**
- Given a fresh install, when `npm run tauri dev` starts, then a drop icon with "💧 0" appears in the macOS menu bar, with no Dock icon and no window.
- Given the app is running with a short test interval (env var `HYDRATION_INTERVAL_SECS`, debug builds only), when the due time passes, then within 5 s the log shows a "reminder due" line and the tray shows ⏰.
- Given state on disk, when the app quits and relaunches on the same day before the due time, then the count and `next_due_at` are unchanged.
- Given the tray menu, when Quit is clicked, then the app exits cleanly.

## Implementation Notes

- Implemented directly in the main session (no subagent). Files: `src-tauri/src/{scheduler,persist,tray,badge,lib}.rs`, `Cargo.toml` (tauri `tray-icon`, chrono; dev: chrono-tz), `tauri.conf.json` (no windows), `package.json`/capabilities (opener plugin removed), minimal `index.html` + `src/main.ts`, `README.md`.
- Badge drawing lives in its own `badge.rs` (pure, dead code on macOS by design) so macOS clippy stays clean; `tray.rs` uses it only under `cfg(not(target_os = "macos"))`.
- macOS tray has no icon image, only the title "💧 N": the emoji is the drop, and an icon beside it would show two drops.
- `tick` returns a single `Action`; `State` carries a `#[serde(skip)] interval_override_secs` for the debug env var.
- Windows check needs `llvm-rc` for tauri-build's resource step; not installed (Homebrew LLVM is >1 GB). The check ran with a stub `llvm-rc` that writes an empty `.res`: fine for `cargo check`/clippy (nothing is linked), but real Windows builds need the story 4 CI.
- Verified: 26 tests pass; clippy `-D warnings` clean on macOS and on `x86_64-pc-windows-msvc`; debug `.app` + `.dmg` build; with `HYDRATION_INTERVAL_SECS=5` the log printed "reminder due" on time and `state.json` kept `interval_min: 45`; quit + relaunch left `state.json` identical, no `.tmp` left behind.
- Not verified here (no screen-capture permission): the menu-bar title, no Dock icon, and the Quit item. User checks these by eye.

## Spec Change Log

## Review Triage Log

Pass 1 (blind hunter B, edge-case hunter E, verification gap V). No intent_gap or bad_spec; patches applied, defers logged.

| # | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| E1/E12/V3 | Unreadable or invalid-UTF-8 store not backed up; unreadable path untested | medium | `read_to_string` failed with InvalidData before decode, skipping the `.bak` branch; next save overwrote it | patch: read bytes, decode with `from_slice`; tests `invalid_utf8_is_backed_up`, `unreadable_path_gives_defaults` |
| E2 | Backup copy fails, next save overwrites the only copy | low | Real but needs corrupt file plus failed copy; fix adds a save-suppression branch | rejected (unlikely, adds complexity) |
| E3/B5 | Second corruption overwrites the single `.bak` | low | Real; fix would change the frozen `state.json.bak` contract | rejected (fix edits spec) |
| E4/B4 | No fsync before rename; power loss can leave an empty file | medium | `fs::write` + rename with no `sync_all` | patch: `sync_all` on the temp file |
| E5/B7 | Two instances race on `state.json` | medium | Nothing prevented a second launch; story 4 autostart makes it likely | patch: `tauri-plugin-single-instance`; second launch verified to exit |
| E6 | Stored day far in the future never resets | low | Needs a clock wrong by days, then corrected; fix adds a guard against the frozen clock-back row | rejected |
| E7/B12 | Interval or snooze of 0 prompts nonstop | low | Only via hand-edited file or debug env; fix is a direct `.max(1)` | patch: floor to 1; test `zero_settings_are_floored_to_one_minute` |
| E8 | Skipped days missing from history | false | Absent day = 0 drinks for the CAP-11 chart; no data is lost | rejected |
| E9 | Tray stays stale if render fails | low | Render only fails if the tray is gone; fix adds tracking | rejected |
| E10/B3 | Failed save logs every second | low | Real under disk full; fix adds a guard; rare | rejected |
| E11 | `app_data_dir` failure panics at launch | low | Very unlikely on desktop; fix adds a fallback mode | rejected |
| E13 | Windows shows no visible due marker (tooltip only) | medium | `badge_rgba` ignored `prompting`; intent asks for a tray marker | patch: badge turns red while due; test `due_badge_changes_colour` |
| E14 | First tick always writes the file | low | Harmless single write per launch, also persists `on_load` changes | rejected |
| V1/B9 | Debug override only works via the clamp, untested through `tick` | medium | Pre-verified by V | patch: test `override_shortens_pending_due_time_through_tick` |
| V2/B10 | Scheduler loop (render/save on change) untested | maybe-false (medium if real) | Glue in an infinite loop; behaviour checked by hand (relaunch, 5 s run) | defer |
| B1 | No schema evolution: new fields would wipe old files | medium | No `#[serde(default)]`; a story 2/3 field would fail decode and reset the data | patch: defaults on optional fields and `Settings`; doc rule for new fields; test `missing_optional_fields_still_load` |
| B2 | Warnings invisible in Windows release builds | medium | `windows_subsystem = "windows"` drops stderr; no log file | defer |
| B6 | `history` grows without bound | low | ~365 small entries a year; negligible | rejected |
| B8 | No tick before first render | low | Setup rendered the loaded state; a new day showed yesterday's count until the first tick | patch: tick once in setup before rendering |
| B11 | DST real-minutes check didn't call `tick` | medium | Test did the arithmetic by hand | patch: tick-driven check that it prompts at 02:15 local, not before |
| B13 | Stale `main` capability, `csp: null` | low | No windows exist yet, so nothing is exposed | rejected (revisit in story 2) |
| B14 | Badge may be unreadable at 16 px | maybe-false (medium if real) | Can't view a Windows tray here | defer (check at story 2 Windows checkpoint) |
| B15 | README gaps; icons missing from diff | low / false | README lacked state location and `llvm-rc`; icons were left out only by the review diff filter | patch README; icons rejected (present on disk) |

## Design Notes

`tick` order: rollover check first (returns ResetDay right away), then the clamp, then the due check, which only fires from Waiting or Snoozed. `State` = `{ settings: {interval_min, snooze_min, popup_timeout_min: Option}, phase: Waiting|Prompting|Snoozed, next_due_at, popup_shown_at: Option, day: NaiveDate, drinks: Vec<{at, prev_next_due_at}>, history: BTreeMap<NaiveDate, u32> }`. Count = `drinks.len()`. Snoozed and timeout fields exist now so the format stays stable for stories 2–3.

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- expected: all scheduler tests pass
- `cd src-tauri && cargo clippy -- -D warnings` -- expected: no warnings
- `npm run tauri build -- --debug` -- expected: builds on macOS
- `cd src-tauri && cargo check --target x86_64-pc-windows-msvc` -- expected: Windows code compiles (`rustup target add` first)

**Manual checks (if no CLI):**
- Run `npm run tauri dev` and check the first three ACs by eye. Windows checks wait for the story 2 checkpoint on the user's PC.
