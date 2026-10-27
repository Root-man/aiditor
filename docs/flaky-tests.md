# Flaky tests

Tests known to fail now and then for reasons unrelated to the code under change. A failure of a test listed here can be ignored for the rest of a task; any other suspected flake is confirmed and added here, or fixed (see the `implement` skill). Remove an entry once its test is fixed.

A flake is confirmed when the test passes alone and on reruns, fails without a change to the code it covers, and the cause is understood well enough to name.

No currently tracked flakes. The entries investigated on 2026-10-27 were addressed
by watcher shutdown, macOS native-test serialization, and bounded readiness
probes. See the [investigation and implementation follow-up](flaky-tests-investigation.md)
for evidence and the remaining OS limits.
