## MODIFIED Requirements

### Requirement: Language from the file extension
A buffer visiting a file SHALL be highlighted as JavaScript for `.js`, `.mjs`, `.cjs` and `.jsx`, as TypeScript for `.ts`, `.mts` and `.cts`, as TSX for `.tsx`, as Rust for `.rs`, as Elixir for `.ex` and `.exs`, as HEEx for `.heex`, and as YAML for `.yml` and `.yaml`. Any other file, and a buffer with no file, SHALL be drawn without syntax colours.

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
