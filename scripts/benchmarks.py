#!/usr/bin/env python3
"""Record core and service benchmark results and report a manual Actions run using gh."""

import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
from urllib.parse import urlencode
import zipfile

ROOT = Path(__file__).resolve().parent.parent
ARTIFACT = "core-benchmarks"


def gh(*args, payload=None):
    return subprocess.run(
        ["gh", *args],
        input=json.dumps(payload).encode() if payload is not None else None,
        stdout=subprocess.PIPE,
        check=True,
    ).stdout


def api(endpoint, *args, payload=None):
    return json.loads(gh("api", endpoint, *args, payload=payload))


def validate_measurements(measurements):
    if not measurements:
        raise ValueError("Empty benchmark results")
    for name, values in measurements.items():
        if not re.fullmatch(r"[a-z0-9/-]+", name):
            raise ValueError("Invalid benchmark name")
        numbers = [values[key] for key in ("min_ns", "median_ns", "max_ns")]
        if any(type(n) not in (int, float) or not math.isfinite(n) or n <= 0 for n in numbers):
            raise ValueError(f"Invalid timings for {name}")
        if numbers != sorted(numbers):
            raise ValueError(f"Unordered timings for {name}")


def read_measurements(source):
    measurements = {}
    for line in source.read_text().splitlines():
        name, median, low, high = line.split("\t")
        if name in measurements:
            raise ValueError(f"Duplicate benchmark: {name}")
        measurements[name] = dict(median_ns=float(median), min_ns=float(low), max_ns=float(high))
    validate_measurements(measurements)
    return measurements


def validate(data):
    if data["version"] != 1:
        raise ValueError("Unsupported benchmark result version")
    if not re.fullmatch(r"[0-9a-f]{40}", data["commit"]):
        raise ValueError("Invalid benchmark commit")
    if type(data["run_id"]) is not int or data["run_id"] <= 0:
        raise ValueError("Invalid benchmark run ID")
    for key in ("runner", "arch", "cpu", "rustc", "suite"):
        if not isinstance(data["environment"][key], str) or not data["environment"][key]:
            raise ValueError(f"Missing benchmark environment: {key}")
    validate_measurements(data["benchmarks"])
    return data


def suite_fingerprint():
    paths = [ROOT / "Cargo.toml", ROOT / "scripts/bench_local.py"]
    paths += sorted((ROOT / "benches").rglob("*.rs"))
    for crate in ("aiditor-core", "aiditor-services"):
        paths.append(ROOT / "crates" / crate / "Cargo.toml")
        paths += sorted((ROOT / "crates" / crate / "benches").rglob("*.rs"))
    digest = hashlib.sha256()
    for path in paths:
        digest.update(str(path.relative_to(ROOT)).encode() + b"\0" + path.read_bytes() + b"\0")
    return digest.hexdigest()


def record(source, destination):
    measurements = read_measurements(source)
    cpu = next(
        line.split(":", 1)[1].strip()
        for line in Path("/proc/cpuinfo").read_text().splitlines()
        if line.startswith("model name")
    )
    suite = suite_fingerprint()
    data = validate({
        "version": 1,
        "commit": os.environ["GITHUB_SHA"],
        "run_id": int(os.environ["GITHUB_RUN_ID"]),
        "environment": {
            "runner": os.environ["BENCH_RUNNER"],
            "arch": platform.machine(),
            "cpu": cpu,
            "rustc": subprocess.check_output(["rustc", "-vV"], text=True).strip(),
            "suite": suite,
        },
        "image_version": os.environ.get("ImageVersion", ""),
        "benchmarks": measurements,
    })
    destination.write_text(json.dumps(data, indent=2) + "\n")


def previous_run(repo, branch, current_id):
    query = urlencode({"branch": branch, "status": "success", "event": "workflow_dispatch", "per_page": 100})
    pages = api(f"repos/{repo}/actions/workflows/benchmarks.yml/runs?{query}", "--paginate", "--slurp")
    return max(
        (run for page in pages for run in page["workflow_runs"] if run["id"] < current_id),
        key=lambda run: run["id"], default=None,
    )


