# Tasks

## 1. Count

- [x] 1.1 Add a grow-only count width to `Completion`, starting at 6, and pad the count right-aligned in `Core::minibuffer`; verify with a test that drives a completing minibuffer through growing and shrinking matches and asserts the count's width and the reopened minibuffer's reset
- [x] 1.2 Update exact count assertions in `minibuffer.rs` and `swiper.rs` tests and verify `cargo test --workspace` passes

## 2. Grep

- [x] 2.1 Add a `grep.rs` test sending batches over a growing hit count (9 to 100): the count's width, the selected hit and the shown rows hold, and the next search's first answer selects the first hit

## 3. Docs

- [x] 3.1 Record the padded, grow-only count in `docs/editor.md`'s Minibuffer entry and verify the text reads ` 1/100 `
