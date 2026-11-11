# Stack

Implementation decisions behind SPEC.md constraints.

| Area | Choice | Why |
|---|---|---|
| Shell | Tauri v2 | One codebase for Mac + Windows; far lighter than Electron for an all-day app |
| UI | TypeScript + Vite (popup, settings) | Tauri default web front end |
| State owner | Rust backend holds scheduler state; UI only renders and sends commands | One source of truth for count and due time |
| Persistence | Local JSON file via `serde_json`, written atomically (temp file + rename) | Settings, `next_due_at`, today's date, and today's drinks as a list of `{at, prev_next_due_at}` (count = length; undo pops the last one and restores its due time). Survives restarts |
| Tray | `TrayIconBuilder` | macOS: `set_title` ("💧 5"). Windows: tooltip + menu header, or count badge drawn on the icon |
| Autostart | `tauri-plugin-autostart` | CAP-7 on both OSes |
| CI / packaging | `tauri-action` GitHub workflow, macOS + Windows runners | Builds both installers from one repo |

## Scheduler

- Pure function `tick(now, state) -> (state, action)`, where `action` ∈ {none, show_popup, reset_day}. The clock is injected so tests can fake time (sleep, midnight, DST).
- State holds `next_due_at` (absolute time) and `day` (local date). Called on a short timer, at startup and on wake.
- `now >= next_due_at` → show the popup (once, even if several intervals were missed).
- `local_date(now) != day` → clear today's drinks and set `day`. Phase 2 CAP-11 needs past days kept, so archive the daily count before clearing.
- Popup timeout (CAP-14) is also a stored time (`popup_shown_at + timeout`), checked by the same tick.

## Popup placement

- Show on the display in use: the monitor holding the cursor (or the focused window), not always the primary.

## Popup window

- Window config: `alwaysOnTop: true`, `focus: false`, `decorations: false`, `skipTaskbar: true`.
- Verify on Windows that showing it doesn't steal focus from the foreground app; that may need native window flags.
- Phase 2 CAP-8: Windows `SHQueryUserNotificationState`; macOS has no equivalent API, so a heuristic is needed (e.g. a full-screen frontmost window).
