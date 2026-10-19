# Proposal

## Why

M0's features are in and manual testing on macOS comes next. Six small, independent items either remove a hot-path cost or cut build time, so they land first; `clip-long-lines` and `split-core` move the same code and come after. The worst one: `Core::poll_completion` (`crates/aiditor-core/src/lib.rs:1995`) copies the whole current buffer into a `String` (`self.text()`, `lib.rs:2200`) before it checks whether a completing minibuffer is open, and `Core::handle` calls it after every message (`lib.rs:1962`). With no minibuffer open, editing a 10 MB file costs a 10 MB copy per keystroke and per `Tick`. Separately, `aiditor-services` declares 14 crates nothing uses (SQLite's bundled C source and five tree-sitter C grammars among them), and CI builds all of them on both operating systems.

## What Changes

- **P1** `poll_completion` returns before copying the text when no completing minibuffer is open: check `self.minibuffer.as_ref().is_some_and(|mb| mb.completion.is_some())` first, then take the text, then the mutable borrow.
- **H1** Remove unused dependencies. From `crates/aiditor-services/Cargo.toml`: `agent-client-protocol`, `grep-regex`, `grep-searcher`, `ignore`, `notify`, `nucleo`, `portable-pty`, `rusqlite`, `tree-sitter`, `tree-sitter-elixir`, `tree-sitter-javascript`, `tree-sitter-md`, `tree-sitter-rust`, `tree-sitter-typescript`. From `crates/aiditor-core/Cargo.toml`: `alacritty_terminal`. Regenerate `Cargo.lock`. They return with the milestone that uses them (M1 `ignore`/`grep-*`, M2 `portable-pty`/`alacritty_terminal`, M3 ACP, M4 `rusqlite`, M5 `tree-sitter`). `docs/editor.md`'s crate shortlist lists choices, not current dependencies, and stays as is.
- **H2** Delete `crates/aiditor/examples/gpui_spike.rs` (556 lines). It survives on the `spike/gpui` branch, locally and on `origin`, which `docs/spikes/gpui.md` already names, so its run command there stays correct. `crates/aiditor`'s dev-dependencies (`gpui` with `test-support`, `tempfile`) are still used by `main.rs`'s tests.
- **H3** `Core::display` (`lib.rs:2237`) and `Core::scroll` (`lib.rs:2327`) become `#[cfg(test)]`. Only the harness calls them (`harness.rs:245`, `harness.rs:1010-1043`); the frontend uses `Core::frame`. `pub(crate)` alone would warn as dead code outside tests, which clippy `-D warnings` fails.
- **H4** Frontend nits in `crates/aiditor/src/main.rs`:
  - Rename `struct Debug` (`main.rs:57`) to `Stats`; it shadows the `Debug` derive's name.
  - `render` already holds the resolved `Font` from `self.font(window)` (`main.rs:505`); use its family at `main.rs:570` instead of repeating the `unwrap_or(FONT.into())` fallback from `main.rs:256`.
  - A failed save appears twice in `*Messages*`: once from `tracing::error!(%err, "save failed")` (`main.rs:135`, through `messages_layer`, WARN and up) and once from the core echoing the error on `Msg::Saved` (`lib.rs:497`). Lower the `tracing` call to `info!` so stderr keeps the record and `*Messages*` gets the echo only. This matches the `messages-buffer` spec: every echo is logged, lines below warning level are not appended, stderr's log is unchanged in content.
- **S8** `impl From<Error> for String` in `crates/aiditor-core/src/buffer.rs`, and replace the 12 `.map_err(|err| err.to_string())` calls in `lib.rs` with `?` (or `Ok(..?)` where the call is a tail expression, `lib.rs:956` and `lib.rs:1043`). `Error`'s `Display` is already the user-facing text, so messages don't change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None: this is refactor and hygiene; behavior stays as `openspec/specs/` specifies (a failed save still logs to `*Messages*` once, as `messages-buffer` and `echo-area` require). The change sets `skip_specs: true`.

## Impact

- `crates/aiditor-core/src/lib.rs`, `crates/aiditor-core/src/buffer.rs`, `crates/aiditor/src/main.rs`, all three crates' `Cargo.toml`s (core and services edited), `Cargo.lock`; `crates/aiditor/examples/` removed.
- No new dependencies, so CI's `cargo tree -p aiditor-core` gpui/tokio check (which includes dev-dependencies) is unaffected.
- Existing tests pass unchanged; no new tests (every item is trivial or covered by the harness's minibuffer, completion and scroll tests).
- Out of scope: everything in the other refactor briefs, new features, API redesign.
