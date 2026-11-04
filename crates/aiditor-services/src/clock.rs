//! Local wall time for the frontend's minute clock.

use std::time::Duration;

/// The local time since midnight.
pub fn read() -> Duration {
    since_midnight(&jiff::Zoned::now())
}

fn since_midnight(now: &jiff::Zoned) -> Duration {
    let secs = now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
    Duration::new(secs, now.subsec_nanosecond() as u32)
}

/// `HH:MM` for a time since midnight.
pub fn minute_text(time: Duration) -> String {
    let s = time.as_secs();
    format!("{:02}:{:02}", s / 3600 % 24, s / 60 % 60)
}

/// How long from `time` until the next minute starts.
pub fn until_next_minute(time: Duration) -> Duration {
    Duration::from_secs(60) - Duration::new(time.as_secs() % 60, time.subsec_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_aligns_to_minutes_and_rolls_over_at_midnight() {
        for (time, text, millis) in [
            ("2026-10-31T20:41:59.750+00:00[UTC]", "20:41", 250),
            ("2026-10-31T20:42:00+00:00[UTC]", "20:42", 60_000),
            ("2026-10-31T23:59:59+02:00[Europe/Tallinn]", "23:59", 1_000),
            ("2026-11-01T00:00:00+02:00[Europe/Tallinn]", "00:00", 60_000),
            ("2026-11-01T03:07:23.125+00:00[UTC]", "03:07", 36_875),
        ] {
            let time = since_midnight(&time.parse().unwrap());
            let next = until_next_minute(time);
            assert_eq!(
                (minute_text(time), next),
                (text.to_string(), Duration::from_millis(millis))
            );
            assert_eq!((time + next).as_secs() % 60, 0);
        }
    }
}
