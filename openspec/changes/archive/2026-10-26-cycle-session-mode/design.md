# Design

## Context

Since #305 the core reads the mode from the first config option of category `mode`, sends `AgentOp::SetConfigOption` for `default_mode` and shows the mode from `Msg::AgentConfigured`. Only the mode string is kept (on the buffer); the options themselves are only in the `session/new` answer, never updated.

## Goals / Non-Goals

**Goals:** cycle with the existing op and message; name the value sent on a refusal.

**Non-Goals:** a mode picker, cycling other config options, an echo of the new mode (the mode line shows it).

## Decisions

- **Keep the latest options in the session.** `config_option_update` and the set answer replace `config_options` on the stored `session/new` answer, so the cycle reads values and current value from one place with the existing `mode_option` and `values` helpers. Alternative: re-read only the buffer's mode string; it lacks the values when the agent changes them.
- **Remember the value last sent** (`Session::mode_sent`) for the refusal note, replacing #305's shortcut that always named `default_mode`.
- **Cycle from the reported current value, not one in flight.** Typing `S-TAB` twice before the answer sends the same value twice. Local agents answer at once; tracking pending values would need clearing rules for interleaved agent updates. Marked as a shortcut in the code.
- **Readiness messages match `agent-send-input`'s**, checked before the mode option, ended first.

## Risks / Trade-offs

- [Rapid presses before an answer don't advance twice] → shortcut comment; revisit if a slow agent shows it.
