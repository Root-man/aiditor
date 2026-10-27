## ADDED Requirements

### Requirement: Permission requests show inline
A `session/request_permission` from a session whose `session/new` was answered and whose agent hasn't exited SHALL add a block above the prompt line and stay pending: the line `Permission: <title>`, then a line per option in order, `<n>. <name> (<kind>)`, numbered from 1, with the ACP name of the kind in the `dim` colour and a `dim` rule beside every line of the block. The title is the first line of the request's tool call title (followed by ` …` if it has more), else that of the tool call shown with the same id, else the tool call id. Several requests MAY be pending at once, each in its own block.

#### Scenario: A request shows
- **WHEN** the agent asks permission for tool call `Edit a.txt` with options `Yes` (`allow_once`) and `No` (`reject_once`)
- **THEN** `Permission: Edit a.txt\n1. Yes (allow_once)\n2. No (reject_once)` appears above the prompt line and nothing is answered yet

#### Scenario: A request without a title
- **WHEN** the request's tool call has no title and no call with its id is shown
- **THEN** the header line is `Permission: ` followed by the tool call id

### Requirement: Answering a permission request
`C-c C-y` in a session buffer (`agent-allow-once`) SHALL answer the oldest pending request with its first `allow_once` option, and `C-c C-n` (`agent-reject-once`) with its first `reject_once` option, wherever point is. `RET` on an option line of any pending request SHALL answer it with that option; on any other line `RET` sends the prompt as before. The answer SHALL be the `selected` outcome with the option's id, and the request's block SHALL become the single line `Chose <name> (<kind>) for <title>`. With no pending request `C-c C-y` and `C-c C-n` SHALL echo `No permission request is pending`; when the oldest has no option of the kind they SHALL echo `No allow_once option for <title>` (or `reject_once`) and answer nothing.

#### Scenario: Allowing
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-y` is typed in the prompt area
- **THEN** the `Yes` option is selected and the block reads `Chose Yes (allow_once) for Edit a.txt`

#### Scenario: Rejecting
- **WHEN** the same request is pending and `C-c C-n` is typed
- **THEN** the `No` option is selected and the block reads `Chose No (reject_once) for Edit a.txt`

#### Scenario: RET on an option
- **WHEN** point is on the line `2. No (reject_once)` and `RET` is typed
- **THEN** the `No` option is selected and no prompt is sent

#### Scenario: Oldest first
- **WHEN** requests for `A` and then `B` are pending and `C-c C-y` is typed twice
- **THEN** `A` is answered first, then `B`

#### Scenario: Nothing to answer
- **WHEN** no request is pending, or the oldest has no `allow_once` option, and `C-c C-y` is typed
- **THEN** `No permission request is pending`, or `No allow_once option for <title>`, is echoed and nothing is answered

### Requirement: Cancelling ends pending permission requests
When `C-c C-c` sends `session/cancel`, every pending permission request of the session SHALL be answered with the `cancelled` outcome. When the agent exits, pending requests are dropped unanswered. Either way each block SHALL become the line `Cancelled permission for <title>`.

#### Scenario: Cancelling with a pending request
- **WHEN** a request for `Edit a.txt` is pending and `C-c C-c` is typed
- **THEN** it is answered `cancelled`, `session/cancel` is sent and the block reads `Cancelled permission for Edit a.txt`