def baseline(repo, run):
    if run is None:
        return None
    pages = api(f"repos/{repo}/actions/runs/{run['id']}/artifacts?per_page=100", "--paginate", "--slurp")
    artifact = next((
        item for page in pages for item in page["artifacts"]
        if item["name"] == ARTIFACT and not item["expired"]
    ), None)
    if artifact is None:
        return None
    archive = gh("api", f"repos/{repo}/actions/artifacts/{artifact['id']}/zip")
    with zipfile.ZipFile(io.BytesIO(archive)) as zipped:
        if zipped.getinfo("results.json").file_size > 1_000_000:
            raise ValueError("Benchmark artifact is too large")
        data = validate(json.loads(zipped.read("results.json")))
    if data["run_id"] != run["id"] or data["commit"] != run["head_sha"]:
        raise ValueError("Baseline artifact does not match its workflow run")
    return data


def render(current, previous, run, repository_url):
    lines = [
        "## Core and service benchmarks", "",
        f"Tested commit: [`{current['commit'][:7]}`]({repository_url}/commit/{current['commit']}). "
        f"[Current run]({repository_url}/actions/runs/{current['run_id']}).",
    ]
    if run is not None:
        lines.append(f"[Previous successful run on this branch]({repository_url}/actions/runs/{run['id']}).")
    if previous is None:
        lines.append("No previous results are available; this run establishes a baseline.")
    elif previous["environment"] != current["environment"] or previous["benchmarks"].keys() != current["benchmarks"].keys():
        lines.append("The previous environment or benchmark definitions differ; this run establishes a new baseline.")
        previous = None
    lines += [
        "", "Times are µs/op; lower is faster. Change is relative to the previous median.", "",
        "| Benchmark | Previous median | Current median | Change | Current batch range |",
        "| --- | ---: | ---: | ---: | ---: |",
    ]
    for name, timing in current["benchmarks"].items():
        before = previous["benchmarks"][name]["median_ns"] if previous else None
        old = f"{before / 1000:.3f}" if before is not None else "—"
        change = f"{(timing['median_ns'] / before - 1) * 100:+.1f}%" if before is not None else "—"
        lines.append(
            f"| `{name}` | {old} | {timing['median_ns'] / 1000:.3f} | {change} | "
            f"{timing['min_ns'] / 1000:.3f}–{timing['max_ns'] / 1000:.3f} |"
        )
    lines += [
        "", "Medians and ranges cover 21 warmed-up batch averages, not per-event latency percentiles. "
        "Hosted runners are noisy; these results are informational and do not gate CI.",
    ]
    return "\n".join(lines) + "\n"


def comment(repo, pr, commit, body):
    if not re.fullmatch(r"[1-9][0-9]*", pr):
        raise ValueError("PR must be a positive integer")
    pull = api(f"repos/{repo}/pulls/{pr}")
    head_repo = pull["head"]["repo"]
    if pull["state"] != "open" or head_repo is None or head_repo["full_name"] != repo or pull["head"]["sha"] != commit:
        raise ValueError("PR must be open, belong to this repository, and have the tested commit as its head; summary retained")
    api(f"repos/{repo}/issues/{pr}/comments", "--method", "POST", "--input", "-", payload={"body": body})


def report(source):
    current = validate(json.loads(source.read_text()))
    if current["commit"] != os.environ["GITHUB_SHA"] or current["run_id"] != int(os.environ["GITHUB_RUN_ID"]):
        raise ValueError("Results do not match the current workflow run")
    repo = os.environ["GITHUB_REPOSITORY"]
    run = previous_run(repo, os.environ["GITHUB_REF_NAME"], current["run_id"])
    previous = baseline(repo, run)
    repository_url = f"{os.environ['GITHUB_SERVER_URL']}/{repo}"
    body = render(current, previous, run, repository_url)
    with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a") as summary:
        summary.write(body)
    pr = os.environ.get("BENCH_PR", "").strip()
    if pr:
        comment(repo, pr, current["commit"], body)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    collect = commands.add_parser("record")
    collect.add_argument("source", type=Path)
    collect.add_argument("destination", type=Path)
    compare = commands.add_parser("report")
    compare.add_argument("source", type=Path)
    args = parser.parse_args()
    if args.command == "record":
        record(args.source, args.destination)
    else:
        report(args.source)


if __name__ == "__main__":
    main()
