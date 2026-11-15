---
title: 'Story 4: Launch at login and packaging'
type: 'feature'
created: '2026-11-15'
status: 'done'
route: 'dispatch'
review_loop_iteration: 0
baseline_commit: '544f1ce3a9f9ee0faacce5700216cc0dde74acfb'
context:
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/SPEC.md'
  - '{project-root}/_bmad-output/specs/spec-hydration-reminder/stack.md'
---

<frozen-after-approval reason="human-owned intent — do not modify unless human renegotiates">

## Intent

**Problem:** The app only runs when started by hand from a dev checkout, so it can't meet the success signal ("starts at login … for a full work week") on either machine (CAP-7). The Windows release build also drops every warning because it has no console (#10).

**Approach:** Add a "Start at login" setting backed by `tauri-plugin-autostart`, a GitHub workflow that builds unsigned macOS and Windows installers with `tauri-action`, and a log file via `tauri-plugin-log`.

**Decisions (user, 2026-11-15):**
- The toggle is a "Start at login" checkbox in the settings window, not the tray menu.
- It is on by default for installed builds. The first launch of a release build turns it on once, then writes a marker file `autostart-initialised` in the app data folder so it never does so again. If the user turns it off, it stays off. Debug builds never register themselves.
- Installers build when a `v*` tag is pushed, into a draft GitHub Release, and also from a manual "Run workflow" button, as workflow artifacts.

## Boundaries & Constraints

**Always:** The OS login entry (macOS LaunchAgent, Windows `HKCU\…\Run`) is the only record of the setting. The UI reads it from the plugin each time and never stores a copy in `state.json`. Rust owns the change through its own command, so no plugin permission is granted to JS. A failed enable/disable shows an inline error and the checkbox reverts. Every current `eprintln!` becomes a `log` macro and goes to stdout and a rotating log file in the OS log folder.

**Never:** No code signing or notarization (non-goal). No auto-updater. `state.json` format unchanged. No change to the scheduler.

## I/O & Edge-Case Matrix

| Scenario | Input / State | Expected Output / Behavior | Error Handling |
|----------|--------------|---------------------------|----------------|
| Turn on | checkbox off → on | login entry created for the running exe; checkbox stays on | enable fails → inline error, checkbox back off |
| Turn off | on → off | login entry removed | disable fails → inline error, checkbox back on |
| Reopen settings | entry exists (set here or removed by hand in OS settings) | checkbox matches the OS entry | `is_enabled` fails → checkbox disabled, error shown |
| Fresh login, on | entry exists | app starts, tray icon shows, no window opens | N/A |
| Warning in release build | e.g. save fails on Windows | line written to the log file | N/A |

</frozen-after-approval>

## Code Map

- `src-tauri/Cargo.toml` -- add `tauri-plugin-autostart = "2"`, `tauri-plugin-log = "2"`, `log = "0.4"`.
- `src-tauri/src/lib.rs` -- `run()` (line ~236) registers plugins: add `tauri_plugin_log` (targets Stdout + LogDir, Info level, ~1 MB, `RotationStrategy::KeepOne`) first so setup warnings are logged, and `tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None)`. New commands `get_launch_at_login() -> Result<bool, String>` and `set_launch_at_login(enabled: bool) -> Result<(), String>` via `tauri_plugin_autostart::ManagerExt::autolaunch()`; register in `generate_handler!`. They take no `Core` lock. In `setup`, for release builds only (`cfg!(not(debug_assertions))`): if `app_data_dir/autostart-initialised` is missing, enable autostart, then write the marker. Write the marker only if the enable succeeded, so a failure is retried on the next launch; log a failure as a warning. Keep the "should we enable" decision as a small pure fn `(is_debug, marker_exists) -> bool` and test it. Replace `eprintln!` (20 sites across `lib.rs`, `settings.rs`, `tray.rs`, `popup.rs`, `persist.rs`) with `log::warn!` / `log::info!` / `log::debug!` keeping the text minus the `[hydration]` prefix.
- `src-tauri/src/settings.rs` -- `inner_size(420.0, 220.0)` → taller for the new row .
- `settings.html`, `src/settings.ts` -- add a "Start at login" checkbox row; it loads from `get_launch_at_login`, sends `set_launch_at_login` on change, and reverts on error. Reuse `show()` for messages. Starts disabled like the other inputs.
- `src-tauri/tauri.conf.json` -- `bundle.macOS.signingIdentity: "-"` (ad-hoc sign; an unsigned Apple Silicon bundle is reported as "damaged"). `targets: "all"` already gives `.app`/`.dmg` and `.msi`/NSIS `.exe`.
- New `.github/workflows/release.yml` -- `tauri-apps/tauri-action@v0`, matrix `macos-latest` (`--target universal-apple-darwin`, needs both Rust targets) and `windows-latest`; `permissions: contents: write`. Triggers: `push: tags: ['v*']` (tauri-action with `tagName`/`releaseDraft: true`) and `workflow_dispatch` (no release: tauri-action with no `tagName`, then `actions/upload-artifact` of `src-tauri/target/**/release/bundle/`). Reuse setup steps from `windows-popup-test.yml`.
- `README.md` -- "Install" section: where to download, and how to open an unsigned app (macOS: right-click → Open or `xattr -dr com.apple.quarantine`; Windows SmartScreen: More info → Run anyway). Log file locations. Remove the "(story 4)" note.

