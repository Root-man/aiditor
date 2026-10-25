# syntax-highlighting Specification

## Purpose
Colouring source code by its syntax: choosing a buffer's language from its file, keeping the parse current through edits, and drawing the visible text in the theme's syntax colours.

## Requirements

### Requirement: Language from the file extension
A buffer visiting a file SHALL be highlighted as JavaScript for `.js`, `.mjs`, `.cjs` and `.jsx`, as TypeScript for `.ts`, `.mts` and `.cts`, as TSX for `.tsx`, as Rust for `.rs`, as Elixir for `.ex` and `.exs`, as HEEx for `.heex`, as YAML for `.yml` and `.yaml`, as Markdown (its block grammar) for `.md` and `.markdown`, and as TOML for `.toml`. An EEx template (a file ending in `.eex`) SHALL be highlighted by the extension before `.eex`, as the file it generates. Any other file, and a buffer with no file, SHALL be drawn without syntax colours.

#### Scenario: TypeScript file
- **WHEN** the user visits `a.ts` holding `const x: number = 1;`
- **THEN** `const` is drawn in the palette's `keyword` colour and `number` in its `type.builtin` or `type` colour

#### Scenario: Plain text
- **WHEN** the user visits `a.txt` holding `const x = 1;`
- **THEN** every char is drawn in the default face

#### Scenario: JSX in a TSX file
- **WHEN** the user visits `a.tsx` holding `const a = <div />;`
- **THEN** `div` is drawn in the palette's `tag` colour

#### Scenario: Rust file
- **WHEN** the user visits `a.rs` holding `fn main() {}`
- **THEN** `fn` is drawn in the `keyword` colour and `main` in the `function` colour

#### Scenario: Elixir file
- **WHEN** the user visits `a.ex` holding `defmodule Foo do end`
- **THEN** `defmodule`, `do` and `end` are drawn in the `keyword` colour and `Foo` in the `module` colour

#### Scenario: HEEx file
- **WHEN** the user visits `a.heex` holding `<div class="a"></div>`
- **THEN** `div` is drawn in the `tag` colour, `class` in the `attribute` colour and `"a"` in the `string` colour

#### Scenario: YAML file
- **WHEN** the user visits `a.yaml` holding `a: true`
- **THEN** `a` is drawn in the `property` colour and `true` in the `boolean` colour

#### Scenario: EEx template of a HEEx file
- **WHEN** the user visits `a.html.heex.eex` holding `<div class="a"></div>`
- **THEN** it is highlighted as HEEx, `div` drawn in the `tag` colour

#### Scenario: EEx template with no highlighted inner language
- **WHEN** the user visits `a.eex` or `a.html.eex`
- **THEN** it is drawn without syntax colours

#### Scenario: Markdown file
- **WHEN** the user visits `a.md` holding `# Title`, a `- item` list line, a `> quote` line, a `---` break and a fenced code block
- **THEN** `#`, `-`, `>` and `---` are drawn in the `punctuation.special` colour, `Title` in the `text.title` colour and the code block, fences and content, in the `text.literal` colour

#### Scenario: TOML file
- **WHEN** the user visits `a.toml` holding `[package]`, `name = "a"` and `x.y = true`
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
The theme SHALL have a `syntax` table mapping capture names to colours. A capture with no entry SHALL use the entry of its longest dotted prefix that has one (`function.method` falls back to `function`), and none means the default colour. A user's `[theme.syntax]` entry SHALL override that entry only.

#### Scenario: User overrides one colour
- **WHEN** the user's settings set `[theme.syntax] keyword = 0xff0000`
- **THEN** keywords are drawn in 0xff0000 and strings keep the bundled colour

#### Scenario: Prefix fallback
- **WHEN** a capture is named `function.method` and the palette has only `function`
- **THEN** it is drawn in the `function` colour

### Requirement: Text properties draw over syntax colours
A face text property and the region SHALL draw over syntax colours where they set a colour.

#### Scenario: Region over a keyword
- **WHEN** the region covers a keyword
- **THEN** its background is the region colour and its foreground the keyword colour
