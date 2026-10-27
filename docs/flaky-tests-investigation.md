# Flaky-test investigation — 2026-10-27

Investigated all 10 distinct tests in `docs/flaky-tests.md` at commit
`340d43ea79e47e7f924be4965235be24c7e0aa7a`. The list repeats one test and groups
several others. No production code, tests, dependency versions, or CI settings
were changed during the initial investigation below. The implementation
follow-up at the end records the subsequent fixes.

## Conclusion

Nine tests depend on native macOS filesystem notifications. Their startup
deadlines include expensive native watch installation and replacement, not just
event delivery. The tenth encounters a reproducible macOS PTY allocation race.
These are shared infrastructure problems, not ten separate broken assertions.

All ten have practical mitigation paths. The watcher service also has a definite
shutdown defect that can be fixed locally. What cannot be guaranteed by aiditor
is bounded OS notification latency under arbitrary load or the absence of an OS
PTY allocation race. A passing rerun or a larger timeout does not establish that
a flake is eliminated.

## Every listed test

| Test | Evidence and disposition |
| --- | --- |
| `integrated_finders_visits_saves_and_directory_recovery` | Reproduced `native watcher ready` failure in the normal-access editor suite. All 25 editor tests passed in each of three serial comparison runs. Serialize native tests and give native readiness its own elapsed-time budget; retain the finder, visit, save, and directory-recovery assertions. |
| `one_write_makes_one_report` | Passed in the original 70-test services run and five isolated repeats. It uses the same native startup path, with a roughly 10-second probe budget. Fix shared startup/lifecycle handling; keep native smoke coverage and test precise debounce/report-count behavior through the existing fake-backend/event-loop seam. This investigation did not reproduce this particular test's historical failure. |
| `overlapping_workspace_saves_deliver_reversed_replies_before_watch_recovery` | Reproduced `ready` failure. Address shared native readiness; preserve reversed-reply and recovery checks. |
| `external_writes_conflict_dirty_copies_while_clean_copies_revert` | Reproduced `ready` failure. Address shared native readiness; preserve dirty-conflict and clean-revert checks. This is the duplicated entry in the original list. |
| `held_reverts_keep_new_edits_and_apply_only_to_their_original_buffer` | Reproduced `ready` failure. Address shared native readiness; preserve held-reply and buffer-identity checks. |
| `an_own_save_updates_the_other_workspace_and_preserves_its_own_undo` | Reproduced `ready` failure. Address shared native readiness; preserve cross-workspace propagation and undo checks. |
| `recovery_crossing_the_limit_keeps_visited_file_synchronization` | Reproduced `ready` failure. Address shared native readiness; preserve the later independent-write check after recovery. |
| `accepted_recovery_covers_lost_directories_ignore_changes_and_replacements` | Reproduced `ready` failure. Address shared native readiness; preserve lost-directory, ignore-change, and replacement checks. |
| `atomic_external_saves_revert_every_clean_copy_and_leave_other_buffers_alone` | Reproduced `ready` failure. Address shared native readiness; preserve atomic replacement and multiple-buffer checks. |
| `kill_ends_the_shell_and_its_foreground_job` | The listed test passed in all 20 repeated terminal-suite runs, but the identical `openpty` error occurred three times in two other tests. Fix or mitigate shared PTY setup, not the shell-kill assertion. Serial terminal-suite runs passed 3/3; a standalone native reproducer also distinguished concurrent from serialized allocation. |

All seven listed file-sync tests passed in each of three isolated, serial runs
of the entire eight-test `file_sync` binary.

## Watcher findings

### Startup is part of the timeout

`Watcher::set` enqueues work asynchronously
(`crates/aiditor-services/src/watcher.rs:46`). The file-sync `ready` helper starts
its 10-second deadline immediately and rewrites the file while that work is
still being processed (`crates/aiditor-services/tests/file_sync.rs:111`).

Temporary instrumentation of the locked `notify` 8.2.0 dependency measured
successful `FSEventStreamStart` calls taking 3.6–7.8 seconds in a loaded run.
The tests initially registered non-recursive visited-file parents; subsequent
project indexing replaced some registrations with recursive root watches,
requiring another stream startup. Thus one readiness deadline can cover more
than one expensive native operation. This explains why attributing the entire
delay to notification delivery is too narrow.

The instrumented failing runs returned success from native startup. Although
8.2.0 discards startup errors, silent startup failure was not the cause observed
here. Temporarily removing the dependency's event purge also did not eliminate
the failures: file-sync still failed 6/8 and the editor readiness check failed.

### Dropping a watcher does not shut it down

`Watcher::spawn` clones the input sender into the backend factory and event
callback. That factory lives in the state owned by the worker consuming the
same channel. Dropping the public `Watcher` therefore cannot disconnect the
channel; there is no explicit shutdown message or `Drop` implementation.

A temporary probe dropped a watcher and observed that its report callback was
still retained after 500 ms, even without registering a path. The ownership
cycle in the source explains why it remains retained. With registrations, this
also retains the native backend and worker resources until process exit.

This is a real local lifecycle defect, but the experiments do **not** establish
that it caused every historical timeout. Fix it with explicit shutdown and a
test that observes callback/backend release. Preserve the service's existing
retry and registration-error behavior. Audit `Editor` ownership, `Watcher::spawn`
callers, native watcher unit helpers, and the file-sync fixture together.

### Practical treatment

1. Run native macOS tests serially and avoid overlapping watcher-suite processes.
   `cargo test --workspace -- --test-threads=1` is an immediate runner-level
   mitigation; Linux need not inherit it. A narrower native-test lock is an
   alternative, but it must cover setup through teardown and all relevant tests.
