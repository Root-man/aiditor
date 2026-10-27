## ADDED Requirements

### Requirement: Transcript colours follow the theme
When reloaded settings change the theme, every session buffer's text properties and line decorations SHALL change each colour the old theme gave as `dim`, `code_block`, or the `text.title`, `text.literal` or `text.uri` syntax colour to the new theme's colour for the same name. This SHALL apply to completed blocks, the block being streamed, tool calls, the plan and permission requests alike. A colour the old theme did not define for a name SHALL stay as it was. Reloaded settings that leave the theme unchanged SHALL leave session buffers untouched.

#### Scenario: A completed reply after a theme change
- **GIVEN** a session holds a rendered reply with inline code and a thought
- **WHEN** the user's settings change `code_block` and `dim` and are reloaded
- **THEN** the inline code's background and the thought's colour are the new theme's `code_block` and `dim`

#### Scenario: A rule beside a tool call
- **GIVEN** a session shows a tool call
- **WHEN** the reloaded theme changes `dim`
- **THEN** the tool call's rule decoration is in the new `dim`
