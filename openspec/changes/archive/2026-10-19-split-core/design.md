# Design

## Context

See proposal.md, Why. Line references are as of `8fa1ddd` and were re-checked against the code; they all still match.

- Everything editor-side is in `crates/aiditor-core/src/lib.rs`: `impl Ctx<'_>` (`235-1382`), `struct Core` (`1384-1452`), `impl Default for Core` (`1459-1799`), `impl Core` (`1801-2330`), `window_display` (`2334-2366`). There are 27 `let core = &mut *self.core;`.
- `harness.rs` has the `Harness` (`1-258`) and one `mod tests` (`261-2867`, 143 `#[test]`s), with no `#[cfg(test)]` of its own because `mod harness` is already `#[cfg(test)]` (`lib.rs:10`). Other modules' tests already use `crate::harness::Harness` (`display.rs:193`), so that pattern exists.
- The crate's other modules (`buffer`, `window`, `display`, `keymap`, `key`, `timer`, `settings`, `command`) stay as they are; only `lib.rs` and `harness.rs` are split.
- Constraints: no behavior change; same test names and count (`cargo test -p aiditor-core -- --list`); no gpui or tokio in `aiditor-core`; disjoint field borrows such as `core.window.point(&core.buffers)` must keep working.

## Goals / Non-Goals

**Goals:**
- A module per subsystem, so a new M1 subsystem gets its own file instead of growing `lib.rs`.
- A command table where adding a command touches one place and a typo or duplicate fails loudly.

**Non-Goals:**
- Any renaming of commands, messages, effects or public `Core` methods.
- The window representation (`window` + `others` + `MINIBUFFER` sentinel), `Result<_, String>` errors, string command names, the one-variant `Content` enum.
- P3 (inverting `where_is` for `M-x` annotations): out, see Decisions.
- A grouped struct for every field cluster the brief lists: only where it earns its keep, see Decisions.

## Decisions

### D1. Modules

Child modules of the crate root, one `impl Ctx<'_>` and/or `impl Core` block each, named after the spec capability they implement:

| Module | Spec | Contents |
|---|---|---|
| `lib.rs` | command-loop, core-loop, commands, echo-area, messages-buffer | `Msg`, `Recovered`, `Effect`, `Ctx` (`start_timer`, `cancel_timer`, `call`, `try_call`, `dispatch`, `key`), `PANIC_HOOK`/`PANIC`, `Core` struct, `Default` (now short), `handle`, `echo`/`log`/`report`, `register`, `next_deadline`, the small public accessors (`current`, `current_mut`, `text`, `point`, `settings`, `this_command`, `last_command`, `start_recording`, `take_recording`, `message`) |
| `builtins.rs` | commands | `builtins() -> Vec<Command>` (D3) |
| `editing.rs` | editing-keys, undo | `insert_n`, `delete_char`, `delete_n`, `delete_to`, `replace_utf16`, `undo` |
| `motion.rs` | motion | `goto`, `goto_clamped`, `forward_char`, `next_line`, `forward_word`, `word_end`, `line_end`, `buffer_end`, `scroll_lines`, `scroll_command`, `recenter`, `click` |
| `kill_ring.rs` | kill-ring | `KillRing` (D2), `KILL_RING_MAX`, `kill`, `kill_to`, `kill_region`, `kill_line`, `yank`, `yank_pop`, `point_before_yank` |
| `mark.rs` | mark | `set_mark_command`, `push_mark`, `pop_to_mark`, `exchange_point_and_mark`, `mark` |
| `prefix.rs` | prefix-argument | `set_prefix` (the universal/digit/negative handlers stay inline in `builtins`, they are one-liners calling it) |
| `minibuffer.rs` | minibuffer | `Minibuffer`, `Completion`, `Prompts` (D2), `MINIBUFFER`, `HISTORY_MAX`, `COMPLETION_ROWS`, `REPOLL`, `read_string`, `exit_minibuffer`, `history_element`, `set_text`, `next_completion`, `outside_minibuffer`, `on_listed` (D4), `poll_completion`, `component_start`, `minibuffer()`, `close_minibuffer` |
| `frame.rs` | windows, display-model | `split_window`, `other_window`, `delete_window`, `delete_other_windows`, `Core::select`, `scroll`, `display()`, `frame()`, `window_display` |
| `visiting.rs` | file-visiting, buffer-switching, quitting | `find_file`, `save`, `on_saved` (D4), `kill_buffer` (both), `save_buffers_kill_terminal`, `directory`, `open`, `visit`, `switch_to`, `switch_to_name`, `buffer_names` |
| `autosave.rs` | auto-save | `AUTOSAVE_IDLE`, `AUTOSAVE_MAX`, `autosaves()`, `Core::autosave`, `write_autosaves` (D4), `recover_buffers` |
| `bindings.rs` | keymaps | `HINT_DELAY`, `parse_keymaps`, `lookup`, `layers`, `where_is`, `hint`, `wants_clipboard` |

