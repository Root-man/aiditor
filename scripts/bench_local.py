#!/usr/bin/env python3
"""Run the core benchmarks locally and print a readable timing table."""

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
    lines = ["aiditor core benchmarks", ""]
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


def main():
    try:
        with tempfile.TemporaryDirectory(prefix="aiditor-bench-") as directory:
            results = Path(directory) / "results.tsv"
            completed = subprocess.run(
                ["cargo", "bench", "--locked", "-p", "aiditor-core", "--bench", "core"],
                cwd=ROOT,
                env={**os.environ, "BENCH_OUTPUT": str(results)},
                stdout=subprocess.PIPE,
                text=True,
            )
            if completed.returncode:
                sys.stderr.write(completed.stdout)
                return completed.returncode if completed.returncode > 0 else 128 - completed.returncode
            print(table(read_measurements(results)))
        return 0
    except (OSError, ValueError) as error:
        print(f"Benchmark run failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
