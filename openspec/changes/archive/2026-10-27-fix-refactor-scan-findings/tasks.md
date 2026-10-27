## 1. Benchmarks run again (#6)

- [x] 1.1 In `crates/aiditor-core/benches/scenarios/mod.rs`, run `toggle-truncate-lines` before `move-end-of-line` in `syntax-display/rs/1mb-single-line-at-end`. Verify: `cargo bench -p aiditor-core --bench core` gets past that scenario and runs `finder`, `reverts`, `terminals` and `splits`.
- [x] 1.2 Add `syntax-display/rs/1mb-single-line-wrapped-at-end`. It wraps by default, asserts that the display row holding the cursor ends in `xx";` and is coloured, then measures. Verify: the bench passes, and record its time before section 3 lands.
- [x] 1.3 Add `session-render/5000-line-styled-message`. It starts a session through `agent-start` and `Msg::AgentStarted`, sends one `Msg::AgentUpdate` chunk of 5,000 lines mixing headings, emphasis, inline code, links and fenced blocks, and measures the frame that renders it. Verify: the bench runs, and record its time before section 2 lands.
- [x] 1.4 Check that `scripts/test_benchmarks.py` passes with the new names (`python3 -m unittest discover -s scripts -p test_benchmarks.py`).

## 2. Markdown properties in one sweep (#2)

- [x] 2.1 Replace the per-piece scans in `markdown::render` with the ordered sweep from design.md §2: drop empty ranges, record start and end events per cut, keep the open faces in push order, count open hidden ranges, and detect code-line starts at each cut. Verify: every existing `markdown.rs` and `agents.rs` test passes unchanged.
- [x] 2.2 Add a test pinning precedence: a thought holding a heading with a link inside strong emphasis, plus a fenced block. Assert each run's face (the inner face wins and `dim` fills the gaps) and that the code lines aren't joined across their starts. Verify: `cargo test -p aiditor-core markdown`.
- [x] 2.3 Re-run `session-render/5000-line-styled-message`. Verify: its time drops well below the 1.3 baseline.

## 3. Lay out only visible wrapped rows (#1)

- [x] 3.1 Change `display::Wrap.skip` to `from`, the top line's char column where the window's first row starts. Compute it in `frame::window_display` from `window.row_starts(buffer, top)[window.top_row()]`, and make `shown` for a wrapped window `0..(rows + 1) * cols`.
- [x] 3.2 In `display::build`, start the top line at `line_to_char(top) + from`: `first`, `pos`, `properties_near`, and the top line's `drawn` highlight range. Count its columns from `from`. Remove the row drop from `wrap_lines`. Verify: every existing `display.rs`, `frame.rs` and `window.rs` wrap test passes, with fixtures updated from `skip` to `from`.
- [x] 3.3 Add the spec's "window starts deep inside a line" test. A word-wrapped line of 1,000 `word ` copies, 3 rows, point at the end: assert the rows equal the last rows of wrapping the whole line, and check the cursor. Add a matching code-wrap case. Verify: `cargo test -p aiditor-core wrap`.
- [x] 3.4 Re-run `syntax-display/rs/1mb-single-line-wrapped-at-end`. Verify: its time is near the truncated case, not the 1.2 baseline.

## 4. A closed connection ends the agent (#4)

- [x] 4.1 Add a `"closeStdout"` step to `aiditor-fake-agent` that closes stdout and keeps running steps. Add the `fake-agent` spec's scenario to `crates/aiditor-services/tests/fake_agent.rs`. Verify: that test passes.
- [x] 4.2 In `aiditor-services/src/agents.rs`, factor the SIGTERM-then-SIGKILL-after-`GRACE` from `AgentOp::Kill` into one function. Pass the pid and `exited` into `connect`, and call that function when `connect_with` returns after `started`. Verify: the existing `tests/agents.rs` kill and exit tests pass.
- [x] 4.3 Add the two `agent-processes` scenarios to `tests/agents.rs`. In the first, session steps `closeStdout` then sleep for 10 s, and an exit message with a signal arrives within a few seconds. In the second, a turn closes stdout mid-prompt, and the exit message arrives. Verify: `cargo test -p aiditor-services --test agents`.

## 5. Session transcripts follow the theme (#5)

- [x] 5.1 Add `Buffer::map_colors(f)`, which rewrites every property's `Face.fg`, `Face.bg` and `Decoration` colour through `f`. Verify: a `buffer.rs` unit test.
- [x] 5.2 On `Msg::SettingsLoaded` with a changed theme, build the `(old, new)` pairs for `dim`, `code_block`, `text.title`, `text.literal` and `text.uri` (skipping missing or equal ones), and map every session buffer's colours through them. Add a `shortcut:` comment for colliding old colours. Verify: the two `session-buffers` scenarios as `agents.rs` tests (a reply with inline code and a thought, and a tool call's rule), plus a test that an unchanged theme leaves the properties equal.

## 6. Only the current listing fills the file prompt (#3)

- [x] 6.1 Change `FileOp::ListDir(dir)` to `ListDir { dir, request: u64 }` and `Msg::Listed { dir, result }` to `Listed { request, result }`. Update `aiditor-services/src/files.rs` (echo the id), its test at `files.rs:623`, `tests/terminal.rs`, `harness.rs` and `visiting.rs`'s test. Verify: `cargo build --workspace --all-targets`.
- [x] 6.2 Add a `listings: u64` counter to `Prompts`, and replace `Completion.listed` with `awaiting: Option<u64>`, set on each `ListDir` push. `on_listed` accepts only a matching `awaiting` and `take()`s it. Verify: the existing file-prompt tests pass.
- [x] 6.3 Add the spec's "reopened prompt ignores the old listing" test, plus one where the text leaves a directory and comes back. In each, deliver the captured `ListDir` ids out of order and assert the list holds only the newer listing. Verify: `cargo test -p aiditor-core visiting`.

## 7. Docs and wrap-up

- [x] 7.1 Update `docs/editor.md`:
  - soft-wrap note: a line is laid out only from the window's first row, and drop that limit from the risks;
  - find-file note: a listing the prompt no longer awaits is dropped;
  - agent process notes: a closed connection terminates the agent;
  - settings note: a theme reload remaps session colours.
- [x] 7.2 Mark findings #1–#6 resolved in `docs/refactor-scan-2026-10-27.md`, or delete the file if you don't want to keep it. Verify: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass on macOS, and the full `cargo bench -p aiditor-core --bench core` runs to the end.
