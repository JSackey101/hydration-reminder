---
id: SPEC-hydration-reminder
companions:
  - stack.md
  - state-machines.md
sources:
  - ~/ideas/02-hydration-reminder.md
---

> **Canonical contract.** This SPEC and the files in `companions:` are the complete, preservation-validated contract for what to build, test, and validate. Source documents listed in frontmatter are for traceability — consult them only if you need narrative rationale or prose color this contract intentionally omits.

# Hydration Reminder

## Why

A pain to solve. The author forgets to drink water while focused at the computer, and ignores phone notifications. They need a reminder at a set interval on the screen they're already looking at, on both their Mac and their Windows machine. It's an alert, not a water measurer: there are no goals or amounts, because "how much is a sip?" has no good answer.

## Capabilities

### Phase 1 (MVP)

- **CAP-1**
  - **intent:** User sees how many times they've drunk today in the menu bar / system tray at all times.
  - **success:** After any Drank, tray log, undo or day reset, the tray shows the new count within 1 s, on both macOS and Windows.
- **CAP-2**
  - **intent:** User gets an on-screen popup reminder when a drink is due.
  - **success:** The popup appears on the display in use within 5 s of the due time, including when the due time passed while the machine was asleep (shown within 5 s of wake).
- **CAP-3**
  - **intent:** User answers the popup with Drank or Snooze.
  - **success:** Drank adds 1 to today's count and sets the next due time to now + interval. Snooze adds nothing and sets the next due time to now + 10 min. Either one closes the popup. The popup carries a short hint to take a few good mouthfuls, not a single sip.
- **CAP-4**
  - **intent:** User can log a drink from the tray menu without waiting for a reminder.
  - **success:** "Log a drink" adds 1 to the count and sets the next due time to now + interval.
- **CAP-5**
  - **intent:** User sets the reminder interval.
  - **success:** A changed interval applies from the next schedule and is still set after quitting and relaunching.
- **CAP-6**
  - **intent:** The count starts at zero each local day.
  - **success:** After local midnight, the count reads 0 whether the app was running, asleep or quit across midnight.
- **CAP-7**
  - **intent:** User can make the app start at login.
  - **success:** With the toggle on, the app is running with its tray icon after a fresh login on both OSes. With it off, it doesn't start.
- **CAP-13**
  - **intent:** User can undo an accidental Drank or tray log.
  - **success:** "Undo last drink" in the tray menu takes 1 off today's count and restores the next due time to what it was before that drink. It's unavailable when there's nothing to undo today.
- **CAP-14**
  - **intent:** User can have an unanswered popup dismiss itself.
  - **success:** With the timeout off (default), the popup stays until answered. With it set (e.g. 2 min), an unanswered popup closes at that time, counts nothing, and sets the next due time to now + interval.

### Phase 2

- **CAP-8**
  - **intent:** Reminders are held while the user is in a meeting or a full-screen app.
  - **success:** While a full-screen app or presentation is active, no popup shows; a held reminder appears within 5 s of leaving that state.
- **CAP-9**
  - **intent:** User limits reminders to working hours.
  - **success:** No popup appears outside the configured hours.
- **CAP-10**
  - **intent:** Reminders become more insistent after repeated snoozes.
  - **success:** After N consecutive snoozes (configurable), the next popup is visibly escalated.
- **CAP-11**
  - **intent:** User sees the last 7 days of drinks.
  - **success:** A chart shows a drink count for each of the past 7 days that matches the logged data.

## Constraints

- One codebase builds for macOS and Windows.
- The reminder is the app's own popup window, never an OS notification (those get ignored).
- The popup never takes keyboard focus or interrupts typing. This is riskiest on Windows.
- Scheduling is based on stored wall-clock due times, never in-memory countdowns, so sleep/wake can't make it drift or stall.
- Local-only: no network calls, no account.
- Small footprint, since it runs all day from login. This rules out heavy runtimes such as Electron.
- Windows tray icons can't show text, so on Windows the count is a number badge drawn on the tray icon (also in the tooltip).

## Non-goals

- Daily goals, or goals that adjust for weather or activity.
- Measuring intake (glasses, ml, oz): a drink is one press, whatever its size.
- Linux.
- Code signing and notarization; unsigned builds are fine for self-use.
- Accounts, cloud sync.
- Mobile apps.
- Apple Health / Google Fit integration.
- App-store distribution.

## Success signal

- On both the Mac and the Windows machine, the app starts at login and reminds on schedule (including after sleep) for a full work week with no restart, and the author presses Drank on most reminders.

## Assumptions

- Defaults: 45 min interval, 10 min snooze, popup timeout off.
- Drank (CAP-3) and tray log (CAP-4) both restart the interval.
- Logging from the tray while the popup is open counts as Drank and closes the popup.
- Reminders missed during sleep collapse into one popup.
- A new local day starts a fresh interval: the first reminder comes one interval after the first launch or wake of the day, with no leftover overdue popup.
- Undo covers today's drinks only; it can be pressed repeatedly to step back through them.
