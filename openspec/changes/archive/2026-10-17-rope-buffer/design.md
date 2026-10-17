# Design

## Context

`aiditor-core` has `Core { text: Rope, point: usize }`, a hardcoded self-insert in `Core::key`, and a harness test (`harness.rs`) that types `hello RET wörld` and checks text and point. `ropey` 1.6.1 is already a dependency, with default features. The design doc's data model (docs/editor.md, Core data model) lists buffer kinds and content backends: a rope for file, scratch, Messages and session buffers (session decision #8), the emulator grid for terminals.

## Goals / Non-Goals

**Goals:**
- A `buffer` module that markers (#15), undo (#16) and the window model can build on without reshaping it.
- No panic on any edit or conversion input.

**Non-Goals:**
- File I/O: the buffer only takes text and a path. Reading and writing come with #10 and the file-io task.
- Buffer kinds, unique names (`<2>` suffixes), the buffer list order a switcher needs.
- UTF-16 offsets for the input handler; `Rope::char_to_utf16_cu` covers it when #10 needs it.
- Tab-aware display columns; columns here are chars.

## Decisions

**Module shape.** One file, `crates/aiditor-core/src/buffer.rs`:

```
BufferId(u64)                    Copy, Eq, Ord, Hash, Debug
Buffers { next: u64, map: BTreeMap<BufferId, Buffer> }
  create(name) -> BufferId
  create_for_file(path, text) -> BufferId
  get(id) -> Option<&Buffer>,  get_mut(id) -> Option<&mut Buffer>
  kill(id) -> Result<Buffer, Error>
Buffer { name: String, content: Content, file: Option<PathBuf>, modified: bool }
  insert(char_idx, &str) -> Result<(), Error>
  delete(Range<usize>) -> Result<(), Error>
  text() -> &Rope,  len_chars(),  len_lines()
  line_col(char_idx) -> Result<(usize, usize), Error>
  char_idx(line, col) -> Result<usize, Error>
  modified(), set_saved()
enum Content { Text(Rope) }
```

The registry's id counter only increases, so a stale id never finds a newer buffer. `BTreeMap` keeps buffers in creation order for free, which a buffer list will want. A `HashMap` or a `Vec<Option<Buffer>>` slot map were the alternatives. The first loses the order, and the second reuses slots unless it adds generations.

**Content backend: a one-variant enum, not a trait.** `Content::Text(Rope)` is the only backend until the terminal task. Edits `match` on it, so adding `Content::Terminal(..)` later turns each edit into a compile error that has to decide what it means for a grid. That is the point of modelling it now. A `trait Content` was rejected: the backends won't share an interface. A grid has no char-offset insert, and copy mode reads the grid as text instead. Adding the enum later was also rejected, because #15 and #16 will write `buffer.text` access paths that would all move. The session buffer stays `Text` (decision #8).

**Fallible edits through ropey's `try_*`.** `Rope::try_insert`, `try_remove`, `try_char_to_line` and `try_line_to_char` already return `Result`. The buffer maps ropey's error to its own `Error` type, which callers can show in the echo area. ropey's panicking methods are not called with unchecked input.

**Errors.** One small enum in `buffer.rs`, for example `Error::NoSuchBuffer(BufferId)` and `Error::OutOfRange`, with `Display` implemented. No `thiserror` dependency for two variants.

**Only LF breaks lines.** By default ropey also breaks lines at CR, VT, FF, NEL, U+2028 and U+2029 (features `unicode_lines` and `cr_lines`). That would make the editor's line numbers disagree with compilers, grep and `file:line` jumps on any file containing such a char. Set `ropey = { version = "1.6.1", default-features = false, features = ["simd"] }`. CRLF files then show a trailing `\r` per line until the file-handling decision settles line endings, which is the honest state.

**Line/column semantics.** Zero-based. `line_col(idx)` is `(line, idx - line_start)` for `idx <= len_chars` (the end of the text is a valid position). `char_idx(line, col)` clamps `col` to the line's length without its trailing LF, and fails for `line >= len_lines`. Clamping matches what vertical motion needs (`C-n` onto a shorter line), so callers don't each reimplement it.

**Modified flag.** Set by `insert` with non-empty text and by `delete` with a non-empty range, cleared only by `set_saved`. It does not compare text with the disk version, so typing a char and deleting it leaves the buffer modified, as in Emacs. Undo (#16) may refine that.

**Core integration.** `Core` holds `buffers: Buffers`, `current: BufferId` (the `*scratch*` buffer made in `Default`), and keeps `point` until the window model takes it. `Core::key` calls `buffer.insert`, and on error logs with `tracing::debug!` as it does for undefined keys. `Core::text()` reads the current buffer, so `harness.rs` is unchanged. The `shortcut:` comment shrinks to "one current buffer and hardcoded self-insert".

## Risks / Trade-offs

- [Turning off `unicode_lines` changes nothing visible today, but a later reader could re-enable the defaults by accident.] → A one-line comment on the dependency in `Cargo.toml`, plus the `Only LF breaks lines` test.
- [The enum with one variant looks like premature structure.] → It is the issue's explicit ask, and it costs one `match` arm per method.
- [Columns count chars, not graphemes or display width.] → Correct for offsets. Display width is the display builder's job (#34).
