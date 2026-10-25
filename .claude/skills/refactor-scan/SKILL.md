---
name: refactor-scan
description: Read-only scan of the repo, or a folder or crate, for refactoring opportunities (code smells, SOLID violations, slow paths, huge files and god objects, plus the bugs, risks and bloat a ponytail audit finds), reported as one table scored on impact and complexity. Changes nothing. Use for "find refactoring opportunities", "what should we refactor", "code smells", "tech debt scan", /refactor-scan or $refactor-scan.
argument-hint: "[path]"
---

# Refactor scan

Scan what the user names (`$ARGUMENTS` in Claude Code, the rest of the `$refactor-scan` message in Codex): a path, a crate or the whole repo. Nothing named: the whole repo. This is a report; change no code.

## 1. Run the ponytail audit

Run the `ponytail-audit` skill on the same scope: the `Skill` tool with `ponytail:ponytail-audit` in Claude Code, `$ponytail-audit` in Codex. If it is not installed, read `~/.claude/plugins/marketplaces/ponytail/skills/ponytail-audit/SKILL.md` and follow it. If that file is missing too, say so and continue with step 2 only.

Use the audit for its map of the code, its findings and its rules in "Check before you report". Don't print its report: its findings become rows of the table in step 4.

## 2. Look for what the audit does not cover

Judge design against the architecture in [docs/editor.md](../../../docs/editor.md) and the layers the code already has (`aiditor-core` is pure state and logic, `aiditor-services` does IO, the `aiditor` crate is the GPUI frontend), not against textbook patterns.

- **Code smells:** long functions doing several jobs, long parameter lists, deep nesting, the same `match` over one enum repeated in several files, flags that switch a function between two behaviors, primitive values that keep travelling together and want a type, magic numbers, comments that explain what unclear code does.
- **SOLID:** a struct or module with several reasons to change (single responsibility); adding a variant means editing many files (open/closed); trait impls that panic or ignore methods (Liskov); fat traits that callers use one method of (interface segregation); core logic that reaches for files, processes or GPUI directly instead of going through the existing boundaries (dependency inversion).
- **Performance:** work in render or per-keystroke paths that could be done once (clones, allocations, re-parsing, re-highlighting whole buffers), blocking IO on the UI thread, O(n²) over buffer lines or project files, collections that only grow.
- **Organization:** the biggest files and types, god objects that everything reaches into, modules in the wrong crate or layer, `pub` items used only inside their crate, tests far from the code they test.

Find the big files first, then read the top of the list:

```sh
git ls-files '*.rs' | xargs wc -l | sort -rn | head -20
```

Every opportunity needs the audit's concrete case: "this change to the code, or this input, hits this problem". No case, no row. Size alone is not a finding: a long file with one job is fine. Don't list what a `shortcut:` comment already names, unless the current work already crosses its limit.

## 3. Score

- **Impact:** what the change buys.
  - `high`: removes a bug or data-loss risk, fixes a slowness the user can feel, or unblocks planned work (check `docs/roadmap.yaml`).
  - `med`: makes a part that changes often clearly easier to read, test or change, or deletes a good chunk of code.
  - `low`: local cleanup.
- **Complexity:** what the change costs.
  - `low`: one file, a mechanical change, existing tests cover it.
  - `med`: several files in one crate, or new tests needed.
  - `high`: crosses crates or changes a public API or data model, or needs a manual check of GPUI behavior.

## 4. Output

Start with `Scope:` (what you scanned) and `What this code does:` in two sentences.

Then one table, sorted by impact (high first), then by complexity (low first). Number the rows so the user can say "do 2 and 5". At most 20 rows; say how many smaller ones you left out.

| # | Opportunity | Category | Where | Impact | Complexity | Fix |
|---|---|---|---|---|---|---|
| 1 | `<Type>` parses keys and also writes files | SOLID: single responsibility | `crates/<crate>/src/<file>.rs:120-480` | high | med | Move the writes to the existing `aiditor-services` file service; key parsing stays. |

Category is one of: `Bug`, `Risk`, `Smell`, `SOLID: <principle>`, `Performance`, `Organization`, `Lean` (delete, reuse, merge), `Test`. Keep cells short: the Opportunity says what is wrong, Fix says the smallest change that solves it. Prefer fixes that delete code; never propose a layer or abstraction the problem does not need.

Below the table, a few lines only for rows whose case does not fit in a cell: `#n: <the concrete case>`.

End with:
- `Start with:` the one or two rows with the best impact for their complexity, and why.
- `Not checked:` the parts you did not read or could not run.
