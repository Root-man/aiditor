## MODIFIED Requirements

### Requirement: Lines carry their decorations
A line decoration property SHALL apply to every display line its range touches. A decoration SHALL be a background color for the whole line, a rule beside it in a color, a 1 px border at the line's top or bottom in a color, or a 1 px horizontal rule through the middle of the line's empty columns in a color.

#### Scenario: A code block background
- **WHEN** lines 1 and 2 have a background decoration
- **THEN** those display lines carry it and lines 0 and 3 do not

#### Scenario: Borders and a horizontal rule
- **WHEN** line 1 has a top border decoration and line 2 a horizontal rule decoration
- **THEN** display line 1 carries the border and display line 2 the rule, and no other line carries either

### Requirement: Mode line segments use colour only for state
In the selected window, `**` and `●` SHALL be in `status-modified`; `%%`, `⊘`, `[deleted]` and `[conflict]` in `status-failed`; `UTF-8 CRLF` and `char` in `status-waiting`; `copy` in `status-info`; a running command in `status-running`; `exit <n>` in `status-failed`; the branch in `vcs-branch` and its ` ±` in `status-modified`; a file's or shell's directory, region size, position and `[unwatched]` in `mode-line-dim`; a file's name and a terminal's name in `mode-line-buffer-id`. A session's title SHALL use `mode-line-buffer-id`, its mode and lifecycle status their specified risk and status faces, and its context usage the face for how full the context is. A session's model and effort SHALL use their item-quality faces. The file's language name and version SHALL use an overridable `mode-line-<language>` face: Elixir and HEEx purple/rose (`mode-line-elixir`), JavaScript green (`mode-line-javascript`), TypeScript and TSX blue (`mode-line-typescript`), Rust rusty red (`mode-line-rust`), YAML yellow (`mode-line-yaml`), Markdown aqua (`mode-line-markdown`), and TOML orange (`mode-line-toml`). Fundamental SHALL retain `mode-line-dim`. This language coding and the session's item quality are exceptions to the state-only colour rule. Other text SHALL be in the `mode-line` face, and every segment's face SHALL sit over it. Inactive windows SHALL retain their shared inactive face.

#### Scenario: A modified file in the selected window
- **GIVEN** the selected window shows a file
- **WHEN** its buffer is modified
- **THEN** `●` has the `status-modified` foreground and the file's name is bold in the `mode-line` foreground

#### Scenario: Every supported language has a color
- **GIVEN** files in every supported highlighted language, with or without a pin
- **WHEN** each file is shown in the selected window
- **THEN** its entire language segment uses its language family's face, including any displayed version

#### Scenario: Inactive language colors
- **GIVEN** a file with a colored language segment
- **WHEN** its window becomes inactive
- **THEN** the language name and version use the shared `mode-line-inactive` face

#### Scenario: Theme override
- **GIVEN** settings override a language face's foreground
- **WHEN** settings are reloaded
- **THEN** the file's language name and version use the new foreground
