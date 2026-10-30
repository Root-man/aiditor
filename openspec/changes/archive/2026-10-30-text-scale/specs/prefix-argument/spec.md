# Spec Delta

## MODIFIED Requirements

### Requirement: Digits and minus give a number
Digits typed after `C-u`, `M-<digit>` or `M--` SHALL append to the number. `-` right after `C-u` SHALL make the argument negative. On macOS `C--` SHALL do all `M--` does. `M-0` to `M-9` SHALL start a number or append to one already being built. `M--` SHALL negate a number, start a lone minus worth -1, or clear a lone minus. A command SHALL receive a number as a number, and a lone minus as a minus worth -1. After digits, `C-u` SHALL end the number, keeping it, so the next digit is no longer part of it.

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

#### Scenario: M-- alone
- **WHEN** `M--` and then a test command's keys are pressed
- **THEN** the command receives a minus worth -1

#### Scenario: M-- and digits
- **WHEN** `M-- 7` and then a test command's keys are pressed
- **THEN** the command receives the number -7

#### Scenario: C-- alone
- **WHEN** on macOS `C--` and then a test command's keys are pressed
- **THEN** the command receives a minus worth -1

#### Scenario: C-- and digits
- **WHEN** on macOS `C-- 7` and then a test command's keys are pressed
- **THEN** the command receives the number -7
