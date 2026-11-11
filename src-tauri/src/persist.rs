//! Saves and loads `State` as a local JSON file. Saves are atomic (temp file,
//! then rename), so a crash mid-save leaves the previous file intact.

use crate::scheduler::State;
use chrono::{DateTime, TimeZone};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub fn decode(json: &[u8]) -> Result<State, serde_json::Error> {
    serde_json::from_slice(json)
}

pub fn encode(state: &State) -> String {
    serde_json::to_string_pretty(state).expect("State always serializes")
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// Loads saved state. A missing file means first launch; an unreadable or
/// corrupt one is copied to `<file>.bak` and replaced by defaults. Never fails.
pub fn load<Tz: TimeZone>(path: &Path, now: DateTime<Tz>) -> State {
    // Read bytes, so invalid UTF-8 counts as corrupt (and gets backed up).
    match fs::read(path) {
        Ok(json) => match decode(&json) {
            Ok(state) => state,
            Err(e) => {
                let bak = with_suffix(path, ".bak");
                let copied = fs::copy(path, &bak);
                eprintln!(
                    "[hydration] warning: state file {} is corrupt ({e}); backed up to {} ({}), starting from defaults",
                    path.display(),
                    bak.display(),
                    if copied.is_ok() {
                        "ok"
                    } else {
                        "backup failed"
                    },
                );
                State::new(now)
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => State::new(now),
        Err(e) => {
            eprintln!(
                "[hydration] warning: can't read {} ({e}); starting from defaults",
                path.display()
            );
            State::new(now)
        }
    }
}

pub fn save(path: &Path, state: &State) -> io::Result<()> {
    let tmp = with_suffix(path, ".tmp");
    let mut f = fs::File::create(&tmp)?;
    f.write_all(encode(state).as_bytes())?;
    // Flush to disk before the rename, so a power cut can't leave an empty file.
    f.sync_all()?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("hydration-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn round_trip() {
        let s = State::new(Utc::now());
        assert_eq!(decode(encode(&s).as_bytes()).unwrap(), s);
    }

    #[test]
    fn decode_rejects_bad_json() {
        assert!(decode(b"{not json").is_err());
        assert!(decode(b"{}").is_err());
    }

    #[test]
    fn missing_file_gives_first_launch_state() {
        let dir = temp_dir("missing");
        let now = Utc::now();
        assert_eq!(load(&dir.join("state.json"), now), State::new(now));
    }

    #[test]
    fn save_then_load_is_unchanged() {
        let dir = temp_dir("save");
        let path = dir.join("state.json");
        let now = Utc::now();
        let s = State::new(now);
        save(&path, &s).unwrap();
        assert!(!dir.join("state.json.tmp").exists());
        assert_eq!(load(&path, now + chrono::Duration::minutes(5)), s);
    }

    #[test]
    fn missing_optional_fields_still_load() {
        // A file from an older version without settings/drinks/history.
        let s = State::new(Utc::now());
        let mut v = serde_json::to_value(&s).unwrap();
        for k in ["settings", "drinks", "history", "popup_shown_at"] {
            v.as_object_mut().unwrap().remove(k);
        }
        assert_eq!(decode(v.to_string().as_bytes()).unwrap(), s);
    }

    #[test]
    fn invalid_utf8_is_backed_up() {
        let dir = temp_dir("utf8");
        let path = dir.join("state.json");
        fs::write(&path, [0xFF, 0xFE, 0x00]).unwrap();
        let now = Utc::now();
        assert_eq!(load(&path, now), State::new(now));
        assert_eq!(
            fs::read(dir.join("state.json.bak")).unwrap(),
            [0xFF, 0xFE, 0x00]
        );
    }

    #[test]
    fn unreadable_path_gives_defaults() {
        let dir = temp_dir("unreadable");
        let path = dir.join("state.json");
        fs::create_dir(&path).unwrap(); // reading a directory fails
        let now = Utc::now();
        assert_eq!(load(&path, now), State::new(now));
    }

    #[test]
    fn corrupt_file_is_backed_up_and_defaults_used() {
        let dir = temp_dir("corrupt");
        let path = dir.join("state.json");
        fs::write(&path, "{\"phase\": tru").unwrap();
        let now = Utc::now();
        assert_eq!(load(&path, now), State::new(now));
        assert_eq!(
            fs::read_to_string(dir.join("state.json.bak")).unwrap(),
            "{\"phase\": tru"
        );
    }
}
