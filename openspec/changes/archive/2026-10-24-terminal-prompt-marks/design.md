# Design

## Context

`Terminal::feed` hands each batch to alacritty's `Processor`, whose `osc_dispatch` sends OSC 133 and OSC 7 to `unhandled`. vte's low-level `Parser` is reachable as `alacritty_terminal::vte::Parser` and has `advance_until_terminated`, which stops right after the byte on which the `Perform` reports `terminated()`.

## Goals / Non-Goals

**Goals:** marks and cwd recorded in core `Terminal`, with accessors for later commands; no measurable cost on batches without OSC.

**Non-Goals:** commands or display that use the marks or cwd; shell scripts that emit them (users' shells or later issues); moving marks on column reflow.

## Decisions

- **A second parser that only splits.** A `Perform` (`Scan`) implements `osc_dispatch` and `csi_dispatch`/`esc_dispatch`, sets an event and `terminated()` for OSC 133, OSC 7, CSI 3 J and ESC c, and ignores everything else. `feed` loops: `n = scan.advance_until_terminated(&mut scan, rest)`, `processor.advance(term, &rest[..n])`, then applies the event against the grid as alacritty now sees it. Both parsers see the same bytes in the same chunks, so their states stay in step across batches. Alternative: fork alacritty's `Processor` handler or wrap `Term` in our own `Handler` — far more code, and alacritty's `Handler` has no hook for unknown OSC.
- **Absolute line = `history_size + cursor line`,** read on the main screen after the prefix is fed. It stays right as lines scroll into history and as rows shrink or grow (alacritty moves lines between screen and history). Column reflow can shift it; kept with a `shortcut:` comment, as the user chose.
- **Full scrollback.** After each chunk, if `history_size` equals the configured limit (10,000), marks are cleared and new ones skipped while it stays there. A `shortcut:` comment notes that a dropped-line counter would let them survive.
- **Clearing.** CSI 3 J on the main screen and ESC c drop all marks. On the alternate screen CSI 3 J is ignored, as alacritty clears no main-screen history there.
- **Hostname.** `libc::gethostname`, read when an OSC 7 with a non-empty, non-`localhost` host arrives. `libc` is already compiled through alacritty_terminal; std has no hostname call.
- **Percent-decoding** by hand over bytes into an `OsString` (unix), for both URL forms; invalid `%` escapes reject the OSC.

## Risks / Trade-offs

- [Second parse pass on every batch] → vte's ground state handles printable runs in bulk and `Scan` does nothing per char; the 64 KB benchmarks check it stays about as fast.
- [Reflow shifts marks] → accepted, `shortcut:` comment.
