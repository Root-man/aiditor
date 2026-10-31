//! Local wall time for the frontend's minute clock.

use std::time::Duration;

pub fn read() -> (String, Duration) {
    at(&jiff::Zoned::now())
}

fn at(now: &jiff::Zoned) -> (String, Duration) {
    let elapsed = Duration::new(now.second() as u64, now.subsec_nanosecond() as u32);
    (
        format!("{:02}:{:02}", now.hour(), now.minute()),
        Duration::from_secs(60) - elapsed,
    )
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
            let time = time.parse().unwrap();
            assert_eq!(at(&time), (text.to_string(), Duration::from_millis(millis)));
        }
    }
}
