# Tasks

## 1. Baseline

- [ ] 1.1 Save `cargo test -p aiditor-core -- --list` from the starting commit to a scratch file and note the count (143 harness tests plus the other modules'); verify the file exists for the comparisons in 3.2 and 7.2.

## 2. Move code into modules (commit 1, pure move)

- [ ] 2.1 Create the child modules of design D1 (`builtins`, `editing`, `motion`, `kill_ring`, `mark`, `prefix`, `minibuffer`, `frame`, `visiting`, `autosave`, `bindings`) and move each function, struct and const into its module verbatim, only adding `use` lines and `pub(crate)` on items used outside their new module; the command table moves as-is into `builtins()` returning today's map (reshaped in 5.x). Verify `git diff --color-moved=dimmed-zebra` shows only moved lines, `use` lines and visibility changes.
- [ ] 2.2 Verify `lib.rs` holds only the D1 `lib.rs` row and is a few hundred lines (`wc -l`), and `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## 3. Move tests next to their modules (commit 2)

- [ ] 3.1 Move each block of `harness.rs`'s `mod tests` into a `#[cfg(test)] mod tests` in the module design D5 assigns, using `crate::harness::Harness`; move helpers shared across modules into `harness.rs` as `pub(crate)` items, single-module helpers with their tests; no test renamed or edited beyond `use` lines. Verify `harness.rs` has no `mod tests` left.
- [ ] 3.2 Verify `cargo test -p aiditor-core -- --list` gives the same count and, ignoring module paths, the same sorted names as the 1.1 baseline; fmt, clippy and tests pass.

## 4. State structs (commit 3)

- [ ] 4.1 Extract `KillRing { ring, yank_index, clipboard, clipboard_seen }` into `kill_ring.rs` as `core.kills`, with `KILL_RING_MAX`, and make `kill_new`, `current_kill` and `take_clipboard` its methods (design D2); update the `Msg::Key` clipboard reset and the kill-ring test helpers. Verify the `kill_ring` tests pass unchanged.
- [ ] 4.2 Extract `Prompts { minibuffer, history, matcher, repoll }` into `minibuffer.rs` as `core.prompts`, with `matches` and `selected_candidate` as its methods; update `Harness::settle` and every caller. Verify the `minibuffer`, `visiting` and `autosave` tests pass unchanged.
- [ ] 4.3 Drop `let core = &mut *self.core;` reborrows the extraction made needless, without adding clones; verify `grep -c 'let core = &mut \*self.core;' crates/aiditor-core/src/*.rs` sums below 27, and fmt, clippy and tests pass.

## 5. Command table (commit 4)

- [ ] 5.1 Rewrite `builtins()` as one `Vec<Command>` of `Command { name, spec, handler }` literals with each spec inline (design D3), and build `Core::commands` from it with `assert!(commands.insert(..).is_none(), "duplicate builtin …")`. Verify `grep -n 'expect("a builtin")\|; 57\]' crates/aiditor-core/src/` finds nothing and `builtins_are_callable_by_name`, `m_x_shows_each_commands_key` and the kill/yank clipboard tests pass.
- [ ] 5.2 Add the `shortcut:` comment for `where_is` at the `M-x` annotation (design D6). Verify by reading the diff.

## 6. `dispatch` arms (commit 5)

- [ ] 6.1 Move the `Msg::Saved`, `Msg::Listed` and `Msg::AutoSave` arms into `Ctx::on_saved` (`visiting.rs`), `Ctx::on_listed` (`minibuffer.rs`) and `Ctx::write_autosaves` (`autosave.rs`), keeping both auto-save timers cancelled first; make `recover_buffers` call `write_autosaves()` instead of `dispatch(Msg::AutoSave)`. Verify the save, quit, find-file and recovery tests pass.

## 7. Integration checks

- [ ] 7.1 Verify `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass on Linux, and on macOS through CI on the PR.
- [ ] 7.2 Verify `cargo test -p aiditor-core -- --list` matches the 1.1 baseline count and names, and `cargo tree -p aiditor-core` shows no gpui or tokio.
