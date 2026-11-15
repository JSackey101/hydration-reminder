# Hydration Reminder

A small macOS + Windows tray app (Tauri v2) that reminds you to drink water at a set interval.
The spec lives in `_bmad-output/specs/spec-hydration-reminder/`.

## Install

Installers come from the [Releases](../../releases) page: the `.dmg` for macOS (Intel or Apple Silicon), and the
`.msi` or `-setup.exe` for Windows. They are unsigned, so each OS warns the first time:

- **macOS:** open the `.dmg`, drag the app to Applications, and start it from there (not from the `.dmg`). The
  first time, macOS blocks it: on macOS 15 or later open System Settings → Privacy & Security and click
  **Open Anyway**; on older versions right-click the app → **Open** → **Open**. If macOS says the app is damaged,
  run `xattr -dr com.apple.quarantine "/Applications/Hydration Reminder.app"` and open it again.
- **Windows:** in the SmartScreen box, click **More info** → **Run anyway**.

The first launch turns on **Start at login**; switch it off in Settings… from the tray menu.

To uninstall, first untick **Start at login** (uninstalling doesn't remove the login entry), then delete the app
(macOS) or uninstall it from Settings → Apps (Windows). A leftover entry is
`~/Library/LaunchAgents/Hydration Reminder.plist` on macOS, or the `Hydration Reminder` value under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` on Windows. The State and Logs folders below can go too.

To make a release, set the same version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`
(the workflow checks all three), then push a matching tag (`git tag v0.1.0 &&
git push origin v0.1.0`). The Release workflow builds both installers into a draft release. "Run workflow" on the
Actions tab, or a PR that changes the packaging files, builds them as artifacts without releasing.

## Develop

```sh
npm install
npm run tauri dev                              # run the app
HYDRATION_INTERVAL_SECS=10 npm run tauri dev   # debug: remind every 10 s
cd src-tauri && cargo test                     # scheduler, persistence and badge tests
```

## State

Saved as `state.json` in the app data folder (macOS: `~/Library/Application Support/com.jeffsackey.hydration-reminder/`,
Windows: `%APPDATA%\com.jeffsackey.hydration-reminder\`). Quit the app and delete it to start fresh; a corrupt file
is copied to `state.json.bak` and replaced with defaults.

## Logs

Warnings and events go to stdout and to `Hydration Reminder.log` (about 1 MB, plus up to two older files):
macOS `~/Library/Logs/com.jeffsackey.hydration-reminder/`, Windows `%LOCALAPPDATA%\com.jeffsackey.hydration-reminder\logs\`.

## Windows check from macOS

`cargo check --target x86_64-pc-windows-msvc` needs `llvm-rc` (Tauri's resource step). Real Windows builds run in CI.
