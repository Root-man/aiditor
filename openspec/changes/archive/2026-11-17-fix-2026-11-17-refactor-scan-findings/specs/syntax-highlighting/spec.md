## MODIFIED Requirements

### Requirement: Markdown inline and fenced-language highlighting
Markdown file buffers SHALL highlight emphasis, strong emphasis, code spans and links with their inline syntax faces, leaving the markup visible. A fence whose info string starts with a highlighted language's full name (`rust`, `elixir`, `heex`, `javascript`, `typescript`, `tsx`, `yaml`, `toml`) or with one of the file extensions that language is chosen by (`rs`, `ex`, `exs`, `js`, `mjs`, `cjs`, `jsx`, `ts`, `mts`, `cts`, `yml`) SHALL highlight its content with that language's syntax faces. Uncaptured content in recognized fences SHALL use the default face; unrecognized and unlabeled fences SHALL retain the plain literal face. Fences SHALL remain in the literal face. Inline parsing SHALL preserve host context such as headings, tables, lists and block quotes.

#### Scenario: Inline syntax
- **GIVEN** a Markdown file with `*emphasis*`, `**strong**`, a code span and `[docs](https://example.com)`
- **WHEN** the buffer is drawn
- **THEN** emphasis is italic, strong emphasis is bold, the code span uses `text.literal`, and the link uses `text.reference` and `text.uri`, with all markup visible

#### Scenario: Recognized fence languages
- **GIVEN** Markdown fences labeled `rust`, `elixir`, `js`, `ts`, `yaml` and `toml`
- **WHEN** the buffer is drawn
- **THEN** the content has the same syntax faces as a standalone file of its language and delimiters retain `text.literal`

#### Scenario: Full names and other extensions
- **GIVEN** Markdown fences labeled `typescript`, `javascript`, `tsx`, `rs` and `yml`
- **WHEN** the buffer is drawn
- **THEN** each content has the syntax faces of TypeScript, JavaScript, TSX, Rust and YAML respectively

#### Scenario: Unknown or unlabeled fence
- **GIVEN** a Markdown fence labeled `unknown` or with no language
- **WHEN** the buffer is drawn
- **THEN** its content and delimiters use `text.literal`

#### Scenario: Incremental changes and undo
- **GIVEN** a Markdown file with inline markup and a recognized code fence
- **WHEN** text before or inside an injection, its delimiters or its language label is edited, undone, redone or reverted
- **THEN** the next display reflects the current inline and fenced syntax, including multibyte text and changed injection boundaries

#### Scenario: Visible portion of an injection
- **GIVEN** a Markdown fence containing a multiline string beginning above the window
- **WHEN** only a middle portion is visible
- **THEN** the visible text retains its string face and only the requested visible ranges are highlighted
