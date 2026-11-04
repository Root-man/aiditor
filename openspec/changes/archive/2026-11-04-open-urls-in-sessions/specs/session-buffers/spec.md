# Spec Delta

## ADDED Requirements

### Requirement: Bare URLs are links
Wherever session text is rendered as markdown, outside code spans and code blocks, a run starting with `http://` or `https://` at the start of a word SHALL be drawn in the `link` face, underlined, as autolinks are. The run SHALL end at whitespace, quotes, backticks, angle or square brackets or braces, without trailing `.`, `,`, `:`, `;`, `!`, `?`, `*`, `_` or `~`, and without a trailing `)` that has no `(` to match it inside the URL. Its text SHALL stay visible and unchanged.

#### Scenario: A bare URL in an agent message
- **WHEN** an agent message says `See https://x.dev/docs.`
- **THEN** `https://x.dev/docs` is in the `link` face and the trailing `.` is not

#### Scenario: Another scheme
- **WHEN** an agent message says `ftp://x.dev/file`
- **THEN** it stays plain text

#### Scenario: Parentheses
- **WHEN** an agent message says `(https://en.wikipedia.org/wiki/Emacs_(text_editor))`
- **THEN** `https://en.wikipedia.org/wiki/Emacs_(text_editor)` is in the `link` face and the outer parentheses are not

#### Scenario: Inline code
- **WHEN** an agent message says `` `https://example.org/not-a-link` ``
- **THEN** the URL is not in the `link` face and `RET` on it does not open it

### Requirement: RET on a URL opens it in the browser
`RET` in a session buffer with point on a URL link above the prompt line SHALL open the URL in the system's default browser and send nothing. A URL link is the visible text of a markdown inline link whose destination is the URL, an autolink, or a bare URL; the URL opened is the link's destination, the autolink's text without its `<` and `>`, or the bare word. Only `http` and `https` URLs SHALL open; `RET` on a link to any other scheme SHALL echo `Not a web link: <url>` and send nothing. This check SHALL come after chips and before file links, tool call headers and sending, and SHALL work while a turn runs and after the session ended.

#### Scenario: A markdown link
- **WHEN** an agent message reads `See [docs](https://x.dev).`, point is on `docs` and `RET` is typed
- **THEN** `https://x.dev` opens in the default browser and the prompt is not sent

#### Scenario: An autolink
- **WHEN** point is on the autolink `<https://x.dev/a>` and `RET` is typed
- **THEN** `https://x.dev/a` opens in the default browser

#### Scenario: A bare URL while a turn runs
- **WHEN** a turn runs, point is on the bare URL `https://x.dev/b` in an earlier message and `RET` is typed
- **THEN** `https://x.dev/b` opens in the default browser and nothing echoes `is busy`

#### Scenario: Another scheme
- **WHEN** point is on the text of `[run](file:///bin/sh)` and `RET` is typed
- **THEN** nothing opens and the echo area shows `Not a web link: file:///bin/sh`

#### Scenario: In the prompt area
- **WHEN** the prompt area holds `https://x.dev` with point on it and `RET` is typed
- **THEN** the prompt is sent as before
