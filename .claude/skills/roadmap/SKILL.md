---
name: roadmap
description: How aiditor's work is tracked on GitHub (milestones, epics, task sub-issues, gates, blocked-by links), how docs/roadmap.yaml describes it and how scripts/roadmap.py creates the issues. Use when picking the next task, breaking down a milestone, adding, changing or closing issues, recording a decision, or editing roadmap.yaml or the script.
---

# Roadmap and GitHub issues

The MVP plan is [docs/roadmap.yaml](../../../docs/roadmap.yaml). [scripts/roadmap.py](../../../scripts/roadmap.py) turns it into labels, milestones and issues on `Root-man/aiditor`. The spec it follows is [docs/editor.md](../../../docs/editor.md). Use `gh` for every GitHub operation.

## Structure on GitHub

milestone → epic → task, plus one gate per milestone.

- **Milestones** are the phases M0–M5 from the spec's milestone table (`M0 · Skeleton` … `M5 · Polish`). The description ends with the gate.
- **Epics** (label `epic`) are parent issues, one per feature area. Their body says "Tasks are tracked as sub-issues."
- **Tasks** are sub-issues of an epic, each one PR. The kind label says what it is:
  - `feature`: implementation work;
  - `decision`: an open question from the spec. Done when the answer is recorded in `docs/editor.md` (usually by removing it from Open questions and writing it into the relevant section);
  - `spike`: a time-boxed investigation with throwaway code. Findings go to `docs/spikes/<topic>.md` or `docs/editor.md`, as the issue says.
- **Gates** (`Gate: <milestone title>`, label `gate`) are blocked by every epic in their milestone. They are tested by hand on macOS and Linux; close the gate, then the milestone.
- **Blocked-by links** give the order. An issue whose blockers are still open shouldn't be started. Decisions and spikes block the tasks they shape.
- **Area labels**: `core`, `text`, `keymap`, `minibuffer`, `frontend`, `files`, `terminal`, `acp`, `project`, `persistence`, `infra`. **Platform labels**: `macos`, `linux`. Tasks inherit their epic's area and platform labels.
- **Issue body**: description, then an "Acceptance criteria" checklist, then a `**Design:**` link to a section of `docs/editor.md`. Tick the checklist as criteria are met; a PR closes its task with `Closes #<n>`.

M0 and M1 are broken down into tasks. M2–M5 have epics plus their known decisions and spikes; each is broken down when its milestone starts.

## roadmap.yaml

Top level: `repo`, `design_doc` (base URL for design links), `labels` (name, color, description) and `milestones`.

```yaml
milestones:
  - key: m2
    title: "M2 · Terminal"
    description: "One line of scope."
    gate: "What you test by hand to close the milestone."
    epics:
      - key: m2-pty
        title: "PTY service"
        labels: [terminal]
        design: "#output-floods"        # anchor in docs/editor.md; tasks inherit it
        body: |
          What the epic covers.
        tasks:
          - key: m2-pty-spawn
            title: "Spawn the user's shell in a PTY"
            labels: [linux]              # optional; area/platform inherited from the epic
            body: |
              What to build.
            acceptance:
              - "Checkable criterion"
            blocked_by: [m0-core-skeleton]  # keys, from any milestone
```

- `key` is a unique id used only by `blocked_by`; it never appears on GitHub.
- `title` must be unique across the file: the script matches existing issues by title.
- Kind labels are added automatically: `epic` for epics, `feature` for tasks unless they already have `decision` or `spike`. Every label used must be defined under `labels`.
- Gates aren't written in the file; the script adds one per milestone from its `gate` text.
- The script rejects duplicate keys or titles, unknown labels, unknown `blocked_by` keys and dependency cycles.

## scripts/roadmap.py

```sh
scripts/roadmap.py                     # dry run, all milestones
scripts/roadmap.py --only m2           # dry run, one milestone
scripts/roadmap.py --only m2 --apply   # create on GitHub
```

- Dry run by default; it still reads GitHub to show what exists. Always dry-run first and check the output before `--apply`.
- Labels are upserted (`gh label create --force`).
- Milestones and issues are matched by title. Missing ones are created; **existing ones are never edited**.
- A task becomes a sub-issue of its epic when it is created.
- Every run adds whatever `blocked_by` links (and gate → epic links) are missing on GitHub, so re-running after a failure is safe.
- It creates in roadmap order, with a 1 s pause per call to stay under GitHub's rate limit. If a `gh` call fails (GitHub sometimes returns "Something went wrong"), check with `gh issue list` that nothing was half-created, then re-run.

## Common work

**Pick the next task.** Open tasks in the current milestone with no open blockers:

```sh
gh issue list --milestone "M0 · Skeleton" --state open --limit 200 \
  --json number,title,labels,blockedBy \
  --jq '.[] | select(any(.labels[]; .name == "epic" or .name == "gate") | not)
        | select([.blockedBy.nodes[] | select(.state == "OPEN")] | length == 0)
        | "#\(.number) \(.title)"'
gh issue view <n> --json title,body,parent,blockedBy
```

**Break down a milestone.** Read the epic bodies and the spec sections they link to, settle or schedule the decisions first, then add `tasks:` under each epic in `roadmap.yaml`. Make each decision or spike block the tasks it shapes, dry-run with `--only <key>`, and apply.

**Add an issue.** Add it to `roadmap.yaml` and run the script, so the file stays the full plan. For a one-off that doesn't belong in the plan (a bug found along the way), use `gh issue create` with the milestone, labels and `--parent <epic>` directly.

**Change an existing issue.** The script won't touch it. Edit it on GitHub with `gh issue edit` (`--title`, `--body-file`, `--add-label`, `--add-blocked-by`, `--remove-blocked-by`, `--parent`), and make the same change in `roadmap.yaml` so the file doesn't drift. Renaming a title in only one place makes the next run create a duplicate.

**Drop an issue.** Close it with `gh issue close <n> --reason "not planned" --comment "<why>"` and remove it from `roadmap.yaml`, including any `blocked_by` that names its key.

**Finish a milestone.** When every epic is closed, test the gate by hand on macOS and Linux, close the gate issue, then close the milestone:

```sh
gh api -X PATCH repos/Root-man/aiditor/milestones/<number> -f state=closed
```
