# Spec Delta

## MODIFIED Requirements

### Requirement: Stderr and exit stream to the core
Each line the agent writes to stderr SHALL reach the core as one message tagged with the agent id, without its line ending, in order, with bytes that are not valid UTF-8 replaced by U+FFFD. Such bytes SHALL NOT stop later lines from being read. When the process exits, the core SHALL get one exit message with its exit code, or the signal that ended it, after every stderr line.

#### Scenario: Stderr then exit
- **WHEN** the test agent writes two lines to stderr and exits with code 3
- **THEN** the core gets both lines in order, then an exit message with code 3

#### Scenario: Stderr that is not UTF-8
- **WHEN** the test agent writes the bytes `caf\xE9\n`, then `next\n`, to stderr
- **THEN** the core gets `caf\u{FFFD}` and then `next`, in order

#### Scenario: A chatty agent after a bad byte
- **WHEN** the test agent writes a non-UTF-8 line, then more stderr than a pipe holds, then exits
- **THEN** the agent does not block and the core gets every line, then the exit message
