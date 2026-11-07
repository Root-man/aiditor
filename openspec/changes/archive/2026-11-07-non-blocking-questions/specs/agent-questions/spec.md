## ADDED Requirements

### Requirement: Non-blocking questions
A question whose request's `_meta.codex.delivery` is `async` SHALL be non-blocking. Its block SHALL show where it arrived, and the turn's later output SHALL go in below it; without a `toolCallId` it SHALL end the run of tool calls before it and join no group, so later calls group below it. It SHALL NOT move point; when the selected window shows the session, the echo line SHALL say `Question waiting: C-c C-q goes to it`. A non-blocking question of one field that is not a multi-select SHALL have the hint ` C-c C-q go to it   C-c C-k skip   an answer joins this turn`, its keys in `status-waiting` bold. It SHALL be answered, skipped and reached with `C-c C-q` like any question. When the turn ends, a non-blocking question still pending SHALL be answered `cancel` and its block SHALL become `○ withdrawn`; one the agent withdraws first SHALL close as any withdrawn request does.

#### Scenario: Output goes in below an open question
- **GIVEN** a running turn
- **WHEN** a non-blocking question arrives, then an agent message
- **THEN** the message shows below the question's block, the block is still pending, and point stays in the prompt area

#### Scenario: Tool calls after an open question
- **GIVEN** a running turn with a non-blocking question pending
- **WHEN** three tool calls arrive
- **THEN** their group's row shows below the question's block

#### Scenario: The turn ends
- **WHEN** the turn ends with a non-blocking question pending
- **THEN** its block reads `○ withdrawn` and the agent gets `cancel`

#### Scenario: Withdrawn at the turn's end
- **WHEN** the agent sends `$/cancel_request` for a pending non-blocking question, then ends the turn
- **THEN** its block reads `○ withdrawn`
