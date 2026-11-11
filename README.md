# Hydration Reminder

A small macOS + Windows tray app (Tauri v2) that reminds you to drink water at a set interval.
The spec lives in `_bmad-output/specs/spec-hydration-reminder/`.

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

## Windows check from macOS

`cargo check --target x86_64-pc-windows-msvc` needs `llvm-rc` (Tauri's resource step). Real Windows builds run in CI (story 4).
