---
name: implement
description: Take one GitHub task issue from issue to merged PR through the OpenSpec flow, with user approval before shipping. Invoke explicitly with /implement in Claude Code or $implement in Codex.
argument-hint: "#<issue>"
disable-model-invocation: true
---

# Implement an issue end to end

Use the issue number supplied with the invocation (`$ARGUMENTS` in Claude Code or the user's `$implement #<n>` message in Codex) on `Root-man/aiditor`. If missing, ask for it before starting. Invoking this skill is the user's go-ahead for the local implementation writes and commits, so skip the OpenSpec skills' usual confirmations; the push, the PR and the merge wait for the user's approval in step 8. At **Stop** points, use the available question tool (`AskUserQuestion` in Claude Code) or ask in chat, then wait for the answer. Report one short line per step.

Read and follow the named `openspec-*` skills below. Claude Code also exposes them as `/opsx:*` commands; Codex exposes them as `$openspec-*` skills. Both read the same skill files.

1. **Pick up.** `gh issue view <n> --json title,body,labels,state`; read the linked `docs/editor.md` sections (the `roadmap` skill explains issue structure). **Stop** if closed, an epic or gate, or it has open blockers (`gh api repos/Root-man/aiditor/issues/<n>/dependencies/blocked_by --jq '.[] | select(.state=="open") | "#\(.number) \(.title)"'`). `git switch main && git pull --ff-only` (**Stop** if the tree is dirty), then `git switch -c <short-kebab-slug>`, like `core-skeleton`.
2. **Explore.** `openspec-explore` with the issue's title, body and acceptance criteria. Collect only questions whose answers change scope, behavior or design; pick sensible defaults for the rest and note them. **Stop** if any questions remain: ask them together (recommended option first).
3. **Propose.** `openspec-propose` with the issue, the answers and the defaults; the change name is the branch slug. A settled open question is recorded in `docs/editor.md` (AGENTS.md).
4. **Apply.** `openspec-apply-change`, every task. Then `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and anything else `.github/workflows/ci.yml` runs must pass; fix and rerun until they do.
5. **Manual checks.** Turn every acceptance criterion you can into an automated test: core behavior through the harness, frontend behavior through GPUI's test platform (`TestAppContext`, `VisualTestContext`). For what is left (what the frame actually draws, real key input), check it yourself if a computer-use tool is available: launch with `cargo run -p aiditor -- --config-dir <scratch dir> [--debug]`, drive it, judge the screenshots, close it. **Stop** only for what you still can't check (IME, dead keys, macOS-only behavior without computer use): list the exact steps and expected results and ask whether they passed. On a failure, fix, rerun step 4's checks and check again.
6. **Verify.** Run `openspec-verify-change` in a fresh subagent so it judges the code against the proposal alone, not this session's reasoning. In Claude Code use `Agent` with `subagent_type: "general-purpose"`, `model: "sonnet"`, `effort: "high"`; in Codex use `spawn_agent` with `fork_turns: "none"` and the inherited model. Supply only the change name and its path `openspec/changes/<change>/`, tell it to run the `openspec-verify-change` skill without asking questions, and return the full report without fixing anything. If fresh subagents are unavailable, **Stop** and ask how to perform the separate review. Fix every CRITICAL and WARNING, apply the suggestions that fit this change (keep a one-line reason for each skipped one, for the PR body), rerun step 4's checks, and rerun verify (a new subagent each time) until it reports no CRITICAL. **Stop** if a fix would change scope or contradict the spec.
7. **Commit.** One commit: subject is the issue title, a short body, and an attribution trailer only if the active agent's instructions supply one.
8. **Review.** **Stop** before pushing, every time: the user tests the branch by hand first. Give an overview of what was done: what changed and why, by area, with the main files; the defaults picked in step 2; known limits and skipped verify suggestions; what is tested automatically and what was checked by hand. Then testing suggestions: exact steps to try in the app (`cargo run -p aiditor`, keys to press, starting state such as files or projects to create) with the expected result of each, edge cases worth poking, and what to look at on disk or in `*Messages*`. Ask whether to ship, over `Ship it` and `Changes needed`, and wait for explicit approval. On changes, fix them, update the change's specs and docs to match, rerun step 4's checks, commit `Address review: <what>`, and come back to this step with what changed and how to retest it.
9. **Archive.** `openspec-archive-change` (sync the delta specs if offered; don't stop on warnings), after the review so its fixes reach the main specs. Commit separately: `Archive <change> OpenSpec change`.
10. **Ship.**
   - `git push -u origin HEAD`.
   - `gh pr create --title "<issue title>" --body-file <scratchpad file>`, body in the style of `gh pr view 125`: `Closes #<n>`, bullets of what changed and why, a `Tests:` line (what is automated, what was checked by hand or that nothing was), `## Notes for review` (known limits, skipped suggestions, platforms not tested), and any PR attribution supplied by the active agent's instructions.
   - `gh pr checks <pr> --watch --fail-fast` in the background.
   - Red: `gh run view <run> --log-failed`, fix, rerun step 4's checks, commit `Fix CI: <what>`, push, wait again. **Stop** after three failed rounds.
   - Green: `gh pr merge <pr> --squash --subject "<issue title> (#<n>)"`, then `git switch main && git pull --ff-only`.

Finish with the PR link, what was checked by hand, and anything the user should know.
