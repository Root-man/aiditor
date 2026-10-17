# Proposal

## Why

Point moves only through hardcoded arrow keys with no goal column, no prefix argument and no names. The Emacs motion keys are the first real commands on top of the command loop, keymaps and prefix argument (#22, #23, #24), and every later editing feature needs them.

## What Changes

- New named commands, bound in the bundled `keymap.toml`: `forward-char` (`C-f`, `<right>`), `backward-char` (`C-b`, `<left>`), `next-line` (`C-n`, `<down>`), `previous-line` (`C-p`, `<up>`), `forward-word` (`M-f`), `backward-word` (`M-b`), `move-beginning-of-line` (`C-a`), `move-end-of-line` (`C-e`), `beginning-of-buffer` (`M-<`), `end-of-buffer` (`M->`), `scroll-up-command` (`C-v`), `scroll-down-command` (`M-v`), `recenter-top-bottom` (`C-l`).
- Each honours the prefix argument as in Emacs: a count for char, line and word motion (negative reverses), tenths of the buffer for `M-<`/`M->`, lines for `C-v`/`M-v`, a target row for `C-l`.
- Vertical motion keeps a goal column across consecutive `C-n`/`C-p`.
- Motion that runs into an end of the buffer moves as far as it can and reports `Beginning of buffer` or `End of buffer`.
- The window remembers how many lines it showed at the last redisplay, so scrolling commands know a screenful.
- **BREAKING** (behaviour): the arrow keys are now these commands, so up on the first line goes to the buffer start and down on the last line to its end, instead of doing nothing. The hardcoded arrow-key motion in `Ctx::key` is removed.

## Capabilities

### New Capabilities
- `motion`: the commands that move point by chars, lines, words, line ends, buffer ends and screenfuls, and recenter the window.

### Modified Capabilities
- `editing-keys`: the arrow-key requirement is removed; arrows are bindings of the motion commands.

## Impact

- `crates/aiditor-core/src/lib.rs` (commands, goal column, fallback removal), `window.rs` (rows, setting the first line), `keymap.toml`, harness tests.
- No frontend change: `Core::display` already passes the rows to the window.
