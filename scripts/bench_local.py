#!/usr/bin/env python3
"""Run core/service benchmarks, or an explicit real-window latency benchmark."""

import argparse
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from benchmarks import ROOT, read_measurements


def duration(ns):
    if ns < 1000:
        return f"{ns:.1f} ns"
    if ns < 1_000_000:
        return f"{ns / 1000:.3f} µs"
    return f"{ns / 1_000_000:.3f} ms"


def table(measurements):
    rows = [["Benchmark", "Median", "Minimum", "Maximum"]]
    rows += [
        [name, *(duration(timing[key]) for key in ("median_ns", "min_ns", "max_ns"))]
        for name, timing in measurements.items()
    ]
    widths = [max(len(row[i]) for row in rows) for i in range(4)]
    lines = ["aiditor core and service benchmarks", ""]
    for index, row in enumerate(rows):
        lines.append("  ".join(
            value.ljust(widths[i]) if i == 0 else value.rjust(widths[i])
            for i, value in enumerate(row)
        ))
        if index == 0:
            lines.append("  ".join("-" * width for width in widths))
    lines += [
        "", "Times per operation; lower is faster.",
        "Median/minimum/maximum of 21 warmed-up batch averages, not latency percentiles.",
    ]
    return "\n".join(lines)


def ui_table(source):
    samples = {}
    byte_counts = {}
    for line in source.read_text().splitlines():
        scenario, metric, value = line.split("\t")
        value = int(value)
        if scenario not in ("baseline", "yes", "cat") or value < 0:
            raise ValueError("Invalid GUI benchmark sample")
        if metric == "output-bytes":
            if scenario in byte_counts:
                raise ValueError("Duplicate output byte count")
            byte_counts[scenario] = value
        elif metric in ("input-to-paint", "paint-gap") and value > 0:
            samples.setdefault((scenario, metric), []).append(value)
        else:
            raise ValueError("Invalid GUI benchmark metric")
    lines = ["aiditor GUI benchmark (individual events)", "", "Scenario / metric | Samples | p50 | p95 | p99 | Max"]
    for scenario in ("baseline", "yes", "cat"):
        if scenario not in byte_counts or (scenario != "baseline" and byte_counts[scenario] == 0):
            raise ValueError(f"Missing terminal flood output: {scenario}")
        for metric in ("input-to-paint", "paint-gap"):
            values = sorted(samples.get((scenario, metric), []))
            if (metric == "input-to-paint" and len(values) != 120) or not values:
                raise ValueError(f"Incomplete GUI benchmark: {scenario}/{metric}")
            quantile = lambda q: duration(values[math.ceil(q * len(values)) - 1])
            lines.append(f"{scenario} / {metric} | {len(values)} | {quantile(.5)} | {quantile(.95)} | {quantile(.99)} | {duration(values[-1])}")
        lines.append(f"{scenario}: {byte_counts[scenario]} terminal bytes processed during measurement")
    lines += ["", "Synthetic scheduled input to matching frame paint; excludes OS keyboard delivery and display scanout.",
              "Paint gaps include the input cadence and idle time; these are not GPU frame durations."]
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", choices=("all", "core", "services", "ui"), default="all")
    parser.add_argument("--output", type=Path, help="retain raw TSV results (GUI samples have a different schema)")
    args = parser.parse_args(argv)
    try:
        with tempfile.TemporaryDirectory(prefix="aiditor-bench-") as directory:
            directory = Path(directory)
            combined = ""
            suites = ("core", "services") if args.suite == "all" else (args.suite,)
            for suite in suites:
                results = directory / f"{suite}.tsv"
                command = (["cargo", "run", "--locked", "--release", "-p", "aiditor", "--", "--bench-ui", str(results)]
                           if suite == "ui" else
                           ["cargo", "bench", "--locked", "-p", f"aiditor-{suite}", "--bench", suite])
                completed = subprocess.run(command, cwd=ROOT,
                    env={**os.environ, "BENCH_OUTPUT": str(results)},
                    stdout=subprocess.PIPE, text=True)
                if completed.returncode:
                    sys.stderr.write(completed.stdout)
                    return completed.returncode if completed.returncode > 0 else 128 - completed.returncode
                combined += results.read_text()
            results = directory / "combined.tsv"
            results.write_text(combined)
            report = ui_table(results) if args.suite == "ui" else table(read_measurements(results))
            if args.output:
                args.output.write_text(combined)
            print(report)
        return 0
    except (OSError, ValueError) as error:
        print(f"Benchmark run failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
