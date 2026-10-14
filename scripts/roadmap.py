#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pyyaml"]
# ///
"""Create GitHub labels, milestones and issues from docs/roadmap.yaml.

Dry run by default: reads the repo's existing milestones and issues and prints
what it would create. Pass --apply to create them.

Safe to re-run. Labels are upserted. Milestones and issues are matched by title,
and existing ones are skipped and never edited. Sub-issue and "blocked by" links
are only added where this run created the issue or its blocker.

Usage:
    scripts/roadmap.py                      # dry run, all milestones
    scripts/roadmap.py --only m0,m1         # dry run, two milestones
    scripts/roadmap.py --only m0 --apply    # create M0
"""

import argparse
import json
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_ROADMAP = ROOT / "docs" / "roadmap.yaml"
KIND_LABELS = {"epic", "feature", "decision", "spike", "gate"}
# GitHub throttles content creation (roughly 80 requests a minute); stay under it.
CREATE_DELAY_S = 1.0


@dataclass
class Item:
    key: str
    title: str
    body: str
    labels: list[str]
    milestone: str
    parent: str | None = None
    blocked_by: list[str] = field(default_factory=list)
    number: int | None = None
    created: bool = False


def gh(*args: str, stdin: str | None = None) -> str:
    result = subprocess.run(["gh", *args], input=stdin, capture_output=True, text=True)
    if result.returncode != 0:
        sys.exit(f"gh {' '.join(args)} failed:\n{result.stderr.strip()}")
    return result.stdout


def render_body(raw: dict, design_url: str | None, extra: str = "") -> str:
    parts = [raw.get("body", "").strip()]
    if raw.get("acceptance"):
        criteria = "\n".join(f"- [ ] {c}" for c in raw["acceptance"])
        parts.append(f"## Acceptance criteria\n{criteria}")
    if extra:
        parts.append(extra)
    if design_url:
        parts.append(f"**Design:** {design_url}")
    return "\n\n".join(p for p in parts if p)


def build_items(roadmap: dict, only: set[str] | None) -> tuple[list[dict], list[Item]]:
    """Flatten the roadmap into issues in creation order, validating as we go."""
    defined_labels = {label["name"] for label in roadmap["labels"]}
    design_doc = roadmap["design_doc"]
    milestones = [m for m in roadmap["milestones"] if not only or m["key"] in only]
    if only and (unknown := only - {m["key"] for m in roadmap["milestones"]}):
        sys.exit(f"Unknown milestone keys: {', '.join(sorted(unknown))}")

    items: list[Item] = []

    def add(raw: dict, milestone: dict, kind: str, parent: Item | None, design: str | None) -> Item:
        inherited = [label for label in parent.labels if label not in KIND_LABELS] if parent else []
        labels = list(dict.fromkeys(inherited + raw.get("labels", [])))
        if kind == "epic":
            labels.append("epic")
        elif not KIND_LABELS & set(labels):
            labels.append("feature")
        if undefined := set(labels) - defined_labels:
            sys.exit(f"{raw['key']}: undefined labels {sorted(undefined)}")
        design = raw.get("design", design)
        extra = "Tasks are tracked as sub-issues." if kind == "epic" and raw.get("tasks") else ""
        item = Item(
            key=raw["key"],
            title=raw["title"],
            body=render_body(raw, f"{design_doc}{design}" if design else None, extra),
            labels=labels,
            milestone=milestone["title"],
            parent=parent.key if parent else None,
            blocked_by=list(raw.get("blocked_by", [])),
        )
        items.append(item)
        return item

    for milestone in milestones:
        epic_keys = []
        for epic_raw in milestone["epics"]:
            epic = add(epic_raw, milestone, "epic", None, None)
            epic_keys.append(epic.key)
            for task_raw in epic_raw.get("tasks", []):
                add(task_raw, milestone, "task", epic, epic_raw.get("design"))
        gate = {
            "key": f"{milestone['key']}-gate",
            "title": f"Gate: {milestone['title']}",
            "labels": ["gate"],
            "body": f"{milestone['gate']}\n\nTest this by hand on macOS and Linux, then close the issue and the milestone.",
            "blocked_by": epic_keys,
        }
        add(gate, milestone, "gate", None, None)

    all_keys = {
        task["key"]
        for m in roadmap["milestones"]
        for epic in m["epics"]
        for task in [epic, *epic.get("tasks", [])]
    }
    seen_keys, seen_titles = set(), set()
    for item in items:
        if item.key in seen_keys:
            sys.exit(f"Duplicate key: {item.key}")
        if item.title in seen_titles:
            sys.exit(f"Duplicate title: {item.title}")
        seen_keys.add(item.key)
        seen_titles.add(item.title)
        for dep in item.blocked_by:
            if dep not in all_keys and dep not in seen_keys:
                sys.exit(f"{item.key}: blocked_by unknown key {dep}")
    check_no_cycles(roadmap)
    return milestones, items