Why these names: the brief's `keys.rs` would sit next to the existing `key.rs` and `keymap.rs`; `bindings.rs` (what a key sequence is bound to under layering) reads differently. `window.rs` is taken by `Window`/`Tree`, hence `frame.rs`. No module is named `files` (services has one). Quitting stays in `visiting.rs` because it is mostly saving; a separate `quitting.rs` would hold one function plus half of `on_saved`.

Visibility: the brief says no visibility changes are needed. That holds for the crate root's private fields and items, which children can read, but not for methods moved into a child: a private `fn goto` in `crate::motion` is invisible to `builtins.rs` and `lib.rs`. So every moved function or struct used outside its new module becomes `pub(crate)`; `Core`'s and `Ctx`'s fields stay private. Moved code is otherwise verbatim (only `use` lines added), so the first commit reviews with `git diff --color-moved`.

Alternative considered: one `ctx/` and one `core/` directory split by receiver type. Rejected: a subsystem's `Ctx` and `Core` methods belong together (e.g. `read_string` and `poll_completion`), and the capability names match the specs the next reader opens.

### D2. State structs: `KillRing` and `Prompts`, nothing else

The rule: extract a struct only where its methods mostly touch its own fields and borrows don't get worse.

- `KillRing { ring, yank_index, clipboard, clipboard_seen }` in `kill_ring.rs`, field `core.kills`. `kill_new`, `current_kill` and `take_clipboard` touch only these fields, so they become `KillRing` methods (`take_clipboard` moves from `Ctx`; it reads nothing else). `kill` reads `core.last_command` while holding `core.kills.ring.front_mut()`; that is a disjoint field borrow, as today. Test helpers reading `h.core.kill_ring` become `h.core.kills.ring`.
- `Prompts { minibuffer, history, matcher, repoll }` in `minibuffer.rs`, field `core.prompts`. `matches` and `selected_candidate` touch only these, so they become `Prompts` methods. `poll_completion` and `component_start` also need the buffer text and point, and `poll_completion` the timers, so they stay on `Core` and reach in through `self.prompts`. `read_string` calls `annotation(core, &c)` after taking `core.matcher`'s injector; the injector is an owned handle, so the borrow ends as it does today. `Harness::settle` (`harness.rs:95`) changes to the new paths.
- Command-loop fields (`transient`, `pending`, `hint_timer`, `hint_shown`, `this_command`, `last_command`, `prefix_arg`, `this_keys`, `recording`, `self_inserts`): left on `Core`. No method touches only them; `key`, `click`, `kill`, `yank_pop`, `hint` and `wants_clipboard` all mix them with buffers, windows, timers and keymaps, and `last_command`/`this_command` are read across half the modules. A struct would only rename `core.pending` to `core.loop_.pending` at every use.
- Auto-save fields (`autosaved`, `autosave_idle`, `autosave_max`, `recovered`): left on `Core` for the same reason; `autosave`, `write_autosaves` and `recover_buffers` all work on buffers and timers. They already sit together in the struct and live in one module after D1. `quitting` stays with the quit flow.
- Windows, `goal_column`, `recenter`: untouched, as the brief says.

### D3. Command table

`builtins.rs` has `pub(crate) fn builtins() -> Vec<Command>`, one `Command { name, spec, handler }` literal per command, in today's order. Commands without arguments use `spec: ArgSpec::default()`; the 13 with specs write theirs inline (`ArgSpec { clipboard: true, ..ArgSpec::default() }` for the kills and `yank`, plus `region: true` for `kill-region` and `kill-ring-save`, and the prompts/completion/annotation specs moved from `lib.rs:1693-1754` unchanged). Non-capturing closures coerce to `Handler` in a struct literal field.

`Core::default` builds the map with one loop that asserts each insert is new: `assert!(commands.insert(c.name, c).is_none(), "duplicate builtin {}", c.name)`, the same style as the existing `assert!(errors.is_empty(), "bundled keymap: …")`. It runs on every `Core` construction, so every test checks it, and it costs nothing next to keymap parsing. A test instead would add one to the test count, which must stay identical. `builtins_are_callable_by_name` stays.

