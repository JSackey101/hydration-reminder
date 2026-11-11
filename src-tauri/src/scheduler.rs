//! Reminder scheduling as a pure function of (now, state). No I/O, no timers:
//! the caller injects the clock, so tests can fake sleep, midnight and DST.

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub interval_min: u32,
    pub snooze_min: u32,
    /// `None` = the popup stays until answered (CAP-14 default).
    pub popup_timeout_min: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            interval_min: 45,
            snooze_min: 10,
            popup_timeout_min: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Waiting,
    Prompting,
    Snoozed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drink {
    pub at: DateTime<Utc>,
    /// The due time before this drink, so undo (CAP-13) can restore it.
    pub prev_next_due_at: DateTime<Utc>,
}

/// Saved as `state.json`. Fields added in later stories must carry
/// `#[serde(default)]` so older files still load instead of being reset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub settings: Settings,
    pub phase: Phase,
    pub next_due_at: DateTime<Utc>,
    #[serde(default)]
    pub popup_shown_at: Option<DateTime<Utc>>,
    /// The local date that `drinks` belongs to.
    pub day: NaiveDate,
    #[serde(default)]
    pub drinks: Vec<Drink>,
    /// Archived daily counts, kept for the 7-day chart (CAP-11).
    #[serde(default)]
    pub history: BTreeMap<NaiveDate, u32>,
    /// Debug-only interval override in seconds. Lives in memory, never saved.
    #[serde(skip)]
    pub interval_override_secs: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    ShowPopup,
    ResetDay,
}

impl State {
    /// State for a first launch: count 0, first reminder one interval from now.
    pub fn new<Tz: TimeZone>(now: DateTime<Tz>) -> Self {
        let mut s = Self {
            settings: Settings::default(),
            phase: Phase::Waiting,
            next_due_at: now.with_timezone(&Utc),
            popup_shown_at: None,
            day: now.date_naive(),
            drinks: Vec::new(),
            history: BTreeMap::new(),
            interval_override_secs: None,
        };
        s.next_due_at += s.interval();
        s
    }

    pub fn count(&self) -> u32 {
        self.drinks.len() as u32
    }

    pub fn interval(&self) -> Duration {
        match self.interval_override_secs {
            Some(secs) => Duration::seconds(secs.max(1).into()),
            // A 0 from a hand-edited file would prompt nonstop.
            None => Duration::minutes(self.settings.interval_min.max(1).into()),
        }
    }

    pub fn snooze(&self) -> Duration {
        Duration::minutes(self.settings.snooze_min.max(1).into())
    }
}

/// Called once after loading saved state. A popup can't survive a restart, so a
/// stored Prompting goes back to Waiting and the next tick prompts again.
pub fn on_load(mut state: State) -> State {
    if state.phase == Phase::Prompting {
        state.phase = Phase::Waiting;
        state.popup_shown_at = None;
    }
    state
}

