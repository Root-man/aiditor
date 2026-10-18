# Design

## Context

The minibuffer (#38) is a `minibuffer-mode` buffer in a window kept outside the split tree, opened by `Ctx::read_string` when a call lacks a prompted string and closed by `exit-minibuffer` or `abort-minibuffers`. The frame display carries `display::Minibuffer { prompt, display }` and the frontend draws it in the echo area line. The core must stay free of GPUI and Tokio (CI checks it).

## Goals / Non-Goals

**Goals:** a candidate source on the command spec, fuzzy filtering with `nucleo` off the UI thread, a selection, and the list in the frame display and the frontend.

**Non-Goals:** `M-x` itself (#40) and other completing commands; match highlighting; async candidate sources (a file walk feeding the injector over time).

## Decisions

- **Candidates on `ArgSpec`.** `completion: Option<fn(&Core, &Args) -> Vec<String>>`, called when the minibuffer opens, with the args so far. A plain fn fits the `&'static` spec the way `prompts` does, and `Args` lets a later command vary candidates by earlier answers without a per-prompt table. Alternative: a per-prompt slice of sources — more shape than any planned command needs.
- **One `nucleo::Nucleo<String>` per core, made on first use.** `Nucleo::new` starts a thread pool, so the core keeps one and calls `restart(true)` per prompt, then pushes the candidates through its injector. The notify callback does nothing: the core polls instead.
- **Polling.** After every message the core, when completing, reparses the pattern from the minibuffer text (`append` when the new text extends the old, as nucleo can then narrow its last matches) and calls `tick(10)`; `Core::minibuffer`, which the frame display calls, ticks with 0 ms. If the matcher still reports `running`, the core starts a 16 ms timer with `Msg::Tick`, so the app wakes, polls again and redraws. Small lists finish inside the first tick, so the list is current the same frame. Alternative: a notify callback waking GPUI — that needs the frontend to own a channel into the core for no gain over the timer the core already has.
- **Selection is an index into the matches**, reset to 0 when the text differs from the text last matched, clamped to the match count after every tick. The visible window of 10 scrolls only as far as needed to hold the selection, Vertico's behavior.
- **Keymap.** `[minibuffer-completion]` binds `C-n`, `C-p`, `TAB`; the minibuffer buffer gets it as a minor mode only when the spec has candidates, so it outranks `minibuffer-mode` and the global map and plain prompts are untouched. `RET` stays `exit-minibuffer`, which takes the selected candidate when there is one; that candidate, not the typed text, goes into the prompt's history, as in Emacs. Command names follow Emacs: `minibuffer-next-completion`, `minibuffer-previous-completion`, `minibuffer-complete`.
- **Display.** `display::Minibuffer` gains `candidates: Vec<String>` (the visible window) and `selected: Option<usize>` (index into it). The frontend draws one line per candidate below the prompt line, the selected one on the theme's `region` background.
- **Harness.** Matching is threaded, so the harness keeps ticking with a longer timeout until the matcher stops running before it returns from a message, keeping tests deterministic.

## Risks / Trade-offs

- [A huge candidate list is pushed on the UI thread when the prompt opens] → fine for commands and buffers; file lists will need the injector fed from a service, left to `find-file`.
- [The 10 ms tick blocks the UI thread up to 10 ms per key on big lists] → bounded and only while matching; drop to 0 if it shows in the frame timings.
- [nucleo's tie order among equal scores may not be the given order] → specs only promise best first and the given order for empty text; tests use candidates with distinct matches.
