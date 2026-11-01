# syntax-highlighting Specification

## Purpose
Colouring source code by its syntax: choosing a buffer's language from its file, keeping the parse current through edits, and drawing the visible text in the theme's syntax colours.

## Requirements

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

### Requirement: Highlighting follows the text
Highlighting SHALL reflect the buffer's current text whenever it is drawn, after any edit, undo, redo or revert from disk.

#### Scenario: Typing makes a comment
- **WHEN** the user types `// ` at the start of a line of code in a JS buffer
- **THEN** the whole line is drawn in the `comment` colour

#### Scenario: Undo restores colours
- **WHEN** the user then undoes the typing
- **THEN** the line is drawn with the colours it had before

#### Scenario: Revert
- **WHEN** the file changes on disk and the unmodified buffer reverts to it
- **THEN** the buffer is drawn with the new text's colours

### Requirement: Only visible lines are highlighted
The display SHALL compute syntax colours only for the lines a window shows, so drawing cost does not grow with the length of the file.

#### Scenario: Scrolled window
- **WHEN** a window shows the last lines of a long JS file
- **THEN** they are drawn in their syntax colours

#### Scenario: Query limited to a range
- **WHEN** highlights are asked for the second of three lines
- **THEN** only that line's captures are returned

### Requirement: Theme syntax palette
The theme SHALL have a `syntax` table mapping capture names to faces, so a capture can be italic. A capture with no entry SHALL use the entry of its longest dotted prefix that has one (`function.method` falls back to `function`), and none means the default face. A user's `[themes.<name>.syntax]` entry SHALL override that entry only.

#### Scenario: User overrides one colour
- **WHEN** the user's settings set `[themes.default.syntax] keyword = { fg = 0xff0000 }`
- **THEN** keywords are drawn in 0xff0000 and strings keep the bundled colour

#### Scenario: Prefix fallback
- **WHEN** a capture is named `function.method` and the palette has only `function`
- **THEN** it is drawn in the `function` face

#### Scenario: Italic comments
- **WHEN** the theme sets `comment = { fg = 0x928374, italic = true }`
- **THEN** comments are drawn italic in that colour

### Requirement: Text properties draw over syntax colours
A face text property and the region SHALL draw over syntax colours where they set a colour.

#### Scenario: Region over a keyword
- **WHEN** the region covers a keyword
- **THEN** its background is the region colour and its foreground the keyword colour

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

### Requirement: Local references retain definition faces
Languages with a locals query SHALL draw a resolved local reference with its definition's syntax face, including dotted-name fallback and theme overrides. Rust SHALL support function parameters and simple local bindings; JavaScript, TypeScript and TSX SHALL use their bundled locals queries. Scope nesting, shadowing, definition-value ranges and scope inheritance settings SHALL control resolution. Highlight query predicates about `local` SHALL be respected. A definition outside the visible range SHALL still determine a visible reference's face. Existing Markdown language fences SHALL have the same local highlighting as standalone files, with separate bindings per fence. Edits, undo and redo SHALL refresh local highlighting on the next draw.

#### Scenario: Rust parameter uses
- **GIVEN** a Rust function `fn f(value: i32) { consume(value); consume(value); }`
- **WHEN** the buffer is drawn
- **THEN** the parameter and both uses have the `variable.parameter` face

#### Scenario: Shadowing and scope exit
- **GIVEN** a parameter used before, inside and after a block that defines a local of the same name
- **WHEN** the buffer is drawn
- **THEN** uses before the local's initializer ends retain the parameter face, subsequent uses inside the block have the local definition face, and uses after the block have the parameter face

#### Scenario: Local shadows a builtin
- **GIVEN** JavaScript declares a local called `console` or `require`
- **WHEN** its references are drawn
- **THEN** their faces follow the local definition and query patterns restricted to nonlocals apply to unresolved builtins

#### Scenario: Offscreen definition and theme change
- **GIVEN** a Rust parameter is above the visible range
- **WHEN** its uses are drawn and the theme's parameter face changes
- **THEN** only requested ranges are returned and visible uses take the current parameter face

#### Scenario: Separate Markdown fences
- **GIVEN** one Rust fence defines and uses a parameter and another fence uses the same name without a definition
- **WHEN** the file is drawn
- **THEN** the first fence uses the parameter face and the second uses its ordinary syntax face

#### Scenario: Editing bindings
- **GIVEN** a function parameter and a reference of the same name
- **WHEN** the parameter is renamed, then the edit is undone and redone
- **THEN** each draw uses the bindings in the current text
