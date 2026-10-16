# Spike: GPUI drawing, raw keys, IME, Tokio and CI

Oct 16, 2026 · issue [#7](https://github.com/Root-man/aiditor/issues/7) · GPUI 0.2.2

The throwaway example is `crates/aiditor/examples/gpui_spike.rs` on the `spike/gpui` branch. It draws styled lines with a cursor from its own text model, logs every key-down and input-handler call, and drains a channel fed by a Tokio task. It was run on macOS (US layout) and on Linux (KDE, Wayland). Run it with `cargo run -p aiditor --example gpui_spike`; F1 switches Option between Meta and characters, F2 (or `SPIKE_STRESS=1` at start) scrolls 500 generated lines every frame.

## Summary

| Question | Answer | Recommendation |
| --- | --- | --- |
| Drawing from our own model at frame rate | Yes, on both platforms | A custom `Element` that shapes each visible line with `shape_line` |
| Raw key-down events before the keymap | Yes, with all modifiers | Bind nothing in GPUI's keymap; handle `on_key_down` on the editor view |
| Option as Meta vs Option for characters | Both work; the choice is made per key in our handler | A setting, default Option as Meta; no per-side choice |
| IME and marked text | Works through `EntityInputHandler` | Typed text enters the core through the input handler, not as key events |
| Tokio to a GPUI foreground task | Yes | `tokio::sync::mpsc` drained by a `cx.spawn` task |
| Screenshot tests in CI | Not pursued | Frontend rendering is checked by hand; no screenshot tests |

## Drawing

The example's `BufferElement` shapes each visible line with `window.text_system().shape_line(text, size, &runs, None)`, one `TextRun` per span (colour, background, underline), then paints backgrounds, text and a cursor quad. The same shaped lines answer the IME's position queries (`x_for_index`). This is the pattern of GPUI's `examples/input.rs` and of Zed's editor element.

In the stress view every visible line changes on every frame:

- macOS: holds 120 fps (debug build), the display's maximum.
- Linux: 136 fps (release build), with about 0.5 ms of shaping for 41 lines.

A real display builder only reshapes changed lines, so it has more headroom than this. `request_animation_frame` must be called during a draw (from `render` or an element): calling it from a key handler panics.

## Keys

GPUI dispatches a key in this order: `cx.intercept_keystrokes` hooks, then its keymap and actions, then `on_key_down` listeners (capture, then bubble). With no bindings in the keymap, every key reaches `on_key_down` unchanged. The log confirmed it for plain and modified letters, arrows, F-keys, Cmd-C, Cmd-S, Cmd-Q and Cmd-arrows.

A `Keystroke` has `key` (the key's own character, `"s"` for Option-s), `key_char` (what it would type, `Some("ß")`, or `None` with Ctrl or Cmd) and `Modifiers { control, alt, shift, platform, function }`. Shift is reported for Cmd-Shift-C.

The handler decides, per key, whether the key is a command or typing:

- **Consume** (stop propagation, send to the key trie): Ctrl, Cmd or Fn held; Option held in Meta mode; `key_char` is `None`; Enter, Tab, Backspace, Delete, Escape.
- **Propagate** everything else. GPUI then hands the key to the platform input handler, which inserts the text.

Printable keys must propagate: if the handler consumes them, the platform's text input never sees them and dead keys stop working. So the core gets two kinds of input: `Msg::Key` for commands and an insert message for typed text, which the self-insert command, keyboard macros and the test harness must all handle.

On macOS, F-keys and some other keys arrive as key equivalents and reach the platform's text input before our handler. GPUI calls our handler from inside an Objective-C callback there, so a panic in key handling aborts the process instead of unwinding. The F2 bug showed this. The core must not panic on input, or the frontend must catch it around the call to the core so unsaved buffers survive.

## Option as Meta and dead keys (macOS)

With Option for characters, the US layout gives:

| Keys | Key-down event | Input handler calls | Result |
| --- | --- | --- | --- |
| Option-e, e | `key "e"`, `alt`, `key_char "´\u{a0}"`, propagated | `replace_and_mark "´"`, then `replace_text_in_range "é"` | é |
| Option-u, u | `key "u"`, `alt`, `key_char "¨\u{a0}"`, propagated | `replace_and_mark "¨"`, then `replace_text_in_range "ü"` | ü |
| Option-s | `key "s"`, `alt`, `key_char "ß"`, propagated | `replace_text_in_range "ß"` | ß |

A dead key is visible at the event level: its `key_char` is the accent followed by a no-break space. With Option as Meta, the handler consumes Option-e as `M-e`, and the next `e` types a plain e; dead keys are off.

`Modifiers` has one `alt` flag, so left and right Option cannot be told apart without patching GPUI. Recommendation for the key-handling decision: one setting, Option as Meta by default (an Emacs user's expectation), Option for characters as the alternative.

## IME

Composition reaches us through `EntityInputHandler`, registered each frame with `window.handle_input(&focus, ElementInputHandler::new(bounds, view), cx)`:

- `replace_and_mark_text_in_range` sets the marked text (shown underlined); `replace_text_in_range` commits it; `unmark_text` drops the mark.
- `marked_text_range`, `selected_text_range`, `text_for_range` and `bounds_for_range` are queried synchronously, which is why the core runs on the UI thread.
- All ranges are in **UTF-16 code units**; the buffer must convert to and from its own offsets.

Only dead keys were tried. Japanese and other composing input methods are out of scope. Linux has IME backends (Wayland text-input-v3, X11 XIM) but it was not tried there.

## Tokio and the foreground task

A Tokio runtime started before `Application::run` spawns a task that sends into a bounded `tokio::sync::mpsc` channel every 250 ms. A `cx.spawn` task on GPUI's foreground executor awaits `rx.recv()` and updates the view. Tokio's channel needs no Tokio context on the receiving side. Messages arrived from the `tokio-rt-worker` thread with 15–40 µs latency. Backpressure through a full bounded channel was not tested.

## Screenshot tests in CI

GPUI 0.2.2 cannot read back a frame, and its test platform draws nothing. A screenshot test would need a display server and software Vulkan on Linux runners and a Screen Recording permission on macOS runners. The cost is not worth it for a thin frontend: rendering is checked by hand, and the frontend's logic is tested through the core. GPUI's `TestAppContext` (feature `test-support`) can still drive key dispatch in tests if that is ever needed.

## Not checked

- Alt on Linux: the `key_char` reported for Alt+letter.
- Non-US macOS layouts, and Cmd bindings on layouts that switch with Cmd.
- Bounded-channel backpressure under load.
