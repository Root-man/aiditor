## Context

The core is a GPUI-free state machine (`handle(&mut Core, now, Msg) -> Effects`) and the frontend holds it. GPUI 0.2.2 offers `read_from_clipboard` and `write_to_clipboard` on the app context, synchronously. The bundled keymap has no `s-` bindings yet.

## Goals / Non-Goals

**Goals:** settle how kills and yanks meet the system clipboard and which mouse input the MVP handles, so #31 and #12 can be built.

**Non-Goals:** implementing either; drag selection, multi-click, PRIMARY, rich clipboard formats.

## Decisions

- **One ring, Emacs interprogram defaults.** Alternatives were a separate clipboard behind `s-c`/`s-v` (two clipboards to think about) and a one-way sync (pasting from other apps needs another key). The cost of the chosen one, a kill replacing the clipboard, is softened by saving foreign clipboard text to the ring first.
- **The core stays GPUI-free.** A kill returns a clipboard write as an effect; the frontend reads the clipboard and hands its current text to the core with every kill and yank, so a kill can save foreign text first. The core remembers the text it last wrote, so it can tell foreign text from its own. Tests use a fake clipboard.
- **`s-` keys are Emacs commands**, as in the Emacs Mac port, so `s-c` and `M-w` are the same thing.
- **Mouse: scroll and click only.** Scrolling follows Emacs's rule that point stays visible, so the display model's assumption holds and no "scrolled away" state is needed. Drag is cheap once click hit-testing exists, but it is not needed for the M0 gate.

## Risks / Trade-offs

- A kill overwrites the clipboard of other apps → saved onto the ring before the kill.
- Reading the clipboard on every yank → a local call, cheap; on Linux it can be slower under Wayland, acceptable for a key press.
