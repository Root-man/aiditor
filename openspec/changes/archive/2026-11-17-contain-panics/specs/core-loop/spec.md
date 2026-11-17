# Spec Delta

## ADDED Requirements

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
