# agent-processes Specification

## Purpose
Runs ACP agent processes off the UI thread: spawns each one in its own process group, initializes it and opens a session in the primary repo, and streams its stderr, requests and exit to the core, which answers the requests.

## Requirements

### Requirement: Spawning an agent and opening a session
An agent SHALL be spawned from an agent id chosen by the core, a command with its arguments, a working directory (the project's primary repo) and environment variables. Spawning SHALL return once the process has started, without waiting for the handshake. The agent's process SHALL inherit the editor's environment with the spawn's variables on top, applied in order so a later variable overrides an earlier one with the same name; the core lists the captured environment first, then the registry's `env`, then the project file's `env`. The command SHALL be looked up on the resulting `PATH`. Once started, the client SHALL send ACP `initialize` advertising neither file system capability nor the terminal capability, and `_meta.terminal_output: true`, then `session/new` with `cwd` set to the working directory and no MCP servers. The core SHALL get one started message for the agent id with the `session/new` response, holding the session id and the agent's config options, or with why the agent could not start: a command that cannot be found or run, a working directory that is not a directory, an error answer to either request, or the agent exiting before it answered.

#### Scenario: A session is opened
- **WHEN** the test agent is spawned in a temporary directory
- **THEN** the core gets a started message with the session id and config options the agent answered, and the agent saw `cwd` equal to that directory, `fs.readTextFile` and `fs.writeTextFile` false, `terminal` false and `terminal_output` true

#### Scenario: Later variables win
- **WHEN** the test agent is spawned with variables `A=1`, `A=2`
- **THEN** the agent sees `A=2`

#### Scenario: The command is found on the given PATH
- **WHEN** the test agent is spawned by its bare file name with `PATH` set to its directory
- **THEN** the core gets a started message with the session

#### Scenario: A missing program
- **WHEN** an agent is spawned with the command `/nonexistent/agent`
- **THEN** the core gets a failure started message for that id with the reason, and the service does not panic

#### Scenario: The agent exits before answering
- **WHEN** an agent is spawned with a command that exits at once without speaking ACP
- **THEN** the core gets a failure started message for that id, then its exit message

### Requirement: Stderr and exit stream to the core
Each line the agent writes to stderr SHALL reach the core as one message tagged with the agent id, without its line ending, in order, with bytes that are not valid UTF-8 replaced by U+FFFD. Such bytes SHALL NOT stop later lines from being read. When the process exits, the core SHALL get one exit message with its exit code, or the signal that ended it, after every stderr line.

#### Scenario: Stderr then exit
- **WHEN** the test agent writes two lines to stderr and exits with code 3
- **THEN** the core gets both lines in order, then an exit message with code 3

#### Scenario: Stderr that is not UTF-8
- **WHEN** the test agent writes the bytes `caf\xE9\n`, then `next\n`, to stderr
- **THEN** the core gets `caf\u{FFFD}` and then `next`, in order

#### Scenario: A chatty agent after a bad byte
- **WHEN** the test agent writes a non-UTF-8 line, then more stderr than a pipe holds, then exits
- **THEN** the agent does not block and the core gets every line, then the exit message

### Requirement: Agent requests wait for the core's reply
Each request the agent sends to the client SHALL reach the core as a message with the agent id, the request id and the request. The request SHALL stay pending until the core replies to that agent and request id with a response or an error, which SHALL then be sent to the agent. A reply for an unknown agent or request id SHALL be ignored. The core SHALL answer `session/request_permission` for a ready session from its session buffer, SHALL answer it with the `cancelled` outcome for an agent whose session has ended or that has no session (its session buffer was killed), and SHALL reply to every other agent request, and to a permission request before the session is ready, with a "method not found" error.

#### Scenario: A request is rejected
- **WHEN** a session agent sends `terminal/create`
- **THEN** the core replies with a "method not found" error, which the service sends to the agent

#### Scenario: A permission request with no session is cancelled
- **WHEN** the test agent sends `session/request_permission` to a core that has no session for it
- **THEN** the core replies with the `cancelled` outcome, and the agent receives it

#### Scenario: A permission request is answered
- **WHEN** a session started with `agent-start` gets a permission request from the fake agent and `C-c C-y`, `C-c C-n` or `RET` on an option line answers it
- **THEN** the agent receives the `selected` outcome with that option's id

### Requirement: Killing an agent
Killing an agent SHALL send SIGTERM to the agent's process group, which it leads, and SIGKILL to it 100 ms later, so wrappers such as `npx` and the processes they start do not outlive it. Killing SHALL return to the caller at once and drop the agent's pending requests. The core SHALL still get the exit message. A kill of an unknown agent SHALL be ignored.

#### Scenario: Killed agent is gone
- **WHEN** a running test agent is killed
- **THEN** the core gets its exit message and within seconds its process no longer exists

### Requirement: No agents left behind on quit
When the app quits, every live agent and every agent still listing sessions SHALL be killed before the process exits, waiting once for the 100 ms grace before SIGKILL.

#### Scenario: Quitting kills every agent
- **WHEN** two test agents are running, one ignoring SIGTERM, and all agents are killed for quitting
- **THEN** it returns after the grace, and within seconds neither agent's process exists

### Requirement: Listing an agent's sessions
A list request SHALL carry a request number chosen by the core, a command with its arguments, a working directory (the project's primary repo) and environment variables, and SHALL return at once. The agent SHALL be spawned as for a session, in its own process group, with its stderr discarded. The client SHALL send ACP `initialize` and, when the agent advertises `sessionCapabilities.list`, `session/list` with `cwd` set to the working directory, then `session/list` again with each `nextCursor` until an answer has none or 50 pages were asked. The client SHALL then send SIGTERM to the process group, and SIGKILL 100 ms later unless it exited. The core SHALL get one listed message with the request number and every page's sessions in order, or why it could not list: a command that cannot be found or run, a working directory that is not a directory, an agent without `sessionCapabilities.list`, an error answer, or the agent exiting before it answered.

#### Scenario: Every page is listed
- **WHEN** the test agent with two pages of sessions in its working directory and one session elsewhere is listed in that directory
- **THEN** the core gets a listed message with the request number and the sessions of both pages in order, without the other one, and within seconds the agent's process no longer exists

#### Scenario: No list capability
- **WHEN** the test agent without sessions is listed
- **THEN** the core gets a listed failure saying the agent can't list sessions

#### Scenario: A missing program
- **WHEN** a list request has the command `/nonexistent/agent`
- **THEN** the core gets a listed failure with the reason

#### Scenario: The agent exits before answering
- **WHEN** a list request has a command that exits at once without speaking ACP
- **THEN** the core gets a listed failure

### Requirement: Session updates stream to the core
Each `session/update` notification the agent sends SHALL reach the core as one message tagged with the agent id, holding the notification as ACP parses it, in the order the agent sent them.

#### Scenario: An update reaches the core
- **WHEN** the test agent sends one session update after its session opened
- **THEN** the core gets an update message for that agent id with the update's session id and content

### Requirement: Prompts and cancels reach the agent
The agent service SHALL send a prompt op's `session/prompt` request to the agent once its session is open and answer with the agent's response or its error message, after every `session/update` the agent sent before that response. A cancel op SHALL send the `session/cancel` notification. Both SHALL be ignored for an unknown agent or one whose session isn't open.

#### Scenario: A turn streams then ends
- **WHEN** a prompt is sent to a fake agent whose turn sends a message chunk then stops with `end_turn`
- **THEN** the core gets the chunk's update and then the prompt answer with `end_turn`

#### Scenario: A cancel ends the turn
- **WHEN** a prompt starts a turn that sleeps and a cancel is sent
- **THEN** the prompt answer has the stop reason `cancelled`

### Requirement: Config options reach the agent
A set-config-option op SHALL send its `session/set_config_option` request to the agent once its session is open, and the core SHALL get a message for the agent id with the config options the agent answered, or the error's message. It SHALL be ignored for an unknown agent or one whose session isn't open.

#### Scenario: Setting the mode option
- **WHEN** a set-config-option op sets a fake agent's `mode` option to `plan`
- **THEN** the core gets the answer whose `mode` option's current value is `plan`

#### Scenario: A refused value
- **WHEN** a set-config-option op sets a fake agent's `mode` option to `nope`
- **THEN** the core gets an error answer

### Requirement: A closed connection ends the agent
When the agent closes its stdout after its session opened, the agent service SHALL terminate the agent as a kill op does: SIGTERM to its process group and SIGKILL after the grace. The core SHALL then get the agent's exit message after its stderr lines, as for any exit. An agent that closed its stdout because it already exited SHALL get the same exit message and nothing more.

#### Scenario: The agent stops speaking but keeps running
- **WHEN** the fake agent's session steps close its stdout and then sleep for 10 seconds
- **THEN** the core gets an exit message for that agent, ended by a signal, well before the sleep ends

#### Scenario: A turn in flight ends
- **WHEN** a prompt is sent to a fake agent whose turn closes its stdout and then sleeps
- **THEN** the core gets the agent's exit message, and the session is no longer busy

### Requirement: Loading a session
A spawn MAY name a session id to load. Such a spawn SHALL start the agent and send `initialize` as any spawn does, then, when the agent advertises `loadSession`, send `session/load` with that session id, `cwd` set to the working directory and no MCP servers, instead of `session/new`. The `session/update`s the agent sends while loading, its replayed history, SHALL reach the core as session updates before the started message. The started message SHALL hold the requested session id and the config options and modes of the `session/load` answer. An agent that doesn't advertise `loadSession` SHALL fail to start with `agent can't load sessions`.

#### Scenario: A session is loaded
- **WHEN** the test agent with a two-update history is spawned to load session `old`
- **THEN** the agent saw `session/load` for `old` with the spawn's `cwd`, and the core gets both updates, then a started message with session id `old` and the agent's config options

#### Scenario: An agent that can't load
- **WHEN** the test agent without a history is spawned to load a session
- **THEN** the core gets a failure started message with `agent can't load sessions`

### Requirement: Additional directories reach the agent
A spawn SHALL carry the project's other roots. When the agent advertises `sessionCapabilities.additionalDirectories`, the service SHALL send them as `additionalDirectories` on `session/new`, or on `session/load` when loading. Otherwise it SHALL send none and, when there were roots to send, report to the core that the agent can't take additional directories before reporting the start.

#### Scenario: The agent takes them on session/new
- **WHEN** a spawn with roots `b` and `c` starts an agent advertising the capability
- **THEN** its `session/new` carries `additionalDirectories` `b` and `c`

#### Scenario: The agent takes them on session/load
- **WHEN** a spawn loading a session with root `b` starts an agent advertising the capability
- **THEN** its `session/load` carries `additionalDirectories` `b`

#### Scenario: The agent can't take them
- **WHEN** a spawn with root `b` starts an agent not advertising the capability
- **THEN** `session/new` carries no additional directories, and the core hears that the agent can't take them before the start

#### Scenario: No roots to send
- **WHEN** a spawn without other roots starts an agent not advertising the capability
- **THEN** the core hears only the start
