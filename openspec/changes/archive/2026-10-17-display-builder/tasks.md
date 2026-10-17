# Tasks

## 1. Text properties

- [x] 1.1 Add `Prop`, `Face`, `Decoration`, `Buffer::add_property` and `Buffer::properties` (marker pairs); verify with unit tests for the text-buffers scenarios (insert inside and at edges, range deleted whole, range past the end)

## 2. Settings

- [x] 2.1 Add `region` to `Theme` and `settings.toml`, and `Settings::line_height()`; use it in the frontend; verify settings tests pass and a user `region` overrides the default

## 3. Display builder

- [x] 3.1 Add `display.rs` with the model types and builder, and `Core::display(height)`; verify with harness `insta` snapshots and asserts covering short and scrolled buffers, faces, a multi-line fold, hidden markup, decorations, cursor after a fold, region with and without mark, and the mode line
- [x] 3.2 Note text properties in the Buffer row of `docs/editor.md`; verify the doc reads correctly

## 4. Checks

- [x] 4.1 `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass

## Workflow follow-up

- Archive the change after review.
