# Tasks

## 1. Hot path

- [x] 1.1 In `Core::poll_completion` (`crates/aiditor-core/src/lib.rs`), return when `!self.minibuffer.as_ref().is_some_and(|mb| mb.completion.is_some())` before `let text = self.text();`, then keep the existing `let Some(completion) = ...` borrow; verify no `self.text()` runs before the early return and the harness minibuffer/completion tests pass (`cargo test -p aiditor-core`)

## 2. Dependencies and dead code

- [x] 2.1 Remove the 14 unused crates from `crates/aiditor-services/Cargo.toml` (keep `aiditor-core`, `serde`, `serde_json`, `tokio`, `tracing`) and `alacritty_terminal` from `crates/aiditor-core/Cargo.toml`; first confirm `grep -rnE '\b<ident>::|use <ident>\b' crates/*/src crates/*/tests` finds nothing for each; verify `cargo build` regenerates `Cargo.lock` without them and `cargo tree -p aiditor-services -e normal --prefix none | sort -u | wc -l` drops well below 171
- [x] 2.2 Delete `crates/aiditor/examples/gpui_spike.rs` (and the now-empty `examples/`); verify `cargo clippy --all-targets -- -D warnings` passes with no unused dev-dependency and `docs/spikes/gpui.md` still points at the `spike/gpui` branch
- [x] 2.3 Mark `Core::display` and `Core::scroll` `#[cfg(test)]`; verify `cargo clippy --all-targets -- -D warnings` (no dead-code warning) and `cargo test -p aiditor-core` pass

## 3. Frontend nits

- [x] 3.1 Rename `struct Debug` to `Stats` in `crates/aiditor/src/main.rs` (field type and `Debug::default` at the constructor); verify `cargo clippy -p aiditor --all-targets -- -D warnings`
- [x] 3.2 In `render`, set `.font_family(font.family.clone())` from the `Font` returned by `self.font(window)` instead of repeating the `settings.font_family` fallback; verify the frontend tests pass (`cargo test -p aiditor`)
- [x] 3.3 Lower `tracing::error!(%err, "save failed")` to `tracing::info!` in the `Effect::Save` task; verify against `openspec show messages-buffer --type spec` that a failed save now yields one `*Messages*` line (the core's echo) and stderr still logs it

## 4. Error conversion

- [x] 4.1 Add `impl From<Error> for String` in `crates/aiditor-core/src/buffer.rs` (via `err.to_string()`) and replace every `.map_err(|err| err.to_string())` in `lib.rs` with `?`, wrapping the two tail expressions as `Ok(...?)`; verify `grep -c 'map_err(|err| err.to_string())' crates/aiditor-core/src/lib.rs` is 0 and the read-only harness tests still see `Buffer is read-only`

## 5. Integration

- [x] 5.1 Run `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and CI's `cargo tree -p aiditor-core` gpui/tokio grep; verify all pass locally, then on CI for Ubuntu and macOS
