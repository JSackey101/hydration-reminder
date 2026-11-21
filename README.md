# Hydration Reminder

A small app for macOS and Windows that reminds you to drink water while you're at the computer. It sits in the menu
bar (macOS) or the system tray (Windows), shows how many drinks you've had today, and every 45 minutes pops up a
small reminder in the corner of the screen. The reminder never takes over your keyboard, so it won't interrupt
your typing.

## Install

Download the latest version from the **[Releases page](https://github.com/JSackey101/hydration-reminder/releases/latest)**:

| Your computer | Download |
|---|---|
| Mac (Intel or Apple Silicon) | the file ending in `_universal.dmg` |
| Windows 10 or 11 PC (Intel or AMD; Windows 11 on ARM also works, through emulation) | the file ending in `_x64-setup.exe` (or `.msi` if you prefer, which needs admin rights) |

The app isn't signed with a paid developer certificate, so your computer will warn you the first time you open it.
That's expected. Here's how to get past it.

### macOS

1. Open the downloaded `.dmg`, then drag **Hydration Reminder** into the **Applications** folder.
2. Open **Applications** and double-click **Hydration Reminder**. Always start it from Applications, not from the
   `.dmg` window.
3. macOS says it can't check the app for malware. Click **Done** (or **Cancel**), then:
   - **macOS 15 (Sequoia) or later:** open **System Settings → Privacy & Security**, scroll down, click
     **Open Anyway** next to the Hydration Reminder message, and confirm.
   - **Older macOS:** right-click (or Control-click) the app in Applications, choose **Open**, then **Open** again.
4. A water drop with today's count (💧 0) appears in the menu bar at the top of the screen. That's the app.

If macOS instead says the app "is damaged and can't be opened", open **Terminal** and run this, then open the app again:

```sh
xattr -dr com.apple.quarantine "/Applications/Hydration Reminder.app"
```

macOS may also show an **"App Background Activity"** notice. That's macOS telling you the app will start at login.

### Windows

1. Double-click the downloaded `_x64-setup.exe`.
2. If a blue **"Windows protected your PC"** box appears, click **More info**, then **Run anyway**.
3. Follow the installer. When it finishes, the app starts.
4. A water drop with today's count appears in the system tray, bottom-right near the clock. If you can't see it,
   click the **^** arrow next to the tray icons, then drag the drop onto the taskbar so it always shows.

## Using it

- **The reminder:** when a drink is due, a small window appears in the corner of the screen. Press **Drank** after
  a few good mouthfuls, or **Snooze 10 min** if now isn't a good time. The next reminder comes one interval after
  you press Drank.
- **Small drop:** prefer something quieter? Set **Reminder style** to **Small drop** in Settings. A due reminder then
  shows just a small 💧 bubble in the same corner, which pulses once and never takes your typing. Click it to open the
  usual reminder, or answer it straight away with the keyboard shortcuts. The auto-close timeout counts from when the
  drop appeared.
- **Keyboard shortcuts:** while a reminder is showing, press **⌃⌥D** (Ctrl+Alt+D on Windows) for Drank or **⌃⌥S**
  (Ctrl+Alt+S) for Snooze, from any app. You keep typing where you were. The popup shows the keys under its
  buttons. The shortcuts only work while a reminder is showing; the rest of the time the keys go to your other apps
  as usual.
- **The count:** the number on the drop is how many drinks you've had today. It resets at midnight. On Windows the
  drop turns red when a drink is due.
- **The menu:** click the drop for:
  - **Log a drink**, if you drank without being reminded. This also restarts the timer.
  - **Undo last drink**, if you pressed Drank by mistake.
  - **Settings…**
  - **Quit**
- **Settings:**
  - **Remind me every:** 1–240 minutes. The default is 45.
  - **Close an unanswered popup after:** optional. With it on, a reminder you ignore closes itself after that many
    minutes.
  - **Reminder style:** **Popup** (the default) or **Small drop**. A change applies at once, even to a reminder on
    screen.
  - **Start at login:** on by default, so the app is there every time you sign in.
  - **Shortcuts:** press **Record** next to Drank or Snooze, then the new key combination (Esc cancels). It needs Ctrl
    or ⌘ on macOS (Ctrl or the Windows key on Windows), and Drank and Snooze must differ. You can't record while a reminder is showing;
    answer it first.

Shortcut notes:

- **Windows, some keyboard layouts:** Ctrl+Alt works like AltGr, so on layouts where AltGr+D or AltGr+S types a
  character, the default shortcut takes that key away while a reminder shows. Record a different one if that gets in
  your way.
- **Another app using the same combination:** on Windows, Settings tells you it can't be registered; pick another.
  On macOS there's no error: the other app gets the key press instead. If a shortcut does nothing, record a different
  one.

Your count and settings are kept on your computer only. Nothing is sent anywhere, and there's no account.

## Uninstall

1. Open **Settings…** from the drop's menu and untick **Start at login**. Uninstalling doesn't remove this entry
   on its own.
2. Choose **Quit** from the menu.
3. Remove the app:
   - **macOS:** drag **Hydration Reminder** from Applications to the Bin.
   - **Windows:** **Settings → Apps → Installed apps → Hydration Reminder → Uninstall**.

If you forgot step 1, remove the leftover login entry by hand:

- **macOS:** delete `~/Library/LaunchAgents/Hydration Reminder.plist`.
- **Windows:** delete the `Hydration Reminder` value under
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` in Registry Editor.

To remove your saved count and settings too, delete the State and Logs folders listed under
[Troubleshooting](#troubleshooting).

## Troubleshooting

- **No reminders?** Check the drop is in the menu bar or tray. If it isn't, start the app again from Applications
  (macOS) or the Start menu (Windows).
- **Logs:** the app writes what it's doing to `Hydration Reminder.log` (up to about 1 MB, plus up to two older
  files):
  - **macOS:** `~/Library/Logs/com.jeffsackey.hydration-reminder/`
  - **Windows:** `%LOCALAPPDATA%\com.jeffsackey.hydration-reminder\logs\`
- **State:** your count and settings are in `state.json`:
  - **macOS:** `~/Library/Application Support/com.jeffsackey.hydration-reminder/`
  - **Windows:** `%APPDATA%\com.jeffsackey.hydration-reminder\`

  To start fresh, quit the app and delete that file. A corrupt file is saved as `state.json.bak` and replaced with
  defaults.

## Contributing

Bug reports, ideas and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for how to set up, the
checks to run and how review works. Report security problems privately: see [SECURITY.md](SECURITY.md).

Hydration Reminder is released under the [MIT license](LICENSE).

## Develop

The spec lives in `_bmad-output/specs/spec-hydration-reminder/`.


```sh
npm install
npm run tauri dev                              # run the app
HYDRATION_INTERVAL_SECS=10 npm run tauri dev   # debug: remind every 10 s
HYDRATION_REMINDER_STYLE=drop npm run tauri dev # debug: use the small drop (not saved)
cd src-tauri && cargo test                     # scheduler, persistence and badge tests
```

## Windows check from macOS

`cargo check --target x86_64-pc-windows-msvc` needs `llvm-rc` (Tauri's resource step). Real Windows builds run in CI.

## Make a release

1. Set the same version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`. The workflow
   checks all three.
2. Push a matching tag, for example `git tag v0.1.0 && git push origin v0.1.0`.
3. The Release workflow builds both installers into a **draft** release. Check it, then publish it.

"Run workflow" on the Actions tab, or a PR that changes the packaging files, builds the installers as workflow
artifacts without releasing.
