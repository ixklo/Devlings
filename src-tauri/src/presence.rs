//! A small wave when the user comes back to the computer after a while (design v1.2). Leaving never costs anything
//! (design v1.1 §2a); coming back gets a greeting. Windows reports the time since the last keyboard or mouse input;
//! elsewhere there's no reading, so no wave.

/// No input at all for this long counts as having left (a locked screen counts too).
pub const AWAY_MS: u64 = 10 * 60 * 1000;
/// Input this recent, after being away, counts as being back.
pub const BACK_MS: u64 = 2_000;

#[derive(Debug, Default)]
pub struct Presence {
    away: bool,
}

impl Presence {
    /// Feeds one reading of the time since the last input (about once a second). True once, on the first reading
    /// that shows the user back after being away.
    pub fn sample(&mut self, idle_ms: u64) -> bool {
        if idle_ms >= AWAY_MS {
            self.away = true;
            return false;
        }
        if self.away && idle_ms <= BACK_MS {
            self.away = false;
            return true;
        }
        false
    }
}

/// Milliseconds since the last keyboard or mouse input anywhere in the session.
#[cfg(windows)]
pub fn idle_ms() -> Option<u64> {
    use windows::Win32::System::SystemInformation::GetTickCount;
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: `info` is a live LASTINPUTINFO with `cbSize` set, as GetLastInputInfo requires.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return None;
    }
    // SAFETY: GetTickCount takes no arguments and has no preconditions.
    let now = unsafe { GetTickCount() };
    // Both are 32-bit millisecond ticks that wrap about every 49.7 days; the difference is still right.
    Some(u64::from(now.wrapping_sub(info.dwTime)))
}

/// No reading off Windows, so no wave there.
#[cfg(not(windows))]
pub fn idle_ms() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves_once_when_the_user_comes_back_after_being_away() {
        let mut p = Presence::default();
        assert!(!p.sample(500), "at work: nothing");
        assert!(!p.sample(AWAY_MS - 1), "not away long enough yet");
        assert!(!p.sample(AWAY_MS), "away now, still nothing");
        assert!(!p.sample(AWAY_MS + 60_000));
        assert!(p.sample(300), "back: one wave");
        assert!(!p.sample(200), "only once");
        assert!(!p.sample(AWAY_MS - 1));
        assert!(!p.sample(100), "a short break doesn't count");
    }

    #[test]
    fn a_reading_between_back_and_away_neither_waves_nor_resets() {
        let mut p = Presence::default();
        p.sample(AWAY_MS);
        // The first reading after the return can land a few seconds late (the tick is ~1 s, but a busy machine).
        assert!(!p.sample(BACK_MS + 1));
        assert!(p.sample(BACK_MS), "the next fresh input still greets");
    }

    #[test]
    fn the_reading_is_available_where_supported() {
        // Windows CI and desktops have input tick counts; other platforms say there's no reading.
        if cfg!(windows) {
            assert!(idle_ms().is_some());
        } else {
            assert_eq!(idle_ms(), None);
        }
    }
}
