# Tasks

## 1. Buffer and registry

- [x] 1.1 In `crates/aiditor-core/Cargo.toml`, set ropey to `default-features = false, features = ["simd"]` with a one-line comment on why (LF-only lines). Verify with `cargo tree -p aiditor-core -e features -i ropey`, which shows no `unicode_lines` or `cr_lines`.
- [x] 1.2 Add `crates/aiditor-core/src/buffer.rs` (exported as `pub mod buffer`) with `BufferId`, `Buffers` (`create`, `create_for_file`, `get`, `get_mut`, `kill`), `Buffer`, `Content::Text` and `Error`, as in design.md. Add unit tests for the spec's registry and file-binding scenarios (create and look up, ids not reused, killed buffer gone, buffer for a file, scratch has no file), and verify with `cargo test -p aiditor-core buffer`.
- [x] 1.3 Add `insert` and `delete` through ropey's `try_*` methods, and the modified flag with `set_saved`. Add unit tests for the spec's edit, invalid-edit and modified-flag scenarios, including the non-ASCII ones and the check that a failed edit leaves the text and flag unchanged. Verify with `cargo test -p aiditor-core buffer`.
- [x] 1.4 Add `line_col`, `char_idx` and `len_lines`. Add unit tests for every line/column scenario in the spec (non-ASCII text, column clamping, LF-only breaks, line past the end), and verify with `cargo test -p aiditor-core buffer`.

## 2. Core uses the registry

- [x] 2.1 Replace `Core`'s `text: Rope` with `buffers: Buffers` and `current: BufferId` (a `*scratch*` buffer created in `Default`). Route self-insert through `Buffer::insert` and log errors with `tracing::debug!`, and narrow the `shortcut:` comment. Add `modified` to the scratch-typing assertion in the harness test. Verify with `cargo test -p aiditor-core`, where the existing harness test passes.
- [x] 2.2 Run CI's checks, `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`, and verify all three pass.

## Workflow follow-up

- Open a PR that closes #14, then archive the change after merge.
