# Flaky tests

Tests known to fail now and then for reasons unrelated to the code under change. A failure of a test listed here can be ignored for the rest of a task; any other suspected flake is confirmed and added here, or fixed (see the `implement` skill). Remove an entry once its test is fixed.

A flake is confirmed when the test passes alone and on reruns, fails without a change to the code it covers, and the cause is understood well enough to name.

The entries investigated on 2026-10-27 were addressed by watcher shutdown,
macOS native-test serialization, and bounded readiness probes. Native
notification delays and PTY allocation by unrelated processes remain OS constraints.

| Test | Where it fails | Why |
| --- | --- | --- |
| `crates/aiditor/src/editor.rs:1170` — `integrated_finders_visits_saves_and_directory_recovery` | macOS full app/workspace suite, at `native watcher never ready` | FSEvents sometimes supplies no notifications within the 30 s readiness probe; the same test passes alone and on suite reruns without changes to watcher logic. Native test bodies are serialized, but watcher shutdown is asynchronous and GPUI fixtures can outlive the native-test lock, leaving stream startup/shutdown overlap possible. |
