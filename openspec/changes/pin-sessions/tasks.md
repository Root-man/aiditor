## 1. Core

- [x] 1.1 `Pin { agent, cwd, session }` and `Core::pins`; `Msg::PinsLoaded` parses the file, logging a failure; `Effect::WritePins` with the serialized pins
- [x] 1.2 `SessionList::cwd`; `render_sessions` groups pinned sessions under Pinned, keeping their time
- [x] 1.3 `agent-sessions-pin` (`P`): toggles the pin on point's row, writes, rebuilds the project's lists; `Select a session row` elsewhere; hint `P pin`
- [x] 1.4 `agent_listed` drops the agent's pins in the list's `cwd` that the answer lacks, writing and rebuilding when any went

## 2. Services and app

- [x] 2.1 `WriteExpectation::Overwrite`; `Runner::pins` path and the latest-wins write on the blocking pool
- [x] 2.2 `main.rs` reads `sessions.toml` from `data_local_dir` at launch and passes the path to the runner; update the other `Runner` constructions

## 3. Tests

- [x] 3.1 Core: P pins and unpins, point stays, P on a heading, pinned row's time and face, loaded pins list under Pinned, bad file logged, gone pins dropped, other project's and errored agent's pins kept, hints
- [x] 3.2 Services: latest write wins and the file reads back

## 4. Docs

- [x] 4.1 `docs/editor.md`: Session manager (built in #484) and Persistence
