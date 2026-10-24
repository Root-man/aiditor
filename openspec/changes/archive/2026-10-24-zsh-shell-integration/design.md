# Design

## Context

`Terminals::spawn` builds a `portable_pty::CommandBuilder`; with no command it is the default program, which `portable-pty` resolves from `SHELL` (then the passwd entry) and runs as a login shell. Core already records OSC 133 and OSC 7 (#243). Under zsh, a precmd hook sees the command's `$?` whatever earlier hooks returned, and zsh iterates a copy of `precmd_functions`, so changing the array inside a hook takes effect from the next prompt.

## Goals / Non-Goals

**Goals:** marks and cwd from any interactive zsh `vterm` starts, with the user's startup files, `ZDOTDIR` and prompt unchanged.

**Non-Goals:** bash and fish integration; explicit commands (`zsh -c ...` from an agent); `PS2` marks; sending `D` without a preceding `C`.

## Decisions

- **One script compiled in, written at spawn.** `include_str!` of `zshenv`, written to `<data dir>/shell/zsh/.zshenv` (the platform data dir, as recovery uses) on each zsh spawn, on the terminal's thread. Writing each time keeps it current with the binary and repairs a deleted file. Not a compile-time path into the source tree, which breaks when the checkout moves; not `/tmp`, which another user could pre-create. If the write fails, the shell starts without integration and a warning is logged, so a broken folder never leaves zsh without the user's files.
- **Detection.** Only the default program is integrated, and only when the resolved shell's file name is `zsh` (`CommandBuilder::get_shell` after the environment is applied).
- **ZDOTDIR handoff.** The user's `ZDOTDIR`, if set in the terminal's environment, travels in `AIDITOR_ZDOTDIR`; otherwise that variable is removed. The script restores or unsets `ZDOTDIR` before sourcing the user's `.zshenv`, so zsh then reads `.zprofile`, `.zshrc` and `.zlogin` from the user's folder, and nested shells behave as outside aiditor. It installs hooks in an `always` block, so a user file that errors or returns still gets them.
- **Hook order.** The script appends a one-shot precmd that, on the first prompt, moves the real precmd to the end of `precmd_functions` (after starship's, added later by `.zshrc`), adds the preexec, and runs the real precmd once itself, since the array change only applies from the next prompt. The precmd prints `D;$?` if a command ran, then `A` and `7`, and appends `%{\e]133;B\a%}` to `PS1` unless it is already there. Preexec prints `C`.
- **OSC 7** is `file://$HOST` plus `$PWD` percent-encoded byte by byte (`LC_ALL=C`), matching core's parser.

## Risks / Trade-offs

- [A `.zshrc` that assigns `precmd_functions=(...)` drops the one-shot hook] → no integration in that shell; same as Ghostty.
- [On the first prompt the real precmd runs before later hooks, so a theme that rewrites `PS1` in its precmd loses `B` on that prompt only] → accepted; starship sets `PROMPT` once at init.
