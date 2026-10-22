# Design

## Context

Completion lists come from one `nucleo` matcher (`minibuffer.rs`), which always sorts matches by score. Previews (`ArgSpec::preview`, `Core::preview`) swap the original window for a fresh `Window` on another buffer and cache the last previewed candidate by name. A command's handler gets only the returned string, and the minibuffer's text is gone once it closes. Swiper needs:

- matches in buffer order, with its own selection rule;
- a selected line at open;
- a preview at a position in the same buffer that depends on the typed text;
- the typed text in the handler;
- its own `C-s` at the prompt.

## Goals / Non-Goals

**Goals:**
- Reuse the minibuffer's list, count, rows, selection keys, preview restore and history. Swiper adds only its matching and its jump.

**Non-Goals:**
- Highlighting matches or the current line.
- Emacs regexp syntax. Words use the `regex` crate's syntax.
- Running the filter asynchronously or in chunks.

## Decisions

### `ArgSpec::filter` replaces the matcher for a command
The new field is `filter: Option<fn(&str, &[String]) -> Vec<usize>>`: it takes the text and every candidate and returns the indices that match, in display order. When it is set, `read_string` keeps the candidates in the `Completion` (`filtered: Option<Filtered>` holding `items: Vec<(String, String)>` with annotations, `matched: Vec<usize>` and `anchor: usize`) instead of injecting them into nucleo. `poll_completion` reruns the filter synchronously when the text changes. Four places branch on `filtered`: `matches`, `selected_candidate`, the rows in `Core::minibuffer` and `poll_completion`.

- **Selection on a text change:** the new selection is the first match whose index is at or after `anchor`, else the last match. `anchor` is the item index of the previous selection, kept while nothing matches. Equivalent to `ivy-recompute-index-swiper`.
- **No match:** `exit_minibuffer` fails with `Match required` when a filtered list has no match. That is ivy's `:require-match`, which swiper passes.
- **Alternatives:**
  - Bending nucleo: its sort can't be turned off in 0.5.
  - A separate swiper UI: it would duplicate the list, count and rows.

### `ArgSpec::preselect`
The new field is `preselect: Option<fn(&Core, &Args) -> usize>`, which sets `Completion::selected` (and `anchor`) when the list opens. It works for nucleo lists too, whose empty-text order is the given order. This is ivy's `:preselect`.

### `Preview::Point(usize)` and previews cached by value
`Point` is a char position in the origin window's buffer.

- **Showing it:** `Core::preview` reuses the window already standing in when it shows that buffer, and otherwise puts back the original and stands in `original.split()`, which copies point, top and rows. It then calls a new `Window::jump(buffers, p)`: set point and, when point's line is outside `top..top + rows`, set `top` so the line sits at `rows / 2`. That is `swiper--maybe-recenter`: the view is kept from one preview to the next.
- **Caching:** the cache moves from the candidate name to the computed value (`previewed: Option<Preview>`, `Preview: PartialEq` already), because swiper's point depends on the text as well as the line. Preview functions are cheap lookups, so they run after every message. `on_previewed` still clears the cache to force the read file to show.
- **Origin access:** `Core::origin()` gives preview functions the window as it was: `saved`, else `others[from]`.

### The typed text after exit
`Prompts::last_text` is set by `close_minibuffer`, so it is set however the prompt closes, as ivy's `ivy-text` survives the read. Swiper's handler reads it to place point.

- **Alternatives:**
  - A second string in `args.strings`: it breaks prompt chaining.
  - Reading back the history: it skips empty and repeated text.

A filtered prompt's history gets the text instead of the candidate, also in `close_minibuffer`, which covers `RET`, `C-g`, a click and a workspace switch. A filtering command gets a filtered list even with no candidates (an empty buffer), so `RET` still requires a match. One helper, `Prompts::remember(prompt, text)`, holds the existing push rules.

### The command's name as a minor mode of its minibuffer
`read_string` pushes `command.name` onto the minibuffer buffer's `minor_modes`. `keymap.toml` gets `[swiper] "C-s" = "swiper-C-s"`, which works like ivy's `:keymap swiper-map`. Command names and mode names don't collide today.

### `swiper.rs`
- **Candidates:** `format!("{n:<w$} {line}")` for each line without its newline, where `w` is the digit count of the line count. The empty line after a final newline is skipped.
- **Line numbers:** `line_number(candidate)` parses the leading number for the handler and the preview. The filter matches each candidate after its first `w + 1` chars, `w` coming from the candidate count.
- **Regexps:** `regexes(text) -> (Option<Regex>, Vec<Regex>)` follows `ivy--regex-plus`:
  1. Split at the first unescaped `!`; `\!` becomes `!`.
  2. Split on runs of spaces; `\ ` becomes a space.
  3. Use each word if it compiles, else `regex::escape` it.
  4. Join the positive words as `(a).*?(b)`, case-insensitive when the whole input equals its lower-case form. When the joined pattern doesn't compile (two groups of one name), join the escaped words instead.
  5. Make each negative word its own regex.
- **Target:** `target(buffer, line, text)` is the end of the positive regex's first match on the line, else the line start. The preview alone keeps the origin point for empty text on the starting line. `RET` with empty text goes to the line start, as `swiper--action` searches `""` from there.
- **`swiper` handler:** reads `last_text`, computes the target from the restored window's point, calls `window.jump`, and pushes the mark at the start point inactive when point moved and no region is active, with `Mark saved where search started`.
- **`swiper-C-s`:** calls `history_element(1)` on empty text, else `next_completion(count)`.

`regex` becomes a direct dependency of `aiditor-core`. It is already in `Cargo.lock` through other crates, so no new crate is downloaded.

## Risks / Trade-offs

- [Synchronous filtering of a large buffer on each key] → A regex over 100k lines is in the tens of milliseconds. Marked `shortcut:`, to move into chunks or onto a thread if it lags.
- [Previews recomputed after every message] → The existing preview functions are hash lookups. Swiper's runs one regex on one line.
- [`RET` recenters from the original window's view, not from the last preview's] → The final scroll can differ by a few lines from what the preview showed. It is accepted, as the restore-on-close rule stays exact.
- [Rust regex syntax rather than Emacs's] → `\(`, `\|` and `\_<` behave differently. Words that don't compile are matched literally, so typing never errors.
