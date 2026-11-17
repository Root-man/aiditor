# Tasks

## 1. Settings and project file

- [x] 1.1 Add `enabled_agents: Option<Vec<String>>` to `Settings`, rejecting an unregistered name after the merge with an error naming `enabled_agents` and the agent; add a commented example to `settings.toml`; verify with a settings unit test for the accepted list and the rejected name
- [x] 1.2 Add `enabled_agents: Option<Vec<String>>` to `Project` (`skip_serializing_if = "Option::is_none"`), set it in `Loaded::ad_hoc` and the test fixtures; verify the round-trip test and a test that adding a root keeps the key in the file

## 2. Enabled agents in the core

- [x] 2.1 Add `Core::enabled_agents(&Project)` (registry order, project list over global, none means all); verify with a unit test for the four agent-registry scenarios
- [x] 2.2 Use it in `agent_names`, `default_agent_index` and `projects_then_agents`; refuse a registered but disabled name in `start_session` (new sessions) and `set_default_agent` with `"<name>" is not enabled in <project>`; verify with harness tests for the completion, preselection and refusal scenarios
- [x] 2.3 Use it in `agent_sessions` (and so `g`), failing with `No agents enabled` when empty; verify with sessions tests that a disabled agent gets no list request and the empty case fails
- [x] 2.4 In `reconcile_projects`, report each unregistered `enabled_agents` name and a `default_agent` outside the enabled agents as `project <file>: ...`, keeping the project; verify with tests next to `an_unknown_default_agent_is_reported_and_the_project_still_loads`
- [x] 2.5 Add `ArgSpec::single` and set it on `agent-start`; verify with a harness test that `M-x agent-start RET` with one enabled agent spawns it with no prompt, and that two enabled agents still prompt

## 3. Docs

- [x] 3.1 Record the decision in `docs/editor.md` (agent registry bullet and the project file keys); verify `cargo test --workspace` and `openspec validate project-enabled-agents` pass
