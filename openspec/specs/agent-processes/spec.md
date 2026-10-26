# agent-processes Specification

## Purpose
Runs ACP agent processes off the UI thread: spawns each one in its own process group, initializes it and opens a session in the primary repo, and streams its stderr, requests and exit to the core, which answers the requests.

## Requirements

### Requirement: Spawning an agent and opening a session
An agent SHALL be spawned from an agent id chosen by the core, a command with its arguments, a working directory (the project's primary repo) and environment variables. Spawning SHALL return once the process has started, without waiting for the handshake. The agent's process SHALL inherit the editor's environment with the spawn's variables on top, applied in order so a later variable overrides an earlier one with the same name; the core lists the captured environment first, then the registry's `env`, then the project file's `env`. The command SHALL be looked up on the resulting `PATH`. Once started, the client SHALL send ACP `initialize` advertising neither file system capability nor the terminal capability, then `session/new` with `cwd` set to the working directory and no MCP servers. The core SHALL get one started message for the agent id with the `session/new` response, holding the session id and the agent's config options, or with why the agent could not start: a command that cannot be found or run, a working directory that is not a directory, an error answer to either request, or the agent exiting before it answered.

#### Scenario: A session is opened
- **WHEN** the test agent is spawned in a temporary directory
- **THEN** the core gets a started message with the session id and config options the agent answered, and the agent saw `cwd` equal to that directory, `fs.readTextFile` and `fs.writeTextFile` false and `terminal` false

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
Each line the agent writes to stderr SHALL reach the core as one message tagged with the agent id, without its line ending, in order. When the process exits, the core SHALL get one exit message with its exit code, or the signal that ended it, after every stderr line.

#### Scenario: Stderr then exit
- **WHEN** the test agent writes two lines to stderr and exits with code 3
- **THEN** the core gets both lines in order, then an exit message with code 3

### Requirement: Agent requests wait for the core's reply
Each request the agent sends to the client SHALL reach the core as a message with the agent id, the request id and the request. The request SHALL stay pending until the core replies to that agent and request id with a response or an error, which SHALL then be sent to the agent. A reply for an unknown agent or request id SHALL be ignored. Until permissions and terminals are handled, the core SHALL reply to every agent request with a "method not found" error.

#### Scenario: A request is rejected
- **WHEN** the test agent sends `session/request_permission` after its session opened and the core gets it
- **THEN** the core replies with a "method not found" error, and the agent receives that error

### Requirement: Killing an agent
Killing an agent SHALL send SIGTERM to the agent's process group, which it leads, and SIGKILL to it 100 ms later, so wrappers such as `npx` and the processes they start do not outlive it. Killing SHALL return to the caller at once and drop the agent's pending requests. The core SHALL still get the exit message. A kill of an unknown agent SHALL be ignored.

#### Scenario: Killed agent is gone
- **WHEN** a running test agent is killed
- **THEN** the core gets its exit message and within seconds its process no longer exists

### Requirement: No agents left behind on quit
When the app quits, every live agent SHALL be killed before the process exits, waiting once for the 100 ms grace before SIGKILL.

#### Scenario: Quitting kills every agent
- **WHEN** two test agents are running, one ignoring SIGTERM, and all agents are killed for quitting
- **THEN** it returns after the grace, and within seconds neither agent's process exists

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
