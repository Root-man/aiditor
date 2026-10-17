# Tasks

## 1. Core hint

- [x] 1.1 Add `Keymap::bindings` listing a prefix map's keys and bindings, and `display::Hint` with `keys` and sorted `entries`; verify through the hint tests in 1.2
- [x] 1.2 Add `Msg::Hint` and the hint timer to the key loop (start after a prefix, cancel and hide on every key), and `Core::hint()`; verify with harness tests for each spec scenario, including an insta snapshot of the `C-x` hint
- [x] 1.3 Record the decision in `docs/editor.md` under Commands and keymaps; verify the text is there

## 2. Frontend

- [x] 2.1 Draw the hint as one line above the message line in the skeleton frontend and count it in the bars; verify `cargo build -p aiditor` and the CI checks pass
