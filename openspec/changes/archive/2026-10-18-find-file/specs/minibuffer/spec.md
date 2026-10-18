# Spec Delta

## ADDED Requirements

### Requirement: M-RET returns the text as typed
`M-RET` in a completing minibuffer SHALL close it and return its text as typed, whether or not a candidate matches.

#### Scenario: A name that matches something else
- **WHEN** at a prompt completing over `apple`, `banana`, `cherry` the keys `ch M-RET` are typed
- **THEN** the command gets the string `ch`
