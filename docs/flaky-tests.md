# Flaky tests

Tests known to fail now and then for reasons unrelated to the code under change. A failure of a test listed here can be ignored for the rest of a task; any other suspected flake is confirmed and added here, or fixed (see the `implement` skill). Remove an entry once its test is fixed.

A flake is confirmed when the test passes alone and on reruns, fails without a change to the code it covers, and the cause is understood well enough to name.

The entries investigated on 2026-10-27 were addressed by watcher shutdown,
macOS native-test serialization, and bounded readiness probes. Native
notification delays and PTY allocation by unrelated processes remain OS constraints.

Frontend fixtures now retain the native-test lock through GPUI shutdown and watcher reporter closure, and keep watched temporary directories alive until that shutdown completes. This prevents watcher startup in one fixture from overlapping unfinished teardown in another.

| Test | Where it fails | Why |
| --- | --- | --- |
| `an_agent_reply_and_write_reach_the_editor_and_its_workspace_kill_stops_it` | `crates/aiditor-services/tests/scenario.rs:172`, `watcher never ready` | The first native file notification missed the readiness deadline while another `cargo test --workspace` ran alongside (seen on macOS, 2026-11-02); it passed alone 3 of 3 and in the two later full runs. |
| `a_hanging_shell_is_killed_with_its_group` | `crates/aiditor-services/src/environment.rs:245`, reading the pid file | All three give a script a 1 s deadline, which a loaded machine can miss before the script runs. They failed together in one `cargo test --workspace` straight after a rebuild (macOS, 2026-11-02), with this crate's code unchanged, and passed alone 3 of 3 and in the next full run. The panic messages weren't kept, so the deadline is the likely cause, not a proven one. |
| `direnv_sets_and_unsets_with_the_captured_environment` | `crates/aiditor-services/src/environment.rs:286`, `result.unwrap()` | As above. |
| `a_failing_direnv_keeps_the_environment` | `crates/aiditor-services/src/environment.rs:308`, `.envrc is blocked` | As above. |
