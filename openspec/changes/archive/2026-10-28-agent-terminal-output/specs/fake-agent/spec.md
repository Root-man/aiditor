## MODIFIED Requirements

### Requirement: The script names what the fake does
The fake agent SHALL read the JSON script file named by `AIDITOR_FAKE_AGENT`, relative to its working directory. The script is an object with an optional `session` list of steps, run once after the fake answers `session/new`, and an optional `prompts` list of step lists, where the n-th `session/prompt` runs the n-th list. Without `AIDITOR_FAKE_AGENT` the script is empty. A script that cannot be read or parsed SHALL make the fake write the reason to stderr and exit with code 2 before speaking ACP. On every start the fake SHALL report on stderr its pid, the file system and terminal capabilities and the `_meta.terminal_output` capability the client advertised, the session's `cwd` and the variable `A`, and SHALL answer `session/new` with session id `fake-session` and one config option: a select with id `mode`, category `mode`, values `auto` and `plan`, current value `auto`.

#### Scenario: No script
- **WHEN** the fake is spawned without `AIDITOR_FAKE_AGENT` and opens a session
- **THEN** the session opens with id `fake-session` and a `mode` option with values `auto` and `plan`, current `auto`

#### Scenario: A bad script
- **WHEN** `AIDITOR_FAKE_AGENT` names a file that is not a valid script
- **THEN** the fake writes the reason to stderr and exits with code 2
