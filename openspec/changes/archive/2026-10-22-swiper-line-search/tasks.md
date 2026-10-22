# Tasks

## 1. Minibuffer groundwork

- [x] 1.1 Add `ArgSpec::filter` and `ArgSpec::preselect`, and `Preview::Point`, in `command.rs`
- [x] 1.2 In `minibuffer.rs`, keep a filtering command's candidates in `Completion::filtered`. Rerun the filter on text changes in `poll_completion` with the anchor selection rule, and branch `matches`, `selected_candidate` and the rows in `Core::minibuffer` on it
- [x] 1.3 Apply `preselect` when the list opens
- [x] 1.4 `exit_minibuffer`: fail with `Match required` on a filtered list with no match. `close_minibuffer` sets `Prompts::last_text` and, for filtered prompts, remembers the text instead of the candidate through `Prompts::remember`, so `C-g` remembers it too
- [x] 1.5 Push the command's name as a minor mode of its minibuffer buffer
- [x] 1.6 Add `Window::jump` (point plus recenter when out of view) and `Core::origin`. Show `Preview::Point` in `Core::preview`, reusing a standing window on the same buffer, and cache previews by value

## 2. Swiper

- [x] 2.1 Add `regex` to `aiditor-core`'s dependencies
- [x] 2.2 Write `swiper.rs` with the candidates, `line_number`, `regexes`, `target`, the filter, the preselect and the preview
- [x] 2.3 Register `swiper` (prompt `Swiper: `) and `swiper-C-s` in `builtins.rs`. Bind `C-s` globally and `[swiper] C-s` in `keymap.toml`, and update the `C-x` hint snapshot if it changes
- [x] 2.4 Write key-script tests in `swiper.rs` covering the scenarios of both delta specs: the listing and line numbers; words in order, exclusion, smart case and literal fallback; selection anchoring; preview and `C-g` restore; search, move, jump with the mark message; an active region; `C-s` next and history; history after `C-g`; match required; position preview keeping the scroll

## 3. Docs and checks

- [x] 3.1 Record swiper in `docs/editor.md` (Minibuffer note: built in #75, and the deferred highlights; Editing commands `C-s`)
- [x] 3.2 Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace`