def check_no_cycles(roadmap: dict) -> None:
    deps = {
        raw["key"]: raw.get("blocked_by", [])
        for m in roadmap["milestones"]
        for epic in m["epics"]
        for raw in [epic, *epic.get("tasks", [])]
    }
    done, visiting = set(), []

    def visit(key: str) -> None:
        if key in done:
            return
        if key in visiting:
            cycle = visiting[visiting.index(key):] + [key]
            sys.exit(f"Dependency cycle: {' → '.join(cycle)}")
        visiting.append(key)
        for dep in deps.get(key, []):
            visit(dep)
        visiting.pop()
        done.add(key)

    for key in deps:
        visit(key)


def all_titles_by_key(roadmap: dict) -> dict[str, str]:
    """Titles for every key in the roadmap, so blockers outside --only can still be found on GitHub."""
    titles = {}
    for m in roadmap["milestones"]:
        for epic in m["epics"]:
            for raw in [epic, *epic.get("tasks", [])]:
                titles[raw["key"]] = raw["title"]
        titles[f"{m['key']}-gate"] = f"Gate: {m['title']}"
    return titles


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--roadmap", type=Path, default=DEFAULT_ROADMAP)
    parser.add_argument("--only", help="comma-separated milestone keys, e.g. m0,m1")
    parser.add_argument("--apply", action="store_true", help="create on GitHub (default is a dry run)")
    args = parser.parse_args()

    roadmap = yaml.safe_load(args.roadmap.read_text())
    repo = roadmap["repo"]
    only = set(args.only.split(",")) if args.only else None
    milestones, items = build_items(roadmap, only)
    by_key = {item.key: item for item in items}
    mode = "APPLY" if args.apply else "DRY RUN"
    print(f"[{mode}] {repo}: {len(milestones)} milestones, {len(items)} issues in roadmap\n")

    # Existing state on GitHub (read-only calls, also in a dry run).
    existing_milestones = set(
        gh("api", "--paginate", f"repos/{repo}/milestones?state=all&per_page=100", "--jq", ".[].title").splitlines()
    )
    existing_issues: dict[str, int] = {}
    for issue in json.loads(gh("issue", "list", "--repo", repo, "--state", "all", "--limit", "5000", "--json", "number,title")):
        existing_issues.setdefault(issue["title"], issue["number"])

    # Labels.
    print("Labels")
    for label in roadmap["labels"]:
        print(f"  upsert  {label['name']}")
        if args.apply:
            gh("label", "create", label["name"], "--repo", repo, "--color", label["color"],
               "--description", label.get("description", ""), "--force")

    # Milestones.
    print("\nMilestones")
    for milestone in milestones:
        if milestone["title"] in existing_milestones:
            print(f"  exists  {milestone['title']}")
            continue
        print(f"  create  {milestone['title']}")
        if args.apply:
            description = f"{milestone['description']}\n\nGate: {milestone['gate']}"
            gh("api", "-X", "POST", f"repos/{repo}/milestones",
               "-f", f"title={milestone['title']}", "-f", f"description={description}")

    # Issues, in roadmap order so numbers follow it.
    print("\nIssues")
    placeholder = 0
    for item in items:
        indent = "      " if item.parent else "  "
        if item.title in existing_issues:
            item.number = existing_issues[item.title]
            print(f"{indent}exists  #{item.number} {item.title}")
            continue
        item.created = True
        cmd = ["issue", "create", "--repo", repo, "--title", item.title, "--body-file", "-",
               "--milestone", item.milestone]
        for label in item.labels:
            cmd += ["--label", label]
        parent = by_key.get(item.parent) if item.parent else None
        if parent:
            cmd += ["--parent", str(parent.number)]
        if args.apply:
            url = gh(*cmd, stdin=item.body).strip()
            item.number = int(url.rstrip("/").rsplit("/", 1)[1])
            time.sleep(CREATE_DELAY_S)
        else:
            placeholder += 1
            item.number = -placeholder  # stands in for the number GitHub would assign
        shown = f"#{item.number}" if item.number > 0 else "#new"
        print(f"{indent}create  {shown} {item.title}  [{', '.join(item.labels)}]")

    # "Blocked by" links, only where this run created one side of the link.
    print("\nBlocked by")
    titles = all_titles_by_key(roadmap)
    for item in items:
        blockers = []
        for dep in item.blocked_by:
            blocker = by_key.get(dep)
            if blocker is None:  # outside --only: look it up on GitHub
                number = existing_issues.get(titles[dep])
                if number is None:
                    print(f"  skip    {item.title} ← {titles[dep]} (not on GitHub yet)")
                    continue
                blocker = Item(dep, titles[dep], "", [], "", number=number)
            if item.created or blocker.created:
                blockers.append(blocker)
        if not blockers:
            continue
        print(f"  {item.title}\n      ← " + "\n      ← ".join(b.title for b in blockers))
        if args.apply:
            gh("issue", "edit", str(item.number), "--repo", repo,
               "--add-blocked-by", ",".join(str(b.number) for b in blockers))
            time.sleep(CREATE_DELAY_S)

    if not args.apply:
        print("\nDry run only. Re-run with --apply to create.")


if __name__ == "__main__":
    main()