2. Fix watcher shutdown, then repeat loaded comparisons before claiming an effect
   on failure rates.
3. Separate registration readiness from behavioral assertions and give native
   startup a measured, bounded wall-clock budget with useful failure diagnostics.
   A larger budget remains mitigation, not a delivery guarantee. An
   acknowledgment proves registration completed, not that an event was delivered.
4. Use deterministic backend/event input for exact debounce and state-machine
   assertions; retain real native integration tests for filesystem behavior.

Serialization reduced failures but is not a universal cure: one serial
file-sync run failed while another native watcher suite was active. The final
three serial runs with no overlapping watcher suite all passed, taking
16.1–16.5 seconds per full binary.

## PTY findings

Twenty runs of the original terminal binary produced 18 passing suites and two
failing suites. Three `TerminalSpawned` replies contained the exact listed error,
`failed to openpty: ... code: -6 ... Unknown error: -6`. The failures occurred in
`backpressure_stalls_only_its_terminal_and_draining_resumes_it` twice and
`ctrl_c_interrupts_the_foreground_job` once. The failure is before the test's
terminal behavior is exercised.

A standalone C probe used eight threads, each opening and closing 2,000 PTY
pairs. It held at most eight pairs, spawned no children, and reproduced three
`errno -6` failures in 16,000 attempts. Serializing the same workload produced
zero failures. A second probe split allocation into its constituent operations
and reproduced three failures specifically in `posix_openpt`, which opens
`/dev/ptmx`. The configured `kern.tty.ptmx_max` was 511; exhaustion is not
established by these observations.

Apple's [errno definitions](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/errno.h)
name `-6` as `EREDRIVEOPEN`, an internal retry-open result. Its
[VFS implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_vnops.c)
associates that result with tty allocation and NFS open/create races. Together
with the `/dev/ptmx` reproducer, this supports a native PTY allocation race.
The precise path by which the running macOS kernel exposes that internal result
to userspace was not established.

Serialize PTY test lifetimes first; the entire original terminal suite passed
three serial runs. Do not retry the whole test, since that can hide behavioral
failures or leave child processes behind. A production retry should be narrowly
bounded and distinguish allocation failure from command failure. That needs
structured error propagation: `portable-pty` 0.9.0 currently formats the OS error
into an `anyhow` string. Matching arbitrary error text is not a good final fix.
Any retry/locking experiment must also cover concurrent closes, not only opens.

## Validation and boundaries

| Check | Observed result |
| --- | --- |
| Original parallel macOS workspace run with normal OS access | Editor binary: 24 passed, listed readiness test failed. Cargo stopped before subsequent binaries. |
| Original parallel file-sync binary | 6 passed, 2 readiness failures; further instrumented runs reproduced all seven listed file-sync failures. |
| Original editor binary, serial comparison | 3/3 runs passed, 25 tests each. |
| Original file-sync binary, final isolated serial comparison | 3/3 runs passed, 8 tests each. |
| Listed watcher report-count test | Passed in the original services suite and 5/5 isolated repeats. |
| Original terminal binary, parallel repetition | 18/20 suites passed; 3 matching allocation failures across the other 2 suites. |
| Original terminal binary, serial comparison | 3/3 suites passed, 26 tests each. |
| Native PTY reproduction | Concurrent: 3/16,000 failures; serialized: 0/16,000; staged concurrent probe: 3 failures opening the master. |
| Linux at the exact investigated commit | [Existing CI job](https://github.com/Root-man/aiditor/actions/runs/40469531200/job/122073004717) passed all ten listed tests, including all eight file-sync tests. This is one existing CI run, not a new Linux stress test. |

Initial restricted-access runs were excluded from native conclusions. A process
left by the restricted terminal run was identified and cleaned up before the
20-run comparison. All temporary repository probes and dependency changes were
removed; diagnostic source and logs were kept under `/tmp/aiditor-*` for this
session only.

The original services unit suite was **not fully green**: it finished 69 passed,
1 failed because `files::tests::a_failed_create_can_be_retried` reported that its
file-size-limited child did not succeed. That is outside the supplied list and
was not diagnosed or classified as a flake. An exploratory dependency
re-resolution selected notify 8.2.1; its test run is excluded from the original
baseline counts, and `Cargo.lock` was restored before the recorded 8.2.0 rerun.

No Linux draft PR was needed to inspect the exact-commit baseline. No new Linux
stress run, production fix, or claim of permanent flake elimination is included.

## Implementation follow-up

- Dropping `Watcher` explicitly tells its worker to stop. The reporter remains
  alive until the backend has been released; shutdown does not join the worker
  on the GUI thread. A regression covers both an idle watcher and one with a
  registered native watch.
- State-only watcher tests use the existing fake backend. Native integration
  scenarios and their assertions remain in place.
- Native watcher tests, editor tests that own watchers, and PTY integration
  tests serialize within each test binary on macOS. Linux keeps parallel test
  execution. The guards recover after a test panic so one failure does not
  poison the remaining tests.
- Native readiness probes continue for a bounded 30-second startup budget.
  The watcher unit helper accepts only a report containing its probe path;
  status messages cannot falsely signal readiness. Behavioral wait deadlines
  remain unchanged.
- The file-size-limit test captures its child's output through pipes. Previously
  redirecting the suite's output to a regular log file made the child's size
  limit apply to that log too. Child failures now include status and output.

The full macOS workspace run and Clippy with warnings denied passed after
these changes. Twenty repeated terminal-suite runs and three file-sync runs
passed with a 16-thread test harness. [Draft PR #313](https://github.com/Root-man/aiditor/pull/313)
checks the changes on Linux and macOS.
These controls address the reproduced failures; native delivery under arbitrary
external load and PTY allocation by unrelated processes remain OS constraints.
