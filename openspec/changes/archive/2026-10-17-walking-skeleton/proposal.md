# Proposal

## Why

The core can hold buffers and handle keys, but there is no app to run: `aiditor` opens an empty window that ignores the keyboard. Issue [#10](https://github.com/Root-man/aiditor/issues/10) asks for a walking skeleton, `aiditor FILE` that opens, edits and saves a real file, so every later task lands in an app you can use.

## What Changes

- `aiditor FILE` reads the file and the core visits it in a buffer that becomes current. A missing file opens as an empty buffer visiting that path, as in Emacs; any other read error exits with a message. With no argument the app starts on `*scratch*` as before.
- Hardcoded editing keys in the core: self-insert and `RET` (as today), `DEL` deletes the char before point, the four arrow keys move point, and the prefix `C-x` followed by `C-s` saves. Arrow keys get key names (`<left>`, `<right>`, `<up>`, `<down>`) in key scripts.
- Saving goes out as the first real effect: the core asks the app to write the buffer's text to its file, and the app reports success back as a message, which clears the modified flag. A failed write is logged and leaves the buffer modified.
- The GPUI view takes keyboard focus, turns key-down events into key messages, draws the buffer's lines in a monospace font with a cursor, keeps point on screen, and shows the buffer name and a modified mark in the window title.

Out of scope, each replaced by a later task: the key trie and command loop, `Msg::Insert` with IME and dead keys (#12 "Feed key and mouse events"), the display builder and real rendering, the file service with a hash check, goal column, undo, mouse.

## Capabilities

### New Capabilities

- `file-visiting`: opening a file from the command line into a buffer and saving it back with `C-x C-s`.
- `editing-keys`: the keys that edit and move point in the current buffer: self-insert, newline, delete backward and arrow motion.

### Modified Capabilities

None. `core-loop`, `text-buffers` and `timers` keep their requirements.

## Impact

- `crates/aiditor-core`: `key.rs` (arrow key codes and names), `lib.rs` (`Core::open`, editing keys, `C-x` prefix, `Effect::Save`, `Msg::Saved`), `harness.rs` (runs effects, tests).
- `crates/aiditor/src/main.rs`: argument handling, file read and write, key translation, drawing.
- No new dependencies.
