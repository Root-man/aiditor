## ADDED Requirements

### Requirement: C-u builds a prefix argument
`C-u` SHALL start a prefix argument whose value is 4, and each further `C-u` before any digit SHALL multiply it by 4. The next command SHALL receive it as given by `C-u` with that value.

#### Scenario: One C-u
- **WHEN** `C-u` and then a test command's keys are pressed
- **THEN** the command receives a `C-u` prefix argument of 4

#### Scenario: Repeated C-u
- **WHEN** `C-u C-u` and then a test command's keys are pressed
- **THEN** the command receives a `C-u` prefix argument of 16

### Requirement: Digits and minus give a number
Digits typed after `C-u`, `M-<digit>` or `C--` SHALL append to the number. `-` right after `C-u` SHALL make the argument negative. `M-0` to `M-9` SHALL start a number or append to one already being built. `C--` SHALL negate a number, start a lone minus worth -1, or clear a lone minus. A command SHALL receive a number as a number, and a lone minus as a minus worth -1. After digits, `C-u` SHALL end the number, keeping it, so the next digit is no longer part of it.

#### Scenario: C-u and digits
- **WHEN** `C-u 3` and then a test command's keys are pressed
- **THEN** the command receives the number 3

#### Scenario: Several digits
- **WHEN** `C-u 1 2` and then a test command's keys are pressed
- **THEN** the command receives the number 12

#### Scenario: Negative after C-u
- **WHEN** `C-u - 5` and then a test command's keys are pressed
- **THEN** the command receives the number -5

#### Scenario: Meta digits
- **WHEN** `M-4 2` and then a test command's keys are pressed
- **THEN** the command receives the number 42

#### Scenario: C-- alone
- **WHEN** `C--` and then a test command's keys are pressed
- **THEN** the command receives a minus worth -1

#### Scenario: C-- and digits
- **WHEN** `C-- 7` and then a test command's keys are pressed
- **THEN** the command receives the number -7

### Requirement: The next command takes the prefix argument
The first command run after the prefix argument keys SHALL receive the prefix argument, and the argument SHALL be cleared, so the command after it receives none. `C-g` SHALL clear it too. A plain key that types or moves SHALL clear it. The prefix argument keys SHALL leave the last command unchanged.

#### Scenario: Only the next command
- **WHEN** `C-u 3`, a test command's keys and the same keys again are pressed
- **THEN** the first run receives 3 and the second receives no prefix argument

#### Scenario: Quit clears it
- **WHEN** `C-u 3 C-g` and then a test command's keys are pressed
- **THEN** the command receives no prefix argument

#### Scenario: Last command is kept
- **WHEN** a test command runs and then `C-u` is pressed
- **THEN** the last command is still the test command
