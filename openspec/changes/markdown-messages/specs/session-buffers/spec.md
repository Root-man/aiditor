## MODIFIED Requirements

### Requirement: Streaming replies and turn ends
Each text agent message chunk SHALL be appended above the prompt line, consecutive chunks joining into one block, and each block separated from the next by a blank line. Each text agent thought chunk SHALL be appended the same way into a thought block; a message chunk after a thought chunk, or a thought chunk after a message chunk, SHALL start a new block. When the `session/prompt` answer arrives the turn SHALL end; a stop reason other than `end_turn` SHALL add the line `Turn ended: <reason>` (`max_tokens`, `max_turn_requests`, `refusal`, `cancelled`), and an error answer the line `Error: <message>`. Other updates SHALL stay kept and unrendered.

#### Scenario: A streamed reply
- **WHEN** `hello` is sent and the agent streams `Hi` then ` there` and ends the turn with `end_turn`
- **THEN** the buffer reads `> hello`, a blank line, `Hi there`, a blank line, then the prompt line, and `RET` can send again

#### Scenario: A thought between message chunks
- **WHEN** the agent streams the message chunk `Hi`, the thought chunk `hmm`, then the message chunk `there`
- **THEN** the transcript holds the blocks `Hi`, `hmm` and `there`, each followed by a blank line

#### Scenario: A refusal
- **WHEN** the turn ends with `refusal`
- **THEN** the line `Turn ended: refusal` appears above the prompt line

#### Scenario: An error answer
- **WHEN** the agent answers the prompt with an error `Internal error`
- **THEN** the line `Error: Internal error` appears above the prompt line and the turn ends

## ADDED Requirements

### Requirement: Messages render as markdown
Each agent message block, thought block and echoed prompt's text SHALL be rendered as markdown with text properties while its markup stays in the text: ATX and setext headings in the bold `text.title` colour, emphasis italic, strong emphasis bold, inline code in the `text.literal` colour on the `code_block` background, and link text and autolinks underlined in the `text.uri` colour. Heading markers, emphasis delimiters, code span backticks and a link's brackets and destination SHALL be invisible with no placeholder. Every line of a fenced or indented code block SHALL carry the `code_block` background decoration, and a code fence's text SHALL be invisible, its newline kept. A thought block SHALL be in the `dim` colour wherever no other colour applies. `Turn ended` and `Error` lines SHALL stay plain.

#### Scenario: Hidden markup
- **WHEN** the agent streams `# Title\n\nSome **bold**, *it*, `code` and [docs](https://x.dev).` and a frame is built
- **THEN** the lines read `Title`, an empty line, and `Some bold, it, code and docs.`, with `Title` bold, `bold` bold, `it` italic, `code` and `docs` coloured

#### Scenario: A code block
- **WHEN** the agent streams "```rust\nfn main() {}\n```" and a frame is built
- **THEN** the three lines carry the code block background and read empty, `fn main() {}` and empty

#### Scenario: A dim thought
- **WHEN** the agent streams the thought chunk `thinking` and a frame is built
- **THEN** `thinking` is drawn in the `dim` colour

#### Scenario: The echoed prompt
- **WHEN** `**hi** RET` is sent
- **THEN** the echo reads `> hi` with `hi` bold

### Requirement: Markup stays in the text
Rendering SHALL NOT change the buffer's text: search SHALL match hidden markup and copying or killing a rendered message SHALL yield its markdown.

#### Scenario: Swiper finds hidden markup
- **WHEN** a reply `**bold**` is rendered and swiper searches for `**b`
- **THEN** it matches the reply's line

#### Scenario: Copy yields markdown
- **WHEN** a rendered reply `**bold**` is marked whole and `M-w` is typed
- **THEN** the kill ring's top is `**bold**`

### Requirement: Streamed messages re-render once per frame
A streamed chunk's text SHALL be inserted at once, and the block being streamed SHALL be re-rendered when the next frame is built, once however many chunks arrived since the last frame. Only that block SHALL be re-rendered; earlier blocks SHALL keep their properties. A block that stops streaming SHALL be rendered before the next block starts.

#### Scenario: Chunks wait for the frame
- **WHEN** the chunks `**a` and `b**` arrive and no frame is built yet
- **THEN** the text reads `**ab**` and the block has no properties yet; after a frame is built `ab` is bold and the `**` hidden

#### Scenario: Earlier blocks are untouched
- **WHEN** a reply was rendered and another streams after the next prompt
- **THEN** building frames leaves the first reply's properties as they were
