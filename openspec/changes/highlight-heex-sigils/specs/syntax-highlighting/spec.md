# Spec Delta

## ADDED Requirements

### Requirement: HEEx and embedded Elixir highlighting
Elixir buffers SHALL highlight uppercase `~H` sigil content as HEEx, including tags, attributes, components and comments. HEEx SHALL highlight brace expressions and EEx directive contents as Elixir, both inside sigils and in standalone HEEx files. Split directives belonging to one template SHALL share Elixir syntax context. Distinct sigils SHALL keep their syntax context separate. Uncaptured template text SHALL use the default face; sigil and expression delimiters SHALL retain their host faces. Other sigils SHALL retain their existing highlighting. Existing Markdown Elixir fences SHALL support the same nested highlighting while preserving Markdown continuation markers. Nested highlighting SHALL follow edits, undo, redo and reverts, use the current syntax palette and respect requested visible ranges.

#### Scenario: Phoenix template in an Elixir file
- **GIVEN** an `.ex` file with a `~H` template containing tags, attributes, a component, a comment, `{String.upcase("é")}` and `<%= if true do %>...<% end %>`
- **WHEN** the buffer is drawn
- **THEN** template nodes use HEEx faces, embedded modules, calls, strings and keywords use Elixir faces, plain template text uses the default face and delimiters keep their host faces

#### Scenario: Standalone template and separate sigils
- **GIVEN** a standalone `.heex` file or multiple `~H` sigils, including brace and EEx expressions
- **WHEN** the buffer is drawn
- **THEN** each template has embedded Elixir highlighting without borrowing syntax context from another sigil

#### Scenario: Other sigils and delimiters
- **GIVEN** uppercase `~H` templates with quoted or heredoc delimiters and ordinary `~s` and `~h` sigils
- **WHEN** the buffer is drawn
- **THEN** only `~H` content gains HEEx highlighting and sigil delimiters retain their Elixir faces

#### Scenario: Nested template in a Markdown block quote
- **GIVEN** an Elixir fence inside a Markdown block quote containing a `~H` template
- **WHEN** the buffer is drawn
- **THEN** the template and its Elixir expressions have their language faces and quote continuation markers retain Markdown faces

#### Scenario: Nested highlighting after edits
- **GIVEN** a template after multibyte text
- **WHEN** text before it, its content, sigil name or boundaries change, are undone, redone or reverted
- **THEN** the next draw uses the current text's nested language faces

#### Scenario: Visible expression and palette override
- **GIVEN** a template with a multiline Elixir expression starting above the window
- **WHEN** visible portions are highlighted with an overridden palette
- **THEN** only those portions are returned with the current Elixir faces and preceding syntax context preserved