Alternatives: a `static`/`const` slice with a `const NONE: ArgSpec` (needs a new const and buys nothing, since the table is built once); a builder (rejected by the brief).

### D4. `dispatch` arms as named methods

- `Ctx::on_saved(buffer, tick, result)` in `visiting.rs`: the `Msg::Saved` arm (`lib.rs:479-511`), including the quit bookkeeping.
- `Ctx::on_listed(dir, result)` in `minibuffer.rs`: the `Msg::Listed` arm (`512-535`).
- `Ctx::write_autosaves()` in `autosave.rs`: the `Msg::AutoSave` arm (`564-584`), cancelling both timers first, as now.
- `recover_buffers` calls `self.write_autosaves()` instead of `self.dispatch(Msg::AutoSave)` (`lib.rs:1339`). The arm does nothing else, so behavior is identical.

### D5. Tests

Each block of `harness.rs`'s `mod tests` moves to a `#[cfg(test)] mod tests` in its module, in today's order, using `crate::harness::Harness`:

| Tests (harness.rs lines at 8fa1ddd) | Module |
|---|---|
| typing and undo (277-345), DEL and editing keys (620-632, 768-858, 901-915), input methods and inserted text (1533-1579) | `editing` |
| command calls, errors, panics, echo area (347-487), timers (1158-1213), command tracking and recording (1391-1472), `*Messages*` (2389-2425) | `lib.rs` |
| `builtins_are_callable_by_name` (488) | `builtins` |
| open/save/refuse/encoding (503-610), buffer switching (2332-2388), kill buffer and quitting (2426-2546), find-file (2547-2708) | `visiting` |
| `c_x_then_another_key_does_nothing` (611), settings and keymap reload (1075-1157), keymap layering, hints, undefined keys, `C-g` (1214-1390) | `bindings` |
| char/line/word motion, line and buffer ends, scrolling (633-767, 859-900, 1007-1074), wheel and click (1580-1623) | `motion` |
| mark (916-1006) | `mark` |
| prefix argument (1473-1532) | `prefix` |
| kill ring (1624-1809) | `kill_ring` |
| windows, frame display, sideways scroll (1810-2015) | `frame` |
| prompts, completion, `M-x` (2016-2331) | `minibuffer` |
| auto-save and recovery (2709-2867) | `autosave` |

Helpers used by more than one module (`msg`, `MS`, `on`, `at`, `line_start`, `twenty_lines`, and any other the compiler finds shared) move into `harness.rs` as `pub(crate)` items next to the `Harness`; single-module helpers move with their tests. The brief says `harness.rs` keeps "only the Harness"; shared test helpers are part of the harness, and duplicating them per module would be worse. The exact split of a borderline test (say, one that tests both motion and editing keys) follows the block it sits in; nothing is renamed.

Check: `cargo test -p aiditor-core -- --list | sort` before and after must be identical apart from module paths, and the count equal.

### D6. P3 left out

Inverting `where_is` needs a batch annotation hook beside `ArgSpec.annotation`, and must keep the exact rule (shortest, then first in `kbd` order, only sequences `lookup` resolves to the command). That is not a few lines, and today's cost (~57 commands × ~80 bindings per `M-x`) is negligible. Add instead, at the `M-x` annotation in `builtins.rs`: `// shortcut: where_is walks every keymap per command, commands × bindings per M-x; invert into one walk when M-x opens noticeably slower.`

## Risks / Trade-offs

- [Huge diff conflicts with in-flight work] → Land after `m0-quick-wins` and `clip-long-lines`, while nothing else touches `aiditor-core`; commit 1 is a pure move so a rebase can redo it mechanically.
- [Tests silently dropped in the move] → Compare `cargo test -p aiditor-core -- --list` before and after (D5); the harness module loses its `mod tests` entirely, so a forgotten block is a compile-time leftover, not a silent drop.
- [Borrow-checker friction from `KillRing`/`Prompts`] → Both were picked because their call sites already borrow them apart from other fields; if a site still fights, keep that field on `Core` rather than add clones.
- [`pub(crate)` widens visibility] → Only within the crate; the public API seen by `aiditor` and `aiditor-services` is unchanged.
- [The duplicate-name `assert!` runs in release builds] → A one-time startup check over ~57 entries; a duplicate is a programming error worth crashing on, like the bundled-keymap assert.

## Migration Plan

One PR, five commits in the order of tasks.md, each passing fmt, clippy and tests. Rollback is reverting the PR; there is no data or format change.
