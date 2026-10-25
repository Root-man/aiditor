## ADDED Requirements

### Requirement: Loading direnv
After a capture succeeds, when an executable `direnv` is found in a directory of the captured `PATH`, the editor SHALL run `direnv export json` in the project's primary repo, with the captured environment, `TERM=dumb` and stdin from `/dev/null`, and apply its output to the captured environment: a string value sets the variable and `null` removes it. Empty output SHALL change nothing. When direnv exits non-zero, does not finish within 30 seconds, or prints anything but a JSON object of strings and nulls, the captured environment SHALL be kept unchanged and a line naming the project and the reason SHALL be added to `*Messages*`. Nothing SHALL be run when the capture fell back, when nothing is captured because the editor was launched from a shell, or when `direnv` is not on the captured `PATH`.

#### Scenario: An allowed `.envrc`
- **WHEN** the primary repo has an allowed `.envrc` exporting `FOO=bar` and unsetting `OLD`, and the project's environment is captured
- **THEN** the project's environment has `FOO=bar` and no `OLD`

#### Scenario: A blocked `.envrc`
- **WHEN** the primary repo's `.envrc` is not allowed, so `direnv export json` exits non-zero
- **THEN** the project's environment is the captured one unchanged and `*Messages*` has a line naming the project and direnv's error

#### Scenario: No direnv
- **WHEN** no directory of the captured `PATH` holds `direnv`
- **THEN** no direnv runs and the project's environment is the captured one
