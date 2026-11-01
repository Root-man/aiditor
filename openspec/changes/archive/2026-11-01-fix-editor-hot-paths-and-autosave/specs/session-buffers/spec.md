# Spec Delta

## MODIFIED Requirements

### Requirement: Streamed messages re-render once per frame
A streamed chunk's text SHALL be inserted at once. If chunks have arrived since the block's previous rendering, a frame that shows the session buffer SHALL re-render that block once, regardless of the number of chunks. A session is shown when any pane in the active workspace displays it, including an inactive pane or the origin pane underneath an open minibuffer. Frames that do not show the session SHALL defer its streamed-message rendering while retaining the pending text and formatting work. The first frame that shows it again SHALL include formatting for all accumulated chunks, even if no further chunk arrives. Only the block being streamed SHALL be re-rendered; earlier blocks SHALL keep their properties. A block that stops streaming SHALL be rendered before the next block starts, including while the session is hidden.

#### Scenario: Chunks wait for the frame
- **GIVEN** a shown session buffer
- **WHEN** the chunks `**a` and `b**` arrive and no frame is built yet
- **THEN** the text reads `**ab**` and the block has no properties yet; after a frame is built `ab` is bold and the `**` hidden

#### Scenario: Earlier blocks are untouched
- **GIVEN** a reply was rendered
- **WHEN** another reply streams after the next prompt and frames are built
- **THEN** the first reply's properties remain as they were

#### Scenario: Hidden streaming text catches up on display
- **GIVEN** a session is hidden behind another buffer or belongs to an inactive workspace
- **WHEN** chunks `**a` and `b**` arrive while frames show other buffers
- **THEN** its stored text reads `**ab**`, rendering remains pending, and the first frame showing that session displays `ab` in bold with the `**` hidden without needing another chunk

#### Scenario: An inactive pane is still shown
- **GIVEN** a session buffer is displayed in an inactive split pane of the active workspace
- **WHEN** streamed chunks arrive and the next frame is built
- **THEN** that pane shows their updated formatting

#### Scenario: A prompt leaves its origin pane shown
- **GIVEN** a minibuffer prompt is active above its session origin pane
- **WHEN** the session receives streamed chunks and the next frame is built
- **THEN** the origin pane shows their updated formatting

#### Scenario: A hidden block finishes before another begins
- **GIVEN** a hidden session has an unrendered message block
- **WHEN** a new message kind, tool call, plan, permission prompt or submitted prompt ends that block
- **THEN** the completed block receives its final formatting before the next block begins and retains that formatting when later shown

#### Scenario: A hidden session is killed
- **GIVEN** a hidden session has pending streamed-message formatting
- **WHEN** its buffer is killed and another frame is built
- **THEN** remaining buffers render normally and the killed session's pending work is discarded
