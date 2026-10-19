# Tasks

Land after `split-core`. Locations are given by item name; line numbers (commit `8fa1ddd`, `crates/aiditor-core/src/lib.rs`) are only a hint, since `split-core` moves this code into the minibuffer module.

## 1. Choices and history in the minibuffer

- [ ] 1.1 Add a `choices: Option<&[&str]>` parameter to `Ctx::read_string` (lib.rs:297). When it is `Some`, use it as the candidate list in place of `command.spec.completion` (lib.rs:304-307); annotations stay from `spec.annotation`. Pass `None` from `Ctx::try_call` (lib.rs:273). Verify: the crate builds with every caller updated.
- [ ] 1.2 Add `history: bool` to `struct Minibuffer` (lib.rs:148-161), set to `choices.is_none()` in `read_string`, and have `Ctx::exit_minibuffer` (lib.rs:373-377) record the answer only when it is true. Verify: `each_prompt_keeps_a_history` and `history_keeps_a_hundred` in `harness.rs` pass unchanged.
- [ ] 1.3 Add a harness test with a test command whose handler asks `First? ` with `a`, `b`, then on that answer `Second? ` with `x`, `y`, `z` (through `read_string` with choices); assert `listed(...)` for each prompt, and that `RET` alone at a `no`, `yes` question returns `no`. Verify: the test passes (spec: "A command can ask with its own candidates").

## 2. Builtin questions pass their choices

- [ ] 2.1 `Ctx::kill_buffer`: pass `Some(&["no", "yes"])` to its `Buffer modified; kill anyway? ` question; make `kill-buffer`'s `ArgSpec.completion` in `Core::default` (lib.rs:1746-1753) just `|core, _| core.buffer_names(true)` and drop the "second prompt" comment. Verify: `killing_a_modified_file_buffer_asks` passes unchanged.
- [ ] 2.2 `Ctx::save_buffers_kill_terminal`: pass `Some(&["Yes", "No"])` for `Save file <path>? ` and `Some(&["No", "Yes"])` for `Modified buffers exist; exit anyway? `; remove its `ArgSpec` override in `Core::default` (lib.rs:1730-1740), the `len() % 2` completion. Verify: `quitting_offers_to_save_each_modified_file`, `declining_keeps_the_editor_open_unless_confirmed`, `a_failed_save_cancels_the_quit`, `a_close_request_quits_the_same_way` and `killing_or_quitting_removes_recovery_files` pass unchanged, and `closing_the_window_offers_to_save` in `crates/aiditor/src/main.rs` still sees `Save file /nonexistent/t.txt? `.
- [ ] 2.3 `Ctx::recover_buffers`: pass `Some(&["Yes", "No"])` for `Recover auto-saved <what>? `; remove its `ArgSpec` override in `Core::default` (lib.rs:1741-1745). Verify: the recovery tests in `harness.rs` (`recovery_offers_each_file_in_turn_and_c_g_keeps_the_rest` and the other `a_recovered_file_*` tests) and `crates/aiditor-services/tests/crash_recovery.rs` pass unchanged.
- [ ] 2.4 Add a harness test: with a modified file buffer, `C-x C-c no RET no RET C-x C-c M-p` shows `Beginning of history; no preceding item` (spec: "Confirmations keep no history"). Verify: it passes.
- [ ] 2.5 Update the `ArgSpec.completion` doc comment in `crates/aiditor-core/src/command.rs` to say it covers the declared prompts, a handler's own questions passing their choices. Verify: `rg "completion" crates/aiditor-core/src/command.rs` shows the new wording.

## 3. Docs and checks

- [ ] 3.1 In `docs/editor.md`, Minibuffer bullet (#39's Not yet list): drop "different candidates per prompt" and add that a command's own question passes its choices and its answers keep no history. Verify: `rg "different candidates per prompt" docs/editor.md` finds nothing.
- [ ] 3.2 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`; all pass (Linux here, macOS in CI).

## Workflow follow-up

- Archive the change after review; the `minibuffer` delta then updates `openspec/specs/minibuffer/spec.md`.
