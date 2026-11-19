## ADDED Requirements

### Requirement: Agent session events notify while the session can't be seen
A session SHALL request a notification tagged with its session buffer and titled `<agent> · <workspace>`, with the names its buffer was given, when it asks for permission (body: the tool call's title), asks a blocking question (body: the question), ends a turn not cancelled (body: `Turn ended` for `end_turn`, otherwise the line the session buffer shows for the end), or fails to start or exits on its own (body: the line `*Messages*` or the echo area gets). A cancelled turn and a non-blocking question SHALL request none. A notification SHALL be requested only while the window is inactive or the current workspace's windows don't show the session buffer.

#### Scenario: Permission while the window is inactive
- **GIVEN** a ready `claude` session in workspace `work`, shown, and the window inactive
- **WHEN** the agent asks permission for tool call `Edit main.rs`
- **THEN** a notification titled `claude · work` with body `Edit main.rs` is requested, tagged with the session buffer

#### Scenario: Seen session
- **GIVEN** the session buffer is shown in the current workspace and the window is active
- **WHEN** the agent asks permission, asks a question, ends a turn or exits
- **THEN** no notification is requested

#### Scenario: Session in another workspace
- **GIVEN** the window is active and another workspace is current
- **WHEN** the session's turn ends with `end_turn`
- **THEN** a notification with body `Turn ended` is requested

#### Scenario: Cancelled turn
- **GIVEN** the window is inactive
- **WHEN** the turn ends with `cancelled`
- **THEN** no notification is requested

#### Scenario: Crash
- **GIVEN** the window is inactive
- **WHEN** the agent exits with code 1
- **THEN** a notification with body `claude session exited with code 1` is requested

#### Scenario: Later events replace earlier ones
- **WHEN** the session asks a question and then ends its turn while unseen
- **THEN** both notifications carry the same tag
