# Contributing

Thanks for helping. This is a small, single-maintainer project, so the process is light, but every change goes
through a pull request and a review.

## Before you start

- **Open or comment on an issue first** for anything bigger than a typo, so we can agree on the approach before you
  write code. Issues labelled `phase-2` are planned features; `tech-debt` issues are smaller clean-ups.
- **Security problems:** don't open a public issue. See [SECURITY.md](SECURITY.md).
- By contributing, you agree your work is released under the project's [MIT license](LICENSE).

## Set up

You need:

- [Node.js](https://nodejs.org/) 22 and npm
- [Rust](https://rustup.rs/) (stable)
- Tauri's system dependencies for your OS: see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)
  (Xcode Command Line Tools on macOS; Microsoft C++ Build Tools and WebView2 on Windows)

Then:

```sh
git clone https://github.com/<you>/hydration-reminder.git   # your fork
cd hydration-reminder
npm install
npm run tauri dev                              # run the app
HYDRATION_INTERVAL_SECS=15 npm run tauri dev   # debug builds only: remind every 15 s
```

A dev run uses the same saved state as an installed copy (`state.json`, see the README's Troubleshooting section).
Back it up first if you also use the app day to day.

## Make a change

1. Fork the repo and create a branch from `main` (e.g. `fix-tray-tooltip`).
2. Make the change, with tests where the logic can be tested (see below).
3. Run the checks:

   ```sh
   cd src-tauri && cargo fmt --check && cargo test && cargo clippy --all-targets -- -D warnings
   cd .. && npm run build
   ```

4. Try it in the app on your OS. Say which OS in the PR; the maintainer or CI covers the other one.
5. Open a pull request against `main` and fill in the template. Keep each PR to one change.

### What happens to your PR

- **CI:** the Windows popup test builds the app on Windows and checks the popup never steals keyboard focus. PRs
  that touch the packaging files also build the macOS and Windows installers. A bot (Sourcery) leaves review
  comments; treat them as suggestions.
- **First-time contributors:** GitHub waits for the maintainer to approve before CI runs on your PR. That's a safety
  setting for workflows run from forks, not a judgement on your code.
- **Review:** `main` is protected. A PR needs the maintainer's approval and passing checks, and all review threads
  resolved. Pushing new commits after an approval asks for a fresh review.
- **Merge:** the maintainer merges with a merge commit. Force-pushes and deleting `main` are blocked, and only the
  maintainer can create `v*` release tags.

## How the code is laid out

- `src-tauri/src/scheduler.rs`: all the reminder logic, as pure functions of `(now, state)`. No I/O, no clock reads,
  so it is fully unit-tested with a fake clock. Put new behaviour here and test it here.
- `src-tauri/src/lib.rs`: wires the scheduler to the app. Every state change goes through `apply`, which works out
  the effects (redraw tray, show/hide popup, save) with the pure, tested `effects()`.
- `tray.rs`, `popup.rs`, `settings.rs`, `badge.rs`, `persist.rs`: the tray icon and menu, the reminder popup, the
  settings window, the Windows tray number, and saving `state.json`.
- `popup.html`, `settings.html`, `src/*.ts`, `src/*.css`: the two web pages. They only show and send values; Rust
  owns and validates all state.
- `_bmad-output/specs/spec-hydration-reminder/`: the product spec (`SPEC.md`) and one spec per story, including
  review notes. Read `SPEC.md` for what the app should and shouldn't do.

## Rules that keep the app working

- **The popup must never take keyboard focus.** It must not interrupt typing. CI tests this on Windows; test it by
  hand on macOS (type in another app while the popup appears).
- **Tray menu handlers run on the main thread.** Anything that takes the state lock must move off it first
  (`tauri::async_runtime::spawn_blocking`), or the app deadlocks.
- **Times are wall-clock instants, not countdowns,** so reminders stay right across sleep and wake.
- **No network access, accounts or telemetry.** The app is local-only by design.
- **Log with the `log` macros** (`log::warn!`, `log::info!`), not `println!`/`eprintln!`. Windows release builds
  have no console, so only the log file keeps the output.
- **Don't add JS permissions** to `src-tauri/capabilities/default.json`; add a Rust command instead. A test checks
  this.
- **Keep the `state.json` format backward compatible,** or existing users lose their settings.
- **Update the README** when users would notice the change.

## Releases

Maintainer only: see "Make a release" in the README.
