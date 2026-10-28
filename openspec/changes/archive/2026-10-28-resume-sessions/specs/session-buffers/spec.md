## ADDED Requirements

### Requirement: Loading a session into a new session buffer
Loading a listed session SHALL create a session buffer as `agent-start` does, owned by the current workspace, named after the listed title as a `session_info_update` would name it, and a stderr log buffer, without showing the session buffer; the agent SHALL be spawned to load that session in the project's primary repo, waiting for the environment capture as a start does. The echo area SHALL show `Loading <agent> session...`, and `<agent> session ready (modes: <values>)` once the load answers. When the load answers, the selected window SHALL show the session buffer if it still shows the buffer the load was started from; otherwise nothing is switched. Before the load answers, `RET` in the session buffer SHALL echo `<agent> is not ready`.

#### Scenario: Loading shows the history
- **WHEN** a `claude` session titled `Fix it` is loaded in workspace `work` and the agent replays a user message `hi` and an agent message `hello`
- **THEN** `*claude: Fix it*<work>` exists, unshown, and its text is `> hi`, a blank line, `hello`, a blank line and the prompt line

#### Scenario: The loaded session shows once ready
- **WHEN** the load answers while the selected window still shows the sessions buffer it was started from
- **THEN** the selected window shows the session buffer

#### Scenario: A load answered elsewhere stays hidden
- **WHEN** the load answers after the selected window was switched to another buffer
- **THEN** the selected window keeps that buffer

### Requirement: Replayed user messages show as prompts
A `user_message_chunk` with text SHALL be put above the prompt line as its own block, `> ` followed by the text and a blank line, rendered as markdown as a sent prompt is, ending any streamed block; chunks with other content SHALL show nothing.

#### Scenario: A user message between agent messages
- **WHEN** an agent message chunk `a`, a user message chunk `b` and an agent message chunk `c` arrive
- **THEN** the transcript reads `a`, `> b` and `c`, each followed by a blank line
