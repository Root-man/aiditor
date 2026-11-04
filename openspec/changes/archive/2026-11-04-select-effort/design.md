# Design

## Context

See proposal.md. `agent_select_model` finds the model option with `select_option`, asks with `read_string`, and sends through `AgentOp::SetConfigOption`; `model_note` and `model_at` give the annotation and preselection.

## Goals / Non-Goals

**Goals:** One path for model and effort selection.

**Non-Goals:** A key binding; an effort option in the fake agent (the sending path is already covered live by the model's services test).

## Decisions

- `agent_select_model` becomes `agent_select(args, category)`, with the command name, the prompt and the words in its messages derived from the category (`model`, `effort`); `session_models`, `model_note` and `model_at` take the category too.
- The effort category is `thought_level`, as the mode line already reads it.
- The options are read when the command runs, so values changed by a model switch need no extra handling.

## Risks / Trade-offs

- Two values sharing a display name → only the first is reachable, as for the model.
