# Tasks

## 1. Time

- [x] 1.1 Add `jiff` to `aiditor-core` and a zone-taking parse/format helper; verify with a unit test covering `Z`, `+02:00`, `-04:00`, fractions, missing and unreadable times under a fixed zone

## 2. Sessions buffer

- [x] 2.1 Add `sessions.rs` with the `SessionList` state, `agent-sessions` (requests per agent, held while capturing, released by `release_agents`) and rendering; verify with harness tests for one request per agent, waiting for capture, pending and error lines, sorting across agents, `[open]` marks and `No sessions`
- [x] 2.2 Handle `Msg::AgentListed`: drop stale request numbers, rebuild, keep point on the same session; verify with tests for a late answer after `g` and point staying on its session
- [x] 2.3 Commands and `agent-sessions-mode` keys (`n`/`p`/`C-n`/`C-p`/arrows, `RET`, `g`, `q`) in builtins and `keymap.toml`; verify with tests for moving, `RET` on open and not-open rows, and `q`

## 3. Docs

- [x] 3.1 Note what was built in `docs/editor.md`'s session manager bullet; verify by reading it
