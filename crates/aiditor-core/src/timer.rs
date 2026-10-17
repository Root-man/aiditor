//! Timers owned by the core. Time comes from the caller, so tests run on a fake clock.

use std::time::{Duration, Instant};

use crate::Msg;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerId(u64);

struct Timer {
    id: TimerId,
    deadline: Instant,
    repeat: Option<Duration>,
    msg: Msg,
}

/// Every live timer. Ids only increase, so cancelling a stale id never stops a newer timer.
#[derive(Default)]
pub struct Timers {
    next: u64,
    list: Vec<Timer>,
}

impl Timers {
    /// A timer that delivers `msg` after `after`, then every `repeat` if given.
    pub fn start(
        &mut self,
        now: Instant,
        after: Duration,
        repeat: Option<Duration>,
        msg: Msg,
    ) -> TimerId {
        debug_assert!(
            repeat != Some(Duration::ZERO),
            "a zero repeat would wake every frame"
        );
        let id = TimerId(self.next);
        self.next += 1;
        self.list.push(Timer {
            id,
            deadline: now + after,
            repeat,
            msg,
        });
        id
    }

    pub fn cancel(&mut self, id: TimerId) {
        self.list.retain(|t| t.id != id);
    }

    /// Ids due at `now`, earliest first. A snapshot, so timers started while firing wait.
    pub fn due(&self, now: Instant) -> Vec<TimerId> {
        let mut due: Vec<_> = self.list.iter().filter(|t| t.deadline <= now).collect();
        due.sort_by_key(|t| t.deadline);
        due.into_iter().map(|t| t.id).collect()
    }

    /// The message of timer `id` if it is still live; removes a one-shot, reschedules a repeat.
    pub fn fire(&mut self, id: TimerId, now: Instant) -> Option<Msg> {
        let i = self.list.iter().position(|t| t.id == id)?;
        match self.list[i].repeat {
            // From `now`, not the old deadline, so a late wake-up fires once instead of catching up.
            Some(interval) => {
                self.list[i].deadline = now + interval;
                Some(self.list[i].msg.clone())
            }
            None => Some(self.list.swap_remove(i).msg),
        }
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.list.iter().map(|t| t.deadline).min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::{Key, KeyCode};

    const MS: Duration = Duration::from_millis(1);

    fn msg(c: char) -> Msg {
        Msg::Key(Key::plain(KeyCode::Char(c)))
    }

    #[test]
    fn cancelled_while_due_does_not_fire() {
        let (mut timers, t0) = (Timers::default(), Instant::now());
        timers.start(t0, 10 * MS, None, msg('a'));
        let b = timers.start(t0, 10 * MS, None, msg('b'));
        let due = timers.due(t0 + 10 * MS);
        assert_eq!(due.len(), 2);
        assert_eq!(timers.fire(due[0], t0 + 10 * MS), Some(msg('a')));
        timers.cancel(b);
        assert_eq!(timers.fire(due[1], t0 + 10 * MS), None);
        timers.cancel(b);
    }

    #[test]
    fn started_while_firing_waits_for_next_wake_up() {
        let (mut timers, t0) = (Timers::default(), Instant::now());
        timers.start(t0, 10 * MS, None, msg('a'));
        let now = t0 + 10 * MS;
        let due = timers.due(now);
        let late = timers.start(now, Duration::ZERO, None, msg('b'));
        assert!(!due.contains(&late));
        for id in due {
            timers.fire(id, now);
        }
        assert_eq!(timers.due(now), vec![late]);
    }

    #[test]
    fn late_repeat_fires_once_and_reschedules_from_now() {
        let (mut timers, t0) = (Timers::default(), Instant::now());
        let id = timers.start(t0, 100 * MS, Some(100 * MS), msg('a'));
        let now = t0 + 350 * MS;
        assert_eq!(timers.due(now), vec![id]);
        assert_eq!(timers.fire(id, now), Some(msg('a')));
        assert!(timers.due(now).is_empty());
        assert_eq!(timers.next_deadline(), Some(now + 100 * MS));
    }

    #[test]
    fn next_deadline_is_the_earliest() {
        let (mut timers, t0) = (Timers::default(), Instant::now());
        assert_eq!(timers.next_deadline(), None);
        timers.start(t0, 300 * MS, None, msg('a'));
        let b = timers.start(t0, 100 * MS, None, msg('b'));
        assert_eq!(timers.next_deadline(), Some(t0 + 100 * MS));
        assert_eq!(timers.due(t0 + 300 * MS)[0], b);
        timers.fire(b, t0 + 300 * MS);
        assert_eq!(timers.next_deadline(), Some(t0 + 300 * MS));
    }
}
