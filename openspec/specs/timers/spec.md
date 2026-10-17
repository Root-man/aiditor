# timers Specification

## Purpose
Timers let the core schedule work for later (auto-save, the which-key delay, debounced grep) and receive it as ordinary messages, with time supplied by the caller so tests run on a fake clock.

## Requirements

### Requirement: A one-shot timer fires its message once
A timer started with a delay and a message SHALL deliver that message to the core the first time the core handles any message at or after its deadline, and then SHALL be gone.

#### Scenario: Not before the deadline
- **WHEN** a one-shot timer is started with a 100 ms delay and the fake clock advances 99 ms
- **THEN** its message has not been delivered

#### Scenario: Fires once at the deadline
- **WHEN** the fake clock then advances 1 ms more, and later another 100 ms
- **THEN** its message has been delivered exactly once

### Requirement: A repeating timer fires every interval until cancelled
A repeating timer SHALL deliver its message at each interval. After it fires, its next deadline SHALL be one interval after the time it fired, so a late wake-up delivers the message once, not once per missed interval.

#### Scenario: Fires each interval
- **WHEN** a repeating timer with a 100 ms interval is started and the fake clock advances 100 ms three times
- **THEN** its message has been delivered three times

#### Scenario: A late wake-up does not catch up
- **WHEN** a repeating timer with a 100 ms interval is started and the fake clock advances 350 ms at once
- **THEN** its message has been delivered once

### Requirement: A cancelled timer never fires
Cancelling a timer SHALL stop it from delivering its message, including when it is already due in the same wake-up. Cancelling a timer that is gone SHALL do nothing.

#### Scenario: Cancel before the deadline
- **WHEN** a timer is started, cancelled, and the fake clock advances past its deadline
- **THEN** its message is not delivered

#### Scenario: Cancelled while due
- **WHEN** two timers are due in the same wake-up and the first one to fire cancels the second
- **THEN** the second timer's message is not delivered

### Requirement: Timers started while firing wait for the next wake-up
A timer started while due timers are firing SHALL NOT fire in that same wake-up, even if its delay is zero, so firing timers cannot loop forever.

#### Scenario: Zero delay from a firing timer
- **WHEN** a firing timer starts another timer with zero delay
- **THEN** the new timer fires on the next wake-up, not the current one

### Requirement: The core reports its next deadline
The core SHALL report the earliest deadline among its live timers, or none when it has no timers, so the app can wake it in time.

#### Scenario: Earliest deadline
- **WHEN** timers with 300 ms and 100 ms delays are started
- **THEN** the next deadline is 100 ms from the time they were started

#### Scenario: No timers
- **WHEN** the only timer has fired and is gone
- **THEN** there is no next deadline
