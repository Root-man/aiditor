# Flaky tests

Tests known to fail now and then for reasons unrelated to the code under change. A failure of a test listed here can be ignored for the rest of a task; any other suspected flake is confirmed and added here, or fixed (see the `implement` skill). Remove an entry once its test is fixed.

A flake is confirmed when the test passes alone and on reruns, fails without a change to the code it covers, and the cause is understood well enough to name.

The entries investigated on 2026-10-27 were addressed by watcher shutdown,
macOS native-test serialization, and bounded readiness probes. Native
notification delays and PTY allocation by unrelated processes remain OS constraints.

Frontend fixtures now retain the native-test lock through GPUI shutdown and watcher reporter closure, and keep watched temporary directories alive until that shutdown completes. This prevents watcher startup in one fixture from overlapping unfinished teardown in another.

No currently listed flaky tests.
