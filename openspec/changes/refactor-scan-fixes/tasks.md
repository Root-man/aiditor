# Tasks

## 1. Terminal release on exit (A1)

- [ ] 1.1 In `Ctx::end_terminal` (`crates/aiditor-core/src/terminal.rs`), push `TerminalOp::Kill { id }` once the id is removed, and update its comment. Update the core tests for exit and spawn failure to expect exactly one `Kill` (the scenarios in `specs/terminal-buffers`). Verify with `cargo test -p aiditor-core terminal`.
- [ ] 1.2 Add a service test in `crates/aiditor-services/src/terminal.rs`: spawn `/bin/sh -c 'exit 0'` terminals, `Kill` each after its exit message, and assert the handle map is empty and the input thread ends. Count file descriptors via `/dev/fd` where available. Add a `Kill` after a failed spawn too (cwd not a directory). Verify with `cargo test -p aiditor-services terminal`.
- [ ] 1.3 In `docs/editor.md`'s terminal buffers note, record that an exited or failed terminal is released at once. Verify the note matches `specs/terminal-buffers`.

## 2. Recovery safety (A2, A6)

- [ ] 2.1 In `recover_buffers` (`crates/aiditor-core/src/autosave.rs`), replace the `delete`/`insert` pair with `buffer.replace(&[(0..len, r.text.clone())])`. Add a harness test that recovers into a file whose bytes are `[0, 1, 2]`: no panic, recovered text, modified, still read-only. Verify with `cargo test -p aiditor-core autosave`.
- [ ] 2.2 Add `replaces: Option<PathBuf>` to `RecoveryOp::AutoSave` (`crates/aiditor-core/src/lib.rs`). In `recover_buffers`, set it on the `AutoSave` effect queued for the recovered buffer, and push `DeleteRecovery` only when there is none. Update the existing recovery tests' expected effects. Verify with `cargo test -p aiditor-core autosave`.
- [ ] 2.3 In `crates/aiditor-services/src/recovery.rs`, delete `replaces` only after `fs::rename` succeeds, through one helper shared with `DeleteRecovery` that keeps the own-pid guard. Add a test where the temp path is a directory: the write fails and the old recovery file still exists. Add another where the write succeeds: the old file is gone. Verify with `cargo test -p aiditor-services recovery`.

## 3. Grep first batch (B1)

- [ ] 3.1 In `search_with_clock` (`crates/aiditor-services/src/grep.rs`), initialise `sent` from one `now()` call as `t.checked_sub(BATCH).unwrap_or(t)`. Change `one_ongoing_file_streams_ordered_batches_on_the_deadline` to expect `[(1, false), (2, false), (1, false), (0, true)]`. Check that the cancellation tests still hold. Verify with `cargo test -p aiditor-services grep`.
- [ ] 3.2 In `docs/performance.md`, note that `grep/first-batch-two-roots-warm-cache` now measures time to the first hit, not the 50 ms deadline. Verify the table row reads correctly.

## 4. Small hot-path fixes (A4, A7)

- [ ] 4.1 In `Terminal::copy_text` (`crates/aiditor-core/src/terminal.rs`), keep the current logical line's char count as a running total, resetting it on a hard newline. Verify that the existing copy-mode tests (colours, marks, cursor on wrapped lines with wide and zero-width chars) pass unchanged with `cargo test -p aiditor-core copy`.
- [ ] 4.2 In `Buffer::char_idx` (`crates/aiditor-core/src/buffer.rs`), read the last char by index with an empty-slice guard. Add a test covering an empty last line, a line with LF and a line without. Verify with `cargo test -p aiditor-core buffer`.

## 5. Properties near the window (B2)

- [ ] 5.1 Add `props_disjoint` to `Buffer`. `add_property` clears it on overlap and `set_copy_text` resets it. Add `properties_near(range)`, which binary-searches when the flag is set and filters `properties()` otherwise. Add tests for disjoint sorted props, an overlapping long prop covering the window start (fallback still finds it), and props after an edit. Verify with `cargo test -p aiditor-core buffer`.
- [ ] 5.2 In `display::build`, take all properties when any stored prop is `Invisible` (checked by variant, without positions), else `properties_near(first..last)`. Remove the now-stale `shortcut:` comment. Verify that the display and frame tests (folds, line decorations, copy-mode colours) pass, and that `terminal-copy/colored/10k-lines-display` drops in `cargo bench -p aiditor-core`.

## 6. Long lines and highlight cache (A3, B4)

- [ ] 6.1 In `display::build`, jump over point's line's hidden columns to the nearest of the visible start, the newline or the next fold start, mirroring the `col >= range.end` skip. Add a test with a fold inside the hidden prefix and hscroll past it, comparing against today's output. Verify with `cargo test -p aiditor-core display`.
- [ ] 6.2 Change `Syntax::highlights` and `Buffer::highlights` to take the list of char ranges. Have `build` request each displayed line's drawn columns only, clamped to the line, and paint only those ranges. Add a test where a long line's highlight past `shown` is not computed but the shown part keeps colours that depend on earlier context (a string opened before the shown columns). Verify with `cargo test -p aiditor-core syntax display`.
- [ ] 6.3 Add the one-entry `RefCell` cache to `Syntax`, keyed by ranges and palette and cleared in `edit`. Add tests: a second call returns equal runs without requerying, and an edit or a different palette recomputes. Verify with `cargo test -p aiditor-core syntax`.
- [ ] 6.4 Add a core bench scenario for a frame near the end of a 1 MB single line (with `rs` syntax) in `crates/aiditor-core/benches/scenarios`, and list it in `docs/performance.md`. Verify that `cargo bench -p aiditor-core` runs it and that it is far below the reported ~40 ms.

## 7. Benchmark fixtures (B5)

- [ ] 7.1 In `scenarios::source`, wrap every 100 `rs` items in a `mod` and every 100 `tsx` items in a component. Keep the per-line Unicode. Check that the syntax scenarios' validation (coloured frames) still passes with `cargo bench -p aiditor-core`.
- [ ] 7.2 Record in `docs/performance.md` that earlier `rs`/`tsx` `syntax-*` and `revert-apply-frame` results are not comparable. Verify that the fixture description matches the generator.

## 8. Integration

- [ ] 8.1 Run `cargo test --workspace`, `cargo clippy --workspace --all-targets` and `cargo fmt --check` on Linux, and `python3 -m pytest` for the bench-reporting scripts if any changed. Verify that all pass.
- [ ] 8.2 Run `cargo bench -p aiditor-core` and `cargo bench -p aiditor-services` once. Compare `terminal-copy/colored/*`, `syntax-edit-frame/*` and `grep/first-batch` against the numbers in `docs/perf-scan-2026-10-25.md`, and record the results in the PR description.

## Workflow follow-up

- Validate on macOS before treating the performance items as cross-platform wins.
- Add the B3 "move parsing off the core thread" task to the M5 syntax epic through the roadmap skill, as a separate change.
- Archive the change after review.
