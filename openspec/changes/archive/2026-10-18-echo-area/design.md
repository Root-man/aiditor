# Design

## Context

`Core` keeps the message line as `message: Option<String>`. `Ctx::key` and `Ctx::click` clear it first; command, key and load errors set it. The frontend draws `core.message()` as the echo line and `Core::frame()` builds `FrameDisplay` from the split tree. See proposal.md for why.

## Goals / Non-Goals

**Goals:** the frame display, and so its snapshots, carries the echo area text.

**Non-Goals:** moving the key hint into the frame display, a Messages buffer log, message timeouts, or having the frontend read the echo line from `FrameDisplay` (it builds the frame inside the windows element's prepaint, while the echo line is laid out in `render`).

## Decisions

- **A `message: Option<String>` field on `FrameDisplay`**, cloned from `Core::message` in `Core::frame()`. Alternatives: a separate `EchoArea` struct (no other field to hold yet) or a snapshot helper in the harness (would not make the display model the source of what is shown).
- **No change to clearing**: the code already clears on keys and clicks; the new spec pins that down with tests.

## Risks / Trade-offs

- [The frontend still reads `core.message()` directly] → Both read the same field in the same core state, so they cannot disagree within a frame.
