# Spec Delta

## Purpose

Colouring source code by its syntax: choosing a buffer's language from its file, keeping the parse current through edits, and drawing the visible text in the theme's syntax colours.

## ADDED Requirements

### Requirement: Language from the file extension
A buffer visiting a file SHALL be highlighted as JavaScript for `.js`, `.mjs`, `.cjs` and `.jsx`, as TypeScript for `.ts`, `.mts` and `.cts`, and as TSX for `.tsx`. Any other file, and a buffer with no file, SHALL be drawn without syntax colours.

#### Scenario: TypeScript file
- **WHEN** the user visits `a.ts` holding `const x: number = 1;`
- **THEN** `const` is drawn in the palette's `keyword` colour and `number` in its `type.builtin` or `type` colour

#### Scenario: Plain text
- **WHEN** the user visits `a.txt` holding `const x = 1;`
- **THEN** every char is drawn in the default face

#### Scenario: JSX in a TSX file
- **WHEN** the user visits `a.tsx` holding `const a = <div />;`
- **THEN** `div` is drawn in the palette's `tag` colour

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
