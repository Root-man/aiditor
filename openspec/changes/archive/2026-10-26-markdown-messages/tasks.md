# Tasks

## 1. Buffer and display

- [x] 1.1 Add `Buffer::remove_properties(range)` and relax the fast-path flag to "starts and ends ascend"; verify with buffer unit tests (removal scenario, nested ascending properties found by `properties_near`)
- [x] 1.2 With folds, look up properties near the window, widening it by hidden newlines; verify the existing display and frame fold tests still pass

## 2. Theme

- [x] 2.1 Add `dim` and `code_block` to `Theme` and `settings.toml`; verify a settings test reads a user `code_block`

## 3. Markdown rendering

- [x] 3.1 Enable `tree-sitter-md`'s `parser` feature and add `markdown.rs` with `render(text, thought, theme)`; verify unit tests for headings, emphasis, strong, inline code, links, autolinks, fenced and indented code blocks, thoughts, non-ASCII offsets, and sorted output

## 4. Session buffer

- [x] 4.1 Track the streamed block's start and kind, show thought chunks as blocks, and render the echoed prompt on send; verify harness tests for thought blocks and the bold echo
- [x] 4.2 Re-render the dirty streamed block in `Core::frame()` and render a finished block before the next starts; verify harness tests that chunks wait for the frame, earlier blocks keep their properties, and the display shows hidden markup and code backgrounds
- [x] 4.3 Verify swiper matches hidden markup and `M-w` over a rendered reply yields markdown, with harness tests

## 5. Docs

- [x] 5.1 Record what was built in `docs/editor.md` (Session buffer bullet) and drop the "Not yet: faces, markdown" note
