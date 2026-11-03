## ADDED Requirements

### Requirement: File links in agent messages
In an agent message, a whitespace-delimited `path`, `path:line` or `path:line:col` reference, with surrounding quotes, backticks, brackets, `*`, `,` and `;` and a trailing `.`, `:`, `!` or `?` left out, SHALL be drawn in the `link` face, underlined, when the path names a file in the project's file index: relative to any root, primary first, or absolute under a root. Any other reference SHALL stay as it is. Thoughts, the echoed prompt and tool call bodies get no links.

#### Scenario: An existing path
- **WHEN** an agent message says ``see `src/a.rs:12` for it`` and `src/a.rs` is in the primary root
- **THEN** `src/a.rs:12` is in the `link` face

#### Scenario: A missing path
- **WHEN** an agent message names `src/gone.rs` and no root holds it
- **THEN** it stays plain text

#### Scenario: Another root
- **WHEN** an agent message names `lib/b.rs`, which exists only under a second root
- **THEN** it is a link to that root's file

### Requirement: File links in tool call headers
The path a tool call's header shows, from its first location or its diff, SHALL be drawn in the `link` face, underlined, when that file is in the project's file index. A header showing a command, URL or title has no link.

#### Scenario: A read of an existing file
- **WHEN** a read call's location is a file under the primary root
- **THEN** the path on its header is in the `link` face

#### Scenario: A file outside the project
- **WHEN** a read call's location is `/etc/hosts`
- **THEN** the header shows it without the `link` face

### Requirement: RET on a link visits the file
`RET` in a session buffer with point on a link, above the prompt line, SHALL visit the linked file in another window, splitting a lone window, and put point at the start of the link's line: the reference's `line`, or a tool call location's `line`, both counted from 1, else the file's start. It SHALL send nothing. `RET` on a tool call's header outside its link still opens the full output.

#### Scenario: Visiting a referenced line
- **WHEN** point is on the link `src/a.rs:3` in an agent message and `RET` is typed
- **THEN** `src/a.rs` shows in another window with point on line 3 and the prompt is not sent

#### Scenario: Visiting a tool call's location
- **WHEN** point is on the path of a read call's header and `RET` is typed
- **THEN** that file shows in another window

#### Scenario: Elsewhere on a header
- **WHEN** point is on a header's verb and `RET` is typed
- **THEN** the call's full output opens as before
