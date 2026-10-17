# Design

## Context

`Ctx::key` in `crates/aiditor-core/src/lib.rs` hardcodes every key, with private `undo` and `save` methods and an inline settings reload. `Core::message` is the one-line stand-in for the echo area. There is no prefix argument, minibuffer or key trie yet.

## Goals / Non-Goals

**Goals:** a registry the key trie (#21), command loop and `M-x` can build on; tests call commands by name with explicit arguments.

**Non-Goals:** collecting arguments interactively (prompts, prefix keys), listing names for completion, per-buffer or per-mode commands.

## Decisions

- **Plain function pointers, `fn(&mut Ctx, Args) -> Result<(), String>`.** As the design doc says. Closures with captured state are not needed: state lives in the core. String errors match `Core::message`; a typed error enum is not worth it until something matches on errors.
- **`HashMap<&'static str, Command>` owned by `Core`, filled in `Core::default`, with `Core::register`.** Tests register their own commands with `register`. A static table would block that. Names are `&'static str` because commands are defined in code.
- **`ArgSpec { region: bool, prompts: &'static [&'static str] }`.** The prefix argument is not in the spec because every command gets it. The prompts are the prompt texts the minibuffer will show later; for now they also name the missing argument in errors.
- **`Args { prefix: Option<i64>, region: Option<Range<usize>>, strings: Vec<String> }`, `Default`.** `Option<i64>` covers absent versus a number. Whether `C-u` alone must differ from `C-u 4` (Emacs's raw prefix) is for the prefix-argument issue; it can widen the type then.
- **`Ctx::call(&mut self, name, Args)` validates and fills, then runs, and reports any error to `core.message`.** It returns nothing: the caller (a key, a test, later `M-x`) never needs the error besides showing it. The region is filled from the window's point and mark with Emacs's message "The mark is not set now, so there is no region". Tests read `core.message()`.
- **Built-ins move into handlers.** `undo`/`undo-redo` and `save-buffer` reuse the existing methods; `reload-settings` pushes `Effect::LoadSettings`. The undo boundary and self-insert counting stay in `Ctx::key` until the command loop owns them.

## Risks / Trade-offs

- [The message line is overwritten by the next message] → the echo area issue (#36) replaces it; no change here.
- [`undo` reports "No further undo information" through `core.message` directly, not as an `Err`] → it is information, not a failure; keeping it avoids touching undo tests.
