---
name: implement
description: Take one GitHub task issue from issue to merged PR through the OpenSpec flow (explore, propose, apply, verify, archive), stopping only when the user is needed. Use when the user says "/implement #<n>".
argument-hint: "#<issue>"
disable-model-invocation: true
---

# Implement an issue end to end

Issue: `$ARGUMENTS` on `Root-man/aiditor`. Run every step without asking: invoking this skill is the user's go-ahead for all writes, commits, the push, the PR and the merge, so skip the opsx skills' usual confirmations. Stop with `AskUserQuestion` only at the **Stop** points; after the answer, carry on. Report one short line per step.

1. **Pick up.** `gh issue view <n> --json title,body,labels,state`; read the linked `docs/editor.md` sections (the `roadmap` skill explains issue structure). **Stop** if closed, an epic or gate, or it has open blockers (`gh api repos/Root-man/aiditor/issues/<n>/dependencies/blocked_by --jq '.[] | select(.state=="open") | "#\(.number) \(.title)"'`). `git switch main && git pull --ff-only` (**Stop** if the tree is dirty), then `git switch -c <short-kebab-slug>`, like `core-skeleton`.
2. **Explore.** `opsx:explore` with the issue's title, body and acceptance criteria. Collect only questions whose answers change scope, behavior or design; pick sensible defaults for the rest and note them. **Stop** if any questions remain: ask them all in one `AskUserQuestion` (recommended option first).
3. **Propose.** `opsx:propose` with the issue, the answers and the defaults; the change name is the branch slug. A settled open question is recorded in `docs/editor.md` (AGENTS.md).
4. **Apply.** `opsx:apply`, every task. Then `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and anything else `.github/workflows/ci.yml` runs must pass; fix and rerun until they do.
5. **Manual checks.** If any acceptance criterion is only checkable by hand (GUI rendering, real key input, IME, macOS-only behavior), **Stop**: list the exact steps and expected results and ask whether they passed. On a failure, fix, rerun step 4's checks and ask again.
6. **Verify.** `opsx:verify`. Fix every CRITICAL and WARNING, apply the suggestions that fit this change (keep a one-line reason for each skipped one, for the PR body), rerun step 4's checks, and rerun verify until it reports no CRITICAL. **Stop** only if a fix would change scope or contradict the spec.
7. **Commit.** One commit: subject is the issue title, a short body, the attribution trailer from the system reminder.
8. **Archive.** `opsx:archive` (sync the delta specs if offered; don't stop on warnings). Commit separately: `Archive <change> OpenSpec change`.
9. **Ship.**
   - `git push -u origin HEAD`.
   - `gh pr create --title "<issue title>" --body-file <scratchpad file>`, body in the style of `gh pr view 125`: `Closes #<n>`, bullets of what changed and why, a `Tests:` line (what is automated, what was checked by hand or that nothing was), `## Notes for review` (known limits, skipped suggestions, platforms not tested), the PR attribution line.
   - `gh pr checks <pr> --watch --fail-fast` in the background.
   - Red: `gh run view <run> --log-failed`, fix, rerun step 4's checks, commit `Fix CI: <what>`, push, wait again. **Stop** after three failed rounds.
   - Green: `gh pr merge <pr> --squash --subject "<issue title> (#<n>)"`, then `git switch main && git pull --ff-only`.

Finish with the PR link, what was checked by hand, and anything the user should know.
