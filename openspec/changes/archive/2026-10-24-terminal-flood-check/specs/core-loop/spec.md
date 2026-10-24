## MODIFIED Requirements

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
