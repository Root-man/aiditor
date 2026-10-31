# Tasks

## 1. Shell

- [x] 1.1 Send the percent-encoded command line in zsh's `C` mark, sharing the encoding with `OSC 7`
- [x] 1.2 Update the integration tests in `aiditor-services/tests/terminal.rs` and assert the encoded command

## 2. Terminal state

- [x] 2.1 Parse `cmdline_url` from the `C` mark, sharing the percent decoder with `local_dir`
- [x] 2.2 Keep `running` and `exit` in `Terminal`, updated on every mark, with tests for each transition, the alternate screen and a full scrollback

## 3. Mode line

- [x] 3.1 Add `terminal_mode_line` to `display.rs`: name, directory with `~`, running command, exit, mode, position
- [x] 3.2 Display tests for each state and face, and update the existing `%% t` and `[char]` assertions

## 4. Docs

- [x] 4.1 Record the terminal mode line in `docs/editor.md` and correct the mode line segments paragraph
