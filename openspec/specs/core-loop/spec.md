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
Services SHALL send their results to the core as messages on one bounded channel. The app SHALL drain that channel into the core on the UI thread, in the order the messages were sent.

#### Scenario: A test service's result reaches the core
- **WHEN** a service running on the async runtime sends a message on the channel
- **THEN** the receiver gets it, and handling it changes the core's state as that message describes

### Requirement: The core stays free of the UI toolkit and the async runtime
The core crate SHALL NOT depend on the UI toolkit or the async runtime, so tests drive it without either.

#### Scenario: Dependency check
- **WHEN** CI lists the core crate's dependency tree
- **THEN** neither the UI toolkit nor the async runtime appears in it
