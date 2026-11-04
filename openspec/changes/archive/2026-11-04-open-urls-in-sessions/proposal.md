# Proposal

## Why

Agents often answer with web links (docs, issues, PRs), and the session buffer already draws markdown links and `<…>` autolinks in the `link` face, but `RET` on one does nothing useful: it falls through to sending the prompt. Opening a link means copying the URL by hand.

## What Changes

- `RET` in a session buffer with point on a URL link above the prompt line opens the URL in the system's default browser and sends nothing. A URL link is a markdown inline link whose destination is an `http` or `https` URL, a `<http…>` autolink, or a bare `http://` / `https://` word.
- Bare `http://` and `https://` words in rendered markdown (agent messages, thoughts, the echoed prompt, tool call text) are drawn in the `link` face, underlined, as autolinks already are, so every URL `RET` opens looks like a link.
- Only `http` and `https` URLs open; any other scheme (`file:`, `javascript:`, app schemes) stays inert, so agent output can't launch local apps or files.
- A new core effect asks the frontend to open a URL; the frontend uses GPUI's `open_url`, which calls the OS (`NSWorkspace` on macOS, `xdg-open`/portal on Linux).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `session-buffers`: bare URLs are drawn as links, and `RET` on a URL link opens it in the browser.

## Impact

- `crates/aiditor-core/src/markdown.rs`: bare URL words get the link face.
- `crates/aiditor-core/src/agents.rs`: `agent_send_input` checks for a URL at point before file links, headers and sending.
- `crates/aiditor-core/src/lib.rs`: new `Effect::OpenUrl(String)`; `harness.rs`, the headless binary and the service test supports pass it through or ignore it like `OpenWithSystem`.
- `crates/aiditor/src/editor.rs`: handles the effect with `cx.open_url`.
- `docs/editor.md`: the session buffer's link notes mention URLs.
- No new dependencies.
