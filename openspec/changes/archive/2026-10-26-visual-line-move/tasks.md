## 1. Row motion

- [x] 1.1 Move `scroll`'s row-starts closure into `Window::row_starts` and use it in `scroll`
- [x] 1.2 Make `next_line` step visual rows with a goal column within the row, clamped to a non-last row's last char, reporting the buffer's ends
- [x] 1.3 Tests in `motion.rs`: through a wrapped line and back, short-row clamp in word wrap, a count, last row reports `End of buffer`, `C-a`/`C-e` stay logical, a truncating buffer moves by lines

## 2. Docs

- [x] 2.1 Update `docs/editor.md`: drop `C-n`/`C-p` by row from the soft-wrap "Not yet" and note row motion under Motion
