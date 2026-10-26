## 1. Plan

- [x] 1.1 Move the tool call's diff-and-replace rewrite into a helper used by tool calls and plans; the existing tool call tests pass
- [x] 1.2 Render `plan` updates as one block per session, rewritten in place; tests in `agents.rs` for a plan, a replacing plan under a later message, an empty plan and point staying on an unchanged line

## 2. Mode

- [x] 2.1 `Buffer::agent_mode` set when the session is ready and on `current_mode_update`, shown as ` [<mode>]` on the mode line; tests for the mode at start, a change and before ready

## 3. Docs

- [x] 3.1 Record the plan and mode display in `docs/editor.md` under the session buffer

## 4. Review

- [x] 4.1 Set `CLAUDE_CODE_ENABLE_TASKS` and `CLAUDE_CODE_ENABLE_TODO_TOOLS` in the bundled `claude` agent's env; the bundled-agent settings test checks them
