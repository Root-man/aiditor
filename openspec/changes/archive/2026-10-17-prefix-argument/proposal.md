# Proposal

## Why

Every command already gets a prefix argument, but no key can set one yet. The motion and editing commands (#27, #28) need `C-u 3 C-f` and `C-u 3 a`, so issue #23 adds Emacs's prefix argument keys.

## What Changes

- `C-u` starts a prefix argument of 4; each further `C-u` multiplies it by 4.
- Digits typed after `C-u` (and after `M-<digit>`) form the number; `-` right after `C-u` makes it negative.
- `M-0` to `M-9` start or extend a numeric prefix argument; `C--` starts a negative one (-1 alone).
- The next command that is not one of these takes the prefix argument and clears it; quitting with `C-g` clears it too.
- **BREAKING** (internal API): the argument a command gets keeps the raw form (`C-u`s with no digits, a lone minus, or a number) with its numeric value, so commands can tell `C-u` from `C-u 4`.
- The prefix argument keys leave the last command alone, like prefix keys.

Not in this change: `M--`, `C-<digit>`, `C-M-<digit>`, echoing `C-u-` while the argument is typed, and plain typed keys honouring the count (#28 makes them commands).

## Capabilities

### New Capabilities
- `prefix-argument`: the keys that build a prefix argument and how the next command receives it.

### Modified Capabilities
- `commands`: the prefix argument a command receives keeps its raw form as well as its number.

## Impact

`aiditor-core` only: `command::Args.prefix` changes type, three built-in commands and their global and transient bindings are added, and `Core` keeps the prefix argument being built and the keys of the running command. The frontend is unchanged.