## Tasks & Acceptance

**Execution:**
- [x] `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs` -- plugins, logging setup, two autostart commands.
- [x] `src-tauri/src/*.rs` -- `eprintln!` → `log` macros.
- [x] `settings.html`, `src/settings.ts`, `src-tauri/src/settings.rs` -- the settings window checkbox, plus the first-launch default in `lib.rs` setup.
- [x] `src-tauri/tauri.conf.json` -- ad-hoc macOS signing.
- [x] `.github/workflows/release.yml` -- tag → draft release; manual → artifacts.
- [x] `README.md` -- install, unsigned-app and log notes.
- [x] `_bmad-output/implementation-artifacts/deferred-work.md` -- mark the #10 entry resolved.

**Acceptance Criteria:**
- Given the toggle is on, when the user logs out and back in on macOS, then the app is running with its tray icon and no window open.
- Given the release workflow runs, then it produces a macOS `.dmg` (universal) and a Windows installer, and the Windows popup test workflow still passes.
- Given a release build launched for the first time with no marker file, then autostart is on and the marker exists; given the user then turns it off, when the app restarts, then it stays off.
- Given a warning in a Windows release build, then it appears in `%LOCALAPPDATA%\com.jeffsackey.hydration-reminder\logs\`.

## Implementation Notes

- Plugins: `tauri-plugin-log` 2.10 is registered first. It writes to stdout and to `Hydration Reminder.log` in the OS log folder at Info level, in local time. Each file is capped at 1 MB with `RotationStrategy::KeepSome(2)`. `KeepOne` was not used because it deletes the whole file at the limit. `tauri-plugin-autostart` 2.7 uses `MacosLauncher::LaunchAgent`.
- `lib.rs` has new commands `get_launch_at_login` and `set_launch_at_login`. They take no `Core` lock and return a message that the page shows. `default_autostart` runs in `setup` before the state loads. The pure `should_default_autostart` has 1 test.
- `js_gets_no_plugin_permissions` checks that the capability is still `["core:default"]`.
- All 20 `eprintln!` calls are now `log::warn!` or `log::info!`. The `[hydration]` prefix is gone because the log format adds a target. The debug interval override logs at Info so the default filter still shows it.
- Settings window: a "Start at login" row was added and the window is now 420×264. The checkbox loads after the other settings, so if it fails to load the rest still works and the box stays disabled. A failed change reverts the box and shows the error.
- Release: `tauri.conf.json` has `bundle.macOS.signingIdentity: "-"`. `release.yml` runs a macOS universal and a Windows matrix. On a tag it checks the tag against the version, then makes a draft release. On a manual run it uploads the installers as artifacts.
- **Mac UI test, release build:** the local `tauri build --bundles app` was ad-hoc signed (`Signature=adhoc`, 4.4 MB).
  - First launch: it wrote `autostart-initialised` and `~/Library/LaunchAgents/Hydration Reminder.plist` (pointing at the bundle exe, `RunAtLoad`), and the log file appeared.
  - `launchctl bootstrap` of that plist started the app, the same thing launchd does at login.
  - With the plist removed (off) and the marker kept, a relaunch did not recreate it.
  - macOS shows its own "App Background Activity" notice when the LaunchAgent is added. That is expected.
  - The test marker and plist were removed and `state.json` was restored.
- **Mac UI test, dev build:** no plist and no marker on launch.
  - Settings showed "Start at login" off. Ticking it created the plist and showed "Saved.". A closed and rebuilt window showed it on. Unticking removed the plist.
  - With `~/Library/LaunchAgents` made read-only, ticking showed "Couldn't turn on start at login: Permission denied (os error 13)", the box went back off and a WARN line was logged. The permissions were restored afterwards.
- Matrix coverage: the Turn on, Turn off and Reopen rows, plus the enable error, were checked in the Mac UI test. "Fresh login" was checked with the launchd load above. The `is_enabled` and disable error paths and the Windows registry entry have no automated test. There is no front-end test runner yet (#15), and CI can't log in to Windows, so the user checks Windows on their PC.
- Verified: `cargo test` 72 passed; clippy clean for macOS and `x86_64-pc-windows-msvc`; `npm run build` passes.
- **Review patches.** See the Review Triage Log for the full list. The main changes:
  - **Startup:** launch at login now has three cases, decided by the pure `autostart_step`:
    - **EnableAndMark:** the first release launch turns it on and writes the marker.
    - **Refresh:** a later release launch with it already on enables it again, so the login entry follows the app if it was first opened from the `.dmg` or a translocated path.
    - **Nothing:** it was turned off, or this is a debug build.
  - **Tests:** `run_autostart_step` takes the enable call as a closure. It is tested for "the marker is written only after a successful enable" and for "Refresh never writes the marker". The single-instance plugin is registered first again.
  - **Release workflow:** `tagName` is set only on a tag push. A `pull_request` trigger runs on changes to the packaging files, so this PR gets a run. Artifacts upload on any non-tag run. `cargo test` runs before the build, and the tag check covers all three version files.
  - **Windows CI:** it captures stdout and checks that the log file holds "reminder due".
  - **README:** macOS 15+ "Open Anyway", start from Applications rather than the `.dmg`, uninstall notes, and all three version files.
- **Re-tested on Mac after the patches** with a rebuilt release app, copied to two folders:
  - First launch from A: the plist pointed at A and the marker was written.
  - Launch from B: the plist followed to B.
  - Plist removed by hand, then launch from A: it stayed off.
  - Cleaned up afterwards and `state.json` restored.
- **After the patches:** `cargo test` 74 passed; clippy clean for macOS and `x86_64-pc-windows-msvc`; `npm run build` passes.

## Spec Change Log

## Review Triage Log

Reviewers: B = Blind Hunter, E = Edge Case Hunter, V = Verification Gap.

| # | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| B1/E5 | Login entry saved while the app runs from the `.dmg` or a translocated path; the marker stops it ever being fixed | medium | Confirmed: the plugin stores `current_exe()` (autostart `lib.rs` setup), and the marker blocks any rewrite | patch: Refresh step re-enables on each release launch while on; tested, and checked on Mac by moving the app |
| B11/V1 | `default_autostart` side effects untested; only the predicate is | medium | Confirmed: writing the marker before `enable`, or swapping the arguments, keeps the old test green | patch: `run_autostart_step` with an injected enable; marker-on-success, failure-retries and Refresh tests |
| V3 | Release workflow can't run before merge (`workflow_dispatch` needs the default branch) | medium | Confirmed: `gh run list --workflow release.yml` gives 404 | patch: `pull_request` trigger on the packaging files; artifacts on non-tag runs |
| V4 | Nothing checks that warnings reach the log file (#10) | medium | Confirmed: the Windows script never reads a log | patch: the Windows test checks `Hydration Reminder.log` contains "reminder due" and uploads it |
| B3 | README's right-click → Open no longer works on macOS 15+ | medium | Sequoia removed the Control-click bypass; Privacy & Security → Open Anyway is the path | patch: README |
| B8/E1/E8 | A manual run started from a tag creates a draft release | low | Confirmed: `tagName` tested the ref, not the event | patch: `github.event_name == 'push'` |
| B5 | Release builds without running tests | low | Confirmed: no workflow runs `cargo test` | patch: `cargo test` step before the build |
| B6 | Version lives in three files; only one checked | low | Confirmed: `package.json` and `Cargo.toml` also say 0.1.0 | patch: tag check covers all three; README lists them |
| B4 | Uninstall leaves the login entry | low | Confirmed: neither the plist nor the Run value is removed by uninstall | patch: README uninstall notes |
| B2 | "Debug builds never register" overpromises; the checkbox registers in dev | low | Confirmed: comment wording only (the checkbox in dev is intended; used in the Mac test) | patch: comment now says "never registers on its own" |
| V5 | Windows test captures only stderr, but logs now go to stdout | low | Confirmed: `-RedirectStandardError` only | patch: capture stdout too |
| V6 | Log plugin registered before single-instance | low | The single-instance docs say register it first; otherwise a second launch initialises other plugins before exiting | patch: single-instance first again |
| V2 | Checkbox revert / disabled-on-failure have no front-end test | low | Confirmed: no JS test runner | defer: issue #15 (comment added; deferred-work entry) |
| B9 | tao/wry/tauri Info logs flood the 1 MB file | false | Every line in the Mac log file across all test runs is `hydration_reminder_lib`; Windows can be checked in the uploaded CI log | rejected |
| B10/E6 | Shared status line: one error can be overwritten; revert guesses the state | low | Needs two failures at once. A failed enable/disable leaves the OS entry unchanged, so `!enabled` is the real state | rejected |
| E3/E7 | Marker write fails after enable → re-enabled every launch | low | Needs an unwritable app data folder, which also stops `state.json` saving, so the app is already broken there | rejected |
| E4 | `marker.exists()` is false on an IO error | low | Same folder as above; same reasoning | rejected |
| E2 | The two matrix jobs may create duplicate draft releases | maybe-false | tauri-action looks up an existing draft for the tag; settle on the first tag push. Only low if real (delete the extra draft) | rejected |
| B7 | Actions pinned to tags, not SHAs, with `contents: write` | low | Personal repo; same convention as `windows-popup-test.yml` | rejected |

## Verification

**Commands:**
- `cd src-tauri && cargo test` -- expected: all pass (70+).
- `cargo clippy --all-targets -- -D warnings` and the Windows-target clippy (with the `llvm-rc` stub) -- expected: clean.
- `npm run build` -- expected: success.
- Release workflow run (on the branch via `workflow_dispatch`) -- expected: both installers built.

**Manual checks (if no CLI):**
- Mac UI test: toggle on → `~/Library/LaunchAgents/Hydration Reminder.plist` exists; toggle off → gone; reopening settings shows the right state. Remove any test plist afterwards.
- Log file appears under `~/Library/Logs/com.jeffsackey.hydration-reminder/`.
- User installs the Windows build and checks it starts at login (CI can't log in).

### Sourcery review on PR #17

| # | Finding | Verdict | Evidence | Route |
|---|---|---|---|---|
| S1 | PR builds get a `contents: write` token | false | GitHub gives `pull_request` runs from forks a read-only token; same-repo branches are the owner's own code | rejected |
| S2 | An entry removed between the startup check and the refresh is recreated | low | Needs the entry removed within the milliseconds between `is_enabled` and `enable` at startup | rejected |
| S3 | First enable fails, then the user turns it on and off → re-enabled on the next launch | low | Real, but needs a failed first enable (it only writes a plist or Run value); the user can turn it off again and it sticks once the marker is written | defer: issue #18 (user: non-blocking but valid → issue) |
| S4 | Marker write fails → off is undone on every launch | low | Same as E3: the folder also holds `state.json`, so the app can't save state there either | rejected |
| S5 | An `is_enabled` error skips the refresh | low | On macOS it checks whether the plist exists; the only effect is one skipped refresh | rejected |
| S6 | `pull_request.paths` misses `package.json` and the lock files | low | Dependency bumps still build on the tag run, and `windows-popup-test` builds every PR | rejected |
| S7 | Closing and reopening Settings during a pending toggle shows the old state | low | The request takes milliseconds; reopening shows the real state | rejected |
| S8 | The new workflow can't run on its own PR | false | Release run 43752243580 (`pull_request`) succeeded and uploaded the universal `.dmg`, `.msi` and `-setup.exe` | rejected |
| S9 | Issue #10 asked for the app data folder, not the log folder | false | #10 said "e.g."; the OS log folder is the standard place and the README documents it. The Windows CI check found the file there | rejected |
| S10 | `../../releases` leaves the repository | false | From `/<owner>/<repo>/blob/main/README.md`, `../../releases` resolves to `/<owner>/<repo>/releases` | rejected |

- **CI on PR #17:**
  - Windows popup test passed, including the new "Log file records the reminder (#10)" check.
  - The Release workflow built `Hydration Reminder_0.1.0_universal.dmg` (`lipo`: x86_64 arm64; `Signature=adhoc`), plus `_x64_en-US.msi` and `_x64-setup.exe`.
