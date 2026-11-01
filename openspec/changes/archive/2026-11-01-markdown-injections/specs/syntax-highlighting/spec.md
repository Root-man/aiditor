# Spec Delta

## MODIFIED Requirements

### Requirement: Language from the file extension
A buffer visiting a file SHALL be highlighted as JavaScript for `.js`, `.mjs`, `.cjs` and `.jsx`, as TypeScript for `.ts`, `.mts` and `.cts`, as TSX for `.tsx`, as Rust for `.rs`, as Elixir for `.ex` and `.exs`, as HEEx for `.heex`, as YAML for `.yml` and `.yaml`, as Markdown (block and inline grammars) for `.md` and `.markdown`, and as TOML for `.toml`. An EEx template (a file ending in `.eex`) SHALL be highlighted by the extension before `.eex`, as the file it generates. Any other file, and a buffer with no file, SHALL be drawn without syntax colours.

#### Scenario: TypeScript file
- **GIVEN** the user visits `a.ts` holding `const x: number = 1;`
- **WHEN** the buffer is drawn
- **THEN** `const` is drawn in the palette's `keyword` colour and `number` in its `type.builtin` or `type` colour

#### Scenario: Plain text
- **GIVEN** the user visits `a.txt` holding `const x = 1;`
- **WHEN** the buffer is drawn
- **THEN** every char is drawn in the default face

#### Scenario: JSX in a TSX file
- **GIVEN** the user visits `a.tsx` holding `const a = <div />;`
- **WHEN** the buffer is drawn
- **THEN** `div` is drawn in the palette's `tag` colour

#### Scenario: Rust file
- **GIVEN** the user visits `a.rs` holding `fn main() {}`
- **WHEN** the buffer is drawn
- **THEN** `fn` is drawn in the `keyword` colour and `main` in the `function` colour

#### Scenario: Elixir file
- **GIVEN** the user visits `a.ex` holding `defmodule Foo do end`
- **WHEN** the buffer is drawn
- **THEN** `defmodule`, `do` and `end` are drawn in the `keyword` colour and `Foo` in the `module` colour

#### Scenario: HEEx file
- **GIVEN** the user visits `a.heex` holding `<div class="a"></div>`
- **WHEN** the buffer is drawn
- **THEN** `div` is drawn in the `tag` colour, `class` in the `attribute` colour and `"a"` in the `string` colour

#### Scenario: YAML file
- **GIVEN** the user visits `a.yaml` holding `a: true`
- **WHEN** the buffer is drawn
- **THEN** `a` is drawn in the `property` colour and `true` in the `boolean` colour

#### Scenario: EEx template of a HEEx file
- **GIVEN** the user visits `a.html.heex.eex` holding `<div class="a"></div>`
- **WHEN** the buffer is drawn
- **THEN** it is highlighted as HEEx, `div` drawn in the `tag` colour

#### Scenario: EEx template with no highlighted inner language
- **GIVEN** the user visits `a.eex` or `a.html.eex`
- **WHEN** the buffer is drawn
- **THEN** it is drawn without syntax colours

#### Scenario: Markdown file
- **GIVEN** the user visits `a.md` holding `# Title`, a `- item` list line, a `> quote` line, a `---` break and a fenced code block
- **WHEN** the buffer is drawn
- **THEN** `#`, `-`, `>` and `---` are drawn in the `punctuation.special` colour, `Title` in the `text.title` colour and the fence delimiters, info string and unrecognized or unlabeled code content in the `text.literal` colour

#### Scenario: TOML file
- **GIVEN** the user visits `a.toml` holding `[package]`, `name = "a"` and `x.y = true`
- **WHEN** the buffer is drawn
- **THEN** `package` is drawn in the `type` colour, `name`, `x` and `y` in the `property` colour, `"a"` in the `string` colour and `true` in the `boolean` colour

## ADDED Requirements

### Requirement: Markdown inline and fenced-language highlighting
Markdown file buffers SHALL highlight emphasis, strong emphasis, code spans and links with their inline syntax faces, leaving the markup visible. Fences whose info string starts with `rust`, `elixir`, `js`, `ts`, `yaml` or `toml` SHALL highlight their content with that language's syntax faces. Uncaptured content in recognized fences SHALL use the default face; unrecognized and unlabeled fences SHALL retain the plain literal face. Fences SHALL remain in the literal face. Inline parsing SHALL preserve host context such as headings, tables, lists and block quotes.

#### Scenario: Inline syntax
- **GIVEN** a Markdown file with `*emphasis*`, `**strong**`, a code span and `[docs](https://example.com)`
- **WHEN** the buffer is drawn
- **THEN** emphasis is italic, strong emphasis is bold, the code span uses `text.literal`, and the link uses `text.reference` and `text.uri`, with all markup visible

#### Scenario: Recognized fence languages
- **GIVEN** Markdown fences labeled `rust`, `elixir`, `js`, `ts`, `yaml` and `toml`
- **WHEN** the buffer is drawn
- **THEN** the content has the same syntax faces as a standalone file of its language and delimiters retain `text.literal`

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