pub fn tick<Tz: TimeZone>(now: DateTime<Tz>, mut s: State) -> (State, Action) {
    let today = now.date_naive();
    let now = now.with_timezone(&Utc);

    // 1. Day rollover. Only forwards: a clock set back past midnight changes nothing.
    if today > s.day {
        s.history.insert(s.day, s.count());
        s.drinks.clear();
        s.day = today;
        // Fresh start: drop an overdue reminder left from yesterday, but keep a
        // due time still ahead and a popup already on screen.
        if s.phase != Phase::Prompting && s.next_due_at <= now {
            s.next_due_at = now + s.interval();
            s.phase = Phase::Waiting;
        }
        return (s, Action::ResetDay);
    }

    // 2. Clamp a due time pushed too far ahead (clock corrected backwards).
    let bound = match s.phase {
        Phase::Waiting => Some(s.interval()),
        Phase::Snoozed => Some(s.snooze()),
        Phase::Prompting => None,
    };
    if let Some(bound) = bound
        && s.next_due_at > now + bound
    {
        s.next_due_at = now + bound;
    }

    // 3. Due check. Fires once; missed reminders collapse into one prompt.
    if s.phase != Phase::Prompting && now >= s.next_due_at {
        s.phase = Phase::Prompting;
        s.popup_shown_at = Some(now);
        return (s, Action::ShowPopup);
    }

    (s, Action::None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::Europe::London;
    use chrono_tz::Tz;

    fn at(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Tz> {
        London
            .with_ymd_and_hms(y, mo, d, h, mi, 0)
            .single()
            .unwrap()
    }

    fn utc(t: DateTime<Tz>) -> DateTime<Utc> {
        t.with_timezone(&Utc)
    }

    fn with_drinks(mut s: State, n: usize) -> State {
        for _ in 0..n {
            s.drinks.push(Drink {
                at: s.next_due_at,
                prev_next_due_at: s.next_due_at,
            });
        }
        s
    }

    #[test]
    fn first_launch() {
        let now = at(2026, 6, 1, 9, 0);
        let s = State::new(now);
        assert_eq!(s.next_due_at, utc(now) + Duration::minutes(45));
        assert_eq!(s.day, now.date_naive());
        assert_eq!(s.count(), 0);
        assert_eq!(s.phase, Phase::Waiting);
        assert_eq!(s.settings, Settings::default());
        assert_eq!(s.settings.snooze_min, 10);
        assert_eq!(s.settings.popup_timeout_min, None);
    }

    #[test]
    fn not_yet_due() {
        let s = State::new(at(2026, 6, 1, 9, 0));
        let (s2, a) = tick(at(2026, 6, 1, 9, 30), s.clone());
        assert_eq!(a, Action::None);
        assert_eq!(s2, s);
    }

    #[test]
    fn due_prompts_once() {
        let s = State::new(at(2026, 6, 1, 9, 0));
        let (s, a) = tick(at(2026, 6, 1, 9, 45), s);
        assert_eq!(a, Action::ShowPopup);
        assert_eq!(s.phase, Phase::Prompting);
        assert_eq!(s.popup_shown_at, Some(utc(at(2026, 6, 1, 9, 45))));
    }

    #[test]
    fn already_prompting_does_not_repeat() {
        let s = State::new(at(2026, 6, 1, 9, 0));
        let (s, _) = tick(at(2026, 6, 1, 9, 45), s);
        let (s2, a) = tick(at(2026, 6, 1, 10, 30), s.clone());
        assert_eq!(a, Action::None);
        assert_eq!(s2, s);
    }

    #[test]
    fn slept_past_several_due_times_prompts_once() {
        let s = State::new(at(2026, 6, 1, 9, 0));
        let (s, a) = tick(at(2026, 6, 1, 12, 45), s); // due 9:45 + 3 h
        assert_eq!(a, Action::ShowPopup);
        let (_, a) = tick(at(2026, 6, 1, 12, 46), s);
        assert_eq!(a, Action::None);
    }

    #[test]
    fn snoozed_prompts_when_due() {
        let mut s = State::new(at(2026, 6, 1, 9, 0));
        s.phase = Phase::Snoozed;
        s.next_due_at = utc(at(2026, 6, 1, 9, 10));
        let (s, a) = tick(at(2026, 6, 1, 9, 10), s);
        assert_eq!(a, Action::ShowPopup);
        assert_eq!(s.phase, Phase::Prompting);
    }

    #[test]
    fn midnight_archives_and_resets_count() {
        // Same behaviour whether the app was running, asleep or quit: tick only
        // sees the stored day and the current time.
        let mut s = with_drinks(State::new(at(2026, 6, 1, 23, 50)), 5);
        let due = s.next_due_at;
        s.history
            .insert(NaiveDate::from_ymd_opt(2026, 5, 31).unwrap(), 7);
        let (s, a) = tick(at(2026, 6, 2, 0, 1), s);
        assert_eq!(a, Action::ResetDay);
        assert_eq!(s.count(), 0);
        assert_eq!(s.day, NaiveDate::from_ymd_opt(2026, 6, 2).unwrap());
        assert_eq!(s.history[&NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()], 5);
        assert_eq!(s.history[&NaiveDate::from_ymd_opt(2026, 5, 31).unwrap()], 7);
        assert_eq!(s.next_due_at, due); // still ahead (00:35), left alone
    }

    #[test]
    fn rollover_and_overdue_starts_fresh_without_popup() {
        for phase in [Phase::Waiting, Phase::Snoozed] {
            let mut s = with_drinks(State::new(at(2026, 6, 1, 18, 0)), 3);
            s.phase = phase;
            let morning = at(2026, 6, 2, 8, 30);
            let (s, a) = tick(morning, s);
            assert_eq!(a, Action::ResetDay);
            assert_eq!(s.phase, Phase::Waiting);
            assert_eq!(s.next_due_at, utc(morning) + Duration::minutes(45));
            // And the following tick doesn't prompt.
            let (_, a) = tick(at(2026, 6, 2, 8, 31), s);
            assert_eq!(a, Action::None);
        }
    }

    #[test]
    fn rollover_keeps_due_time_still_ahead() {
        let mut s = State::new(at(2026, 6, 1, 23, 0));
        s.next_due_at = utc(at(2026, 6, 2, 0, 15));
        let (s, a) = tick(at(2026, 6, 2, 0, 1), s);
        assert_eq!(a, Action::ResetDay);
        assert_eq!(s.next_due_at, utc(at(2026, 6, 2, 0, 15)));
        let (_, a) = tick(at(2026, 6, 2, 0, 15), s);
        assert_eq!(a, Action::ShowPopup);
    }

    #[test]
    fn rollover_while_prompting_stays_prompting() {
        let s = with_drinks(State::new(at(2026, 6, 1, 23, 0)), 2);
        let (s, _) = tick(at(2026, 6, 1, 23, 45), s);
        assert_eq!(s.phase, Phase::Prompting);
        let due = s.next_due_at;
        let (s, a) = tick(at(2026, 6, 2, 7, 0), s);
        assert_eq!(a, Action::ResetDay);
        assert_eq!(s.phase, Phase::Prompting);
        assert_eq!(s.next_due_at, due);
        assert_eq!(s.count(), 0);
    }

    #[test]
    fn dst_spring_forward_one_reset_at_midnight_and_real_minutes() {
        // London springs forward 01:00 -> 02:00 on 2026-03-29 (a 23 h day).
        let mut s = State::new(at(2026, 3, 28, 12, 0));
        let mut resets = Vec::new();
        let mut t = utc(at(2026, 3, 28, 12, 0));
        let end = utc(at(2026, 3, 30, 12, 0));
        while t <= end {
            let local = t.with_timezone(&London);
            let (s2, a) = tick(local, s);
            s = s2;
            if a == Action::ResetDay {
                resets.push(local);
            }
            if a == Action::ShowPopup {
                // Answer it so the schedule keeps running (stand-in for Drank).
                s.phase = Phase::Waiting;
                s.next_due_at = t + s.interval();
            }
            t += Duration::minutes(1);
        }
        assert_eq!(resets.len(), 2);
        for r in resets {
            assert_eq!((r.format("%H:%M").to_string()), "00:00");
        }

        // An interval spanning the jump is 45 real minutes: started at 00:30
        // local, tick prompts at 02:15 local (01:15 UTC), not before.
        let s = State::new(at(2026, 3, 29, 0, 30));
        let (s, a) = tick(at(2026, 3, 29, 0, 30), s);
        assert_eq!(a, Action::None);
        let (s, a) = tick(London.with_ymd_and_hms(2026, 3, 29, 2, 14, 59).unwrap(), s);
        assert_eq!(a, Action::None);
        let (_, a) = tick(at(2026, 3, 29, 2, 15), s);
        assert_eq!(a, Action::ShowPopup);
    }

    #[test]
    fn dst_fall_back_one_reset() {
        // London falls back 02:00 -> 01:00 on 2026-10-25 (a 25 h day).
        let mut s = State::new(at(2026, 10, 24, 12, 0));
        let mut resets = 0;
        let mut t = utc(at(2026, 10, 24, 12, 0));
        while t <= utc(at(2026, 10, 26, 12, 0)) {
            let (s2, a) = tick(t.with_timezone(&London), s);
            s = s2;
            if a == Action::ResetDay {
                resets += 1;
            }
            if a == Action::ShowPopup {
                s.phase = Phase::Waiting;
                s.next_due_at = t + s.interval();
            }
            t += Duration::minutes(1);
        }
        assert_eq!(resets, 2);
    }

    #[test]
    fn clock_set_back_past_midnight_does_not_reset() {
        let s = with_drinks(State::new(at(2026, 6, 2, 0, 5)), 2);
        let (s2, a) = tick(at(2026, 6, 1, 23, 55), s);
        assert_ne!(a, Action::ResetDay);
        assert_eq!(s2.count(), 2);
        assert!(s2.history.is_empty());
        assert_eq!(s2.day, NaiveDate::from_ymd_opt(2026, 6, 2).unwrap());
    }

    #[test]
    fn due_time_too_far_ahead_is_clamped() {
        // Waiting: clamp to now + interval.
        let mut s = State::new(at(2026, 6, 1, 9, 0));
        s.next_due_at = utc(at(2026, 6, 1, 14, 0));
        let now = at(2026, 6, 1, 10, 0);
        let (s, a) = tick(now, s);
        assert_eq!(a, Action::None);
        assert_eq!(s.next_due_at, utc(now) + Duration::minutes(45));

        // Snoozed: clamp to now + snooze.
        let mut s = State::new(at(2026, 6, 1, 9, 0));
        s.phase = Phase::Snoozed;
        s.next_due_at = utc(at(2026, 6, 1, 14, 0));
        let (s, _) = tick(now, s);
        assert_eq!(s.next_due_at, utc(now) + Duration::minutes(10));

        // Within bounds: untouched.
        let s = State::new(at(2026, 6, 1, 9, 0));
        let due = s.next_due_at;
        let (s, _) = tick(at(2026, 6, 1, 9, 1), s);
        assert_eq!(s.next_due_at, due);

        // Prompting: no clamp.
        let mut s = State::new(at(2026, 6, 1, 9, 0));
        s.phase = Phase::Prompting;
        s.next_due_at = utc(at(2026, 6, 1, 14, 0));
        let (s, _) = tick(now, s);
        assert_eq!(s.next_due_at, utc(at(2026, 6, 1, 14, 0)));
    }

    #[test]
    fn restart_while_prompting_prompts_again() {
        let s = State::new(at(2026, 6, 1, 9, 0));
        let (s, _) = tick(at(2026, 6, 1, 9, 45), s);
        assert_eq!(s.phase, Phase::Prompting);
        let s = on_load(s);
        assert_eq!(s.phase, Phase::Waiting);
        assert_eq!(s.popup_shown_at, None);
        let (_, a) = tick(at(2026, 6, 1, 9, 50), s);
        assert_eq!(a, Action::ShowPopup);
    }

    #[test]
    fn override_shortens_pending_due_time_through_tick() {
        // As in the app: state is created/loaded first, override set after.
        let t = at(2026, 6, 1, 9, 0);
        let mut s = State::new(t);
        s.interval_override_secs = Some(10);
        let (s, a) = tick(t + Duration::seconds(1), s);
        assert_eq!(a, Action::None);
        assert_eq!(s.next_due_at, utc(t) + Duration::seconds(11));
        let (_, a) = tick(t + Duration::seconds(11), s);
        assert_eq!(a, Action::ShowPopup);
    }

    #[test]
    fn zero_settings_are_floored_to_one_minute() {
        let mut s = State::new(at(2026, 6, 1, 9, 0));
        s.settings.interval_min = 0;
        s.settings.snooze_min = 0;
        assert_eq!(s.interval(), Duration::minutes(1));
        assert_eq!(s.snooze(), Duration::minutes(1));
        s.interval_override_secs = Some(0);
        assert_eq!(s.interval(), Duration::seconds(1));
    }

    #[test]
    fn interval_override_is_used_and_not_serialized() {
        let mut s = State::new(at(2026, 6, 1, 9, 0));
        s.interval_override_secs = Some(5);
        assert_eq!(s.interval(), Duration::seconds(5));
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("interval_override"));
        assert_eq!(s.settings.interval_min, 45);
    }
}
