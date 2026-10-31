# Tasks

## 1. Core model

- [x] 1.1 Add `ModeLine`, `Segment`, `ModeBar` and `text()` to `display.rs`; supply each buffer type's segments in `build`; move `[unwatched]` to the right group in `frame.rs`; update every test and snapshot reading `mode_line`, and verify `cargo test -p aiditor-core` passes with `aiditor-services/tests/agents.rs` also compiling
- [x] 1.2 Add `ModeLine::draw` (separators, faces, inactive) and `FrameWindow.mode_line`, sharing `fit_names`' room rule, and verify display tests for both groups, an absent segment, inactive faces, and truncation in a narrow window pass

## 2. Frontend

- [x] 2.1 Draw left and right groups from `FrameWindow.mode_line` in `element.rs`, right group ending at the right inset, and verify a `TestAppContext` test of the right group's position passes

## 3. Docs

- [x] 3.1 Record the segments, colours and truncation in `docs/editor.md` and verify `openspec validate mode-line-segments --strict` passes

## Workflow follow-up

- Archive the change after the review.
