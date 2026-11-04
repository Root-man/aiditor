# Design

## Context

Session text is rendered by `markdown::render` (`crates/aiditor-core/src/markdown.rs`): `inline_link` labels and `uri_autolink`s get the `link` face, a link's brackets and destination are hidden, and file words accepted by `is_file` get the same face. `RET` is `agent_send_input` (`crates/aiditor-core/src/agents.rs`), which tries, in order: a permission option, a chip, `link_at_point` (file links), a tool call header, then sending. `link_at_point` already gates on "point has the `link` face and is above the prompt", takes the point's source line, and finds the word around point with `markdown::words`.

The core never touches the OS: it pushes `Effect`s and the frontend acts. `Effect::OpenWithSystem(PathBuf)` already exists for image chips and maps to GPUI's `cx.open_with_system`. GPUI also has `cx.open_url(&str)` (macOS `NSWorkspace`, Linux `xdg-open`/portal).

## Goals / Non-Goals

**Goals:** `RET` opens web links the buffer already draws, plus bare URLs; agent output can't open anything but `http(s)`.

**Non-Goals:** URLs in non-session buffers (file, terminal, `*Messages*`); a configurable browser; markdown links to files (`[a](src/a.rs)`) visiting the file; reference-style links (`[a][1]`).

## Decisions

- **New `Effect::OpenUrl(String)`, not reusing `OpenWithSystem`.** `open_with_system` takes a path; passing a URL as a `Path` works on macOS by accident and not reliably on Linux. The frontend arm is one line: `cx.open_url(&url)`. The harness passes it through as it does `OpenWithSystem`; the headless binary and service test supports ignore it.
- **Find the URL by re-parsing the point's line.** Add `markdown::url_at(line: &str, at: usize) -> Option<String>` beside `words`: parse the line with the same `MarkdownParser`, return the destination of the `inline_link` or the inner text of the `uri_autolink` whose range holds `at`, else the bare URL from `web_urls` holding `at`. Reusing the parser keeps "what is drawn as a link" and "what `RET` opens" the same code path; a hand-rolled `](` scanner would disagree with the renderer on nesting and escapes.
  - shortcut: a single line is parsed, so a link whose label wraps across source lines opens only from the line holding its destination; upgrade by parsing the whole block if it shows up.
- **Gate shared with file links.** Factor the "point has the `link` face, above the prompt" test out of `link_at_point` so `url_at_point` uses it too, then call `url_at_point` right after chips. Bare URLs come first so a URL like `https://x.dev/src/a.rs` never resolves as a file.
- **Scheme check in the core.** `url_at_point` returns any link destination; `agent_send_input` opens it only if it starts with `http://` or `https://` (ASCII case-insensitive), else returns `Err("Not a web link: <url>")`, which the command loop echoes. This is the trust boundary: agent output is untrusted and `open_url` would launch `file:` or app schemes.
- **Bare URL faces in `render`.** A separate `web_urls` scanner, not `words` (which cuts at `(`), finds bare URLs for every caller, skipping code spans and blocks; the `is_file` closure stays file-only. No hidden markup, so search and copy are unchanged.

## Risks / Trade-offs

- [A bare URL ending in `)` is ambiguous with prose parentheses] → `web_urls` follows GFM's extended autolinks: a trailing `)` is dropped only when unmatched inside the URL, so `(…/Emacs_(text_editor))` keeps one. Code spans and blocks get no URL links, unlike file paths.
- [Opening a URL is outward-facing and can't be undone] → Only on an explicit `RET` on a drawn link, only `http(s)`.
- [`RET` on a link while composing above the prompt changes behaviour from "send"] → Same as file links today; the prompt area itself is excluded.
