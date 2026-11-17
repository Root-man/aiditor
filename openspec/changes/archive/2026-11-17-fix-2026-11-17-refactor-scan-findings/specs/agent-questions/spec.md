## MODIFIED Requirements

### Requirement: Skipping, cancelling and withdrawing a question
`C-c C-k` SHALL answer the pending request whose block point is on, and otherwise the oldest pending request: a question with `decline`, its block becoming `▸ – <message>  Skipped`. `C-c C-c`, the end of the turn and killing the session buffer SHALL answer pending questions `cancel`, the block becoming `▸ ✗ <message>  Cancelled`; when the agent exits, the blocks become that line unanswered. A request the agent withdraws with `$/cancel_request`, question or permission request, SHALL close without a reply from the core: a question's block becoming `▸ ○ <message>  Closed`, a permission request's `○ withdrawn`.

#### Scenario: Skipping
- **WHEN** a question is pending and `C-c C-k` is typed
- **THEN** the agent gets `decline` and the block reads `▸ – <message>  Skipped`

#### Scenario: Skipping the question at point
- **WHEN** two questions are pending, point is on the second's block and `C-c C-k` is typed
- **THEN** the second gets `decline` and the first stays pending

#### Scenario: Cancelling
- **WHEN** a question is pending and `C-c C-c` is typed, or the turn ends, or the session buffer is killed
- **THEN** the agent gets `cancel` and the block reads `▸ ✗ <message>  Cancelled`

#### Scenario: Withdrawn
- **WHEN** the agent sends `$/cancel_request` for a pending question
- **THEN** its block reads `▸ ○ <message>  Closed`, the status no longer awaits an answer, and the agent gets the request-cancelled error
