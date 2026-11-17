# core-loop Specification

## Purpose
The core is a state machine that the app drives with messages: key input, service results and timer wake-ups go in with the time they are handled, and requests for the app to carry out come back.

## Requirements

### Requirement: Every message is handled with the current time
The core SHALL handle one message at a time together with the time it is handled, and SHALL NOT read the system clock itself. Handling returns the effects the app must carry out.

#### Scenario: Time comes from the caller
- **WHEN** a test handles messages with a fake time that never advances
- **THEN** nothing in the core depends on how much real time has passed

#### Scenario: A key message edits the buffer
- **WHEN** the message for the key `x` is handled
- **THEN** the current buffer's text is `x`

### Requirement: Services deliver results as messages over a bounded channel
Services SHALL send their results to the core as messages on bounded channels: one shared by the services, and one for each terminal, so a terminal's output flood cannot hold back other services' results. The app SHALL drain every channel into the core on the UI thread, each in the order its messages were sent, and SHALL let other UI work, such as input and rendering, run between two messages of a channel, so a channel that never empties cannot stop the editor from handling keys and drawing.

#### Scenario: A test service's result reaches the core
- **WHEN** a service running on the async runtime sends a message on the channel
- **THEN** the receiver gets it, and handling it changes the core's state as that message describes

#### Scenario: A full terminal channel does not block other services
- **WHEN** a terminal's channel is full and not being drained
- **THEN** a message sent on the shared channel still reaches the core

#### Scenario: A channel that never empties does not starve the UI
- **WHEN** a producer keeps a channel full while the app drains it
- **THEN** other work queued on the UI thread still runs before the producer stops

### Requirement: The core stays free of the UI toolkit and the async runtime
The core crate SHALL NOT depend on the UI toolkit or the async runtime, so tests drive it without either.

#### Scenario: Dependency check
- **WHEN** CI lists the core crate's dependency tree
- **THEN** neither the UI toolkit nor the async runtime appears in it

### Requirement: A panic while handling a message is contained
When handling a message the frontend passed in panics, whether a key, typed text, a service result, a timer wake-up or a resize, the editor SHALL keep running. The echo line SHALL say that the core panicked and its state may be inconsistent, and `*Messages*` SHALL get that line, the panic message, where it happened and a backtrace. On macOS this SHALL hold for keys too, which arrive inside a callback where an uncaught panic aborts.

#### Scenario: A service result panics
- **WHEN** the frontend hands the core a message whose handling panics with `boom`
- **THEN** the editor keeps running, the echo line says the core panicked and its state may be inconsistent, and `*Messages*` holds `boom`, the panic's source location and a backtrace

#### Scenario: A timer panics
- **WHEN** a timer's wake-up panics while it fires
- **THEN** the editor keeps running and the panic is reported the same way

#### Scenario: A key panics outside a command
- **WHEN** a key's handling panics outside any command handler
- **THEN** the process does not end or abort, the panic is reported the same way, and later keys are handled

### Requirement: A panic while building the frame is contained
When building the frame for redisplay panics, the editor SHALL keep running and report the panic as for a message. That redisplay SHALL draw no windows but SHALL still draw the echo line, so keys such as save and quit keep working.

#### Scenario: Redisplay panics
- **WHEN** building the frame panics
- **THEN** the editor draws the echo line with the report, `*Messages*` holds the panic and a backtrace, and the next key is handled
