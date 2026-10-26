# Flaky tests

Tests known to fail now and then for reasons unrelated to the code under change. A failure of a test listed here can be ignored for the rest of a task; any other suspected flake is confirmed and added here, or fixed (see the `implement` skill). Remove an entry once its test is fixed.

A flake is confirmed when the test passes alone and on reruns, fails without a change to the code it covers, and the cause is understood well enough to name.

| Test | Where it fails | Why |
|---|---|---|
| `integrated_finders_visits_saves_and_directory_recovery`, `crates/aiditor/src/editor.rs:628` | macOS, under `cargo test --workspace` while other crates' tests run in parallel; panics at `editor.rs:669` with `native watcher ready`. Passes alone (about 1.5 s). | It waits for a real FSEvents notification. Under load, delivery takes longer than the `wait` helper's budget of 1000 iterations of a 5 ms sleep. |
| `kill_ends_the_shell_and_its_foreground_job`, `crates/aiditor-services/tests/terminal.rs:653` | macOS, under `cargo test --workspace`; `started` at `terminal.rs:72` gets `TerminalSpawned` with `failed to openpty: … Unknown error: -6`. Seen once in four loaded runs. | `openpty` fails transiently while many PTY tests open and close PTYs at once. The exact errno is unexplained; PTY exhaustion is likely, since `kern.tty.ptmx_max` is 511 and the user's own terminals share it. |
