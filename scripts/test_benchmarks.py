from contextlib import redirect_stderr, redirect_stdout
import io
import json
import os
import subprocess
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import benchmarks
import bench_local


def result():
    return {
        "version": 1,
        "commit": "a" * 40,
        "run_id": 20,
        "environment": dict(runner="ubuntu-24.04", arch="x86_64", cpu="cpu", rustc="rustc 1.99.0", suite="v1"),
        "benchmarks": {"edit-undo/100k-lines": dict(min_ns=900, median_ns=1000, max_ns=1100)},
    }


class BenchmarkReporting(unittest.TestCase):
    def test_reports_current_results_without_baseline(self):
        report = benchmarks.render(result(), None, None, "https://github.com/owner/repo")
        self.assertIn("establishes a baseline", report)
        self.assertIn("| `edit-undo/100k-lines` | — | 1.000 | — | 0.900–1.100 |", report)
        self.assertIn("https://github.com/owner/repo/actions/runs/20", report)

    def test_reports_percentage_changes_in_both_directions(self):
        for median, expected in [(1250, "+25.0%"), (750, "-25.0%")]:
            with self.subTest(median=median):
                current = result()
                current["benchmarks"]["edit-undo/100k-lines"] = dict(min_ns=500, median_ns=median, max_ns=2000)
                previous = result()
                previous["run_id"] = 10
                report = benchmarks.render(current, previous, {"id": 10}, "https://github.com/owner/repo")
                self.assertIn(expected, report)
                self.assertIn("actions/runs/10", report)

    def test_changed_environment_starts_a_new_baseline(self):
        for key in result()["environment"]:
            with self.subTest(key=key):
                previous = result()
                previous["environment"][key] = "changed"
                report = benchmarks.render(result(), previous, {"id": 10}, "https://github.com/owner/repo")
                self.assertIn("establishes a new baseline", report)
                self.assertIn("| `edit-undo/100k-lines` | — | 1.000 | — |", report)

    def test_changed_benchmarks_start_a_new_baseline(self):
        previous = result()
        previous["benchmarks"] = {"old": dict(min_ns=900, median_ns=1000, max_ns=1100)}
        report = benchmarks.render(result(), previous, {"id": 10}, "https://github.com/owner/repo")
        self.assertIn("establishes a new baseline", report)

    def test_validates_results_before_reporting(self):
        self.assertEqual(benchmarks.validate(result()), result())
        for invalid in [0, -1, float("nan"), float("inf"), True, "1000"]:
            with self.subTest(invalid=invalid):
                data = result()
                data["benchmarks"]["edit-undo/100k-lines"]["median_ns"] = invalid
                with self.assertRaises(ValueError):
                    benchmarks.validate(data)
        data = result()
        data["benchmarks"]["edit-undo/100k-lines"]["min_ns"] = 2000
        with self.assertRaises(ValueError):
            benchmarks.validate(data)

    def test_fingerprint_tracks_shared_helpers_and_both_suites(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = ["Cargo.toml", "scripts/bench_local.py", "benches/support.rs",
                     "crates/aiditor-core/Cargo.toml", "crates/aiditor-core/benches/core.rs",
                     "crates/aiditor-core/benches/scenarios/mod.rs",
                     "crates/aiditor-services/Cargo.toml", "crates/aiditor-services/benches/services.rs"]
            for name in files:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(name)
            with patch.object(benchmarks, "ROOT", root):
                before = benchmarks.suite_fingerprint()
                for name in files:
                    path = root / name
                    path.write_text(name + " changed")
                    self.assertNotEqual(benchmarks.suite_fingerprint(), before, name)
                    path.write_text(name)
                self.assertEqual(benchmarks.suite_fingerprint(), before)

    @patch("benchmarks.subprocess.check_output", return_value="rustc 1.99.0")
    def test_records_tsv_with_environment_and_rejects_duplicates(self, rustc):
        cpuinfo = "processor: 0\nmodel name: Test CPU\n"
        read_text = Path.read_text

        def contents(path, *args, **kwargs):
            return cpuinfo if str(path) == "/proc/cpuinfo" else read_text(path, *args, **kwargs)

        environment = dict(GITHUB_SHA="a" * 40, GITHUB_RUN_ID="20", BENCH_RUNNER="ubuntu-24.04")
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, environment):
            source = Path(directory) / "benchmarks.tsv"
            destination = Path(directory) / "results.json"
            source.write_text("edit-undo/100k-lines\t1000\t900\t1100\n")
            with patch.object(Path, "read_text", contents):
                benchmarks.record(source, destination)
                data = json.loads(destination.read_text())
                self.assertEqual(data["benchmarks"], result()["benchmarks"])
                self.assertEqual(data["environment"]["cpu"], "Test CPU")
                self.assertEqual(len(data["environment"]["suite"]), 64)
                source.write_text(source.read_text() * 2)
                with self.assertRaisesRegex(ValueError, "Duplicate benchmark"):
                    benchmarks.record(source, destination)

    @patch("benchmarks.api")
    def test_selects_previous_run_across_pages_excluding_current_and_later_runs(self, api):
        api.return_value = [
            {"workflow_runs": [{"id": 30}, {"id": 20}, {"id": 5}]},
            {"workflow_runs": [{"id": 10}]},
        ]
        self.assertEqual(benchmarks.previous_run("owner/repo", "feature/bench", 20), {"id": 10})
        endpoint = api.call_args.args[0]
        self.assertIn("branch=feature%2Fbench", endpoint)
        self.assertIn("status=success", endpoint)
        self.assertIn("event=workflow_dispatch", endpoint)
        api.return_value = [{"workflow_runs": []}]
        self.assertIsNone(benchmarks.previous_run("owner/repo", "main", 20))

    @patch("benchmarks.gh")
    @patch("benchmarks.api")
    def test_reads_results_from_previous_artifact(self, api, gh):
        api.return_value = [{"artifacts": [{"id": 4, "name": "core-benchmarks", "expired": False}]}]
        archive = io.BytesIO()
        with zipfile.ZipFile(archive, "w") as zipped:
            zipped.writestr("results.json", json.dumps(result()))
        gh.return_value = archive.getvalue()
        self.assertEqual(benchmarks.baseline("owner/repo", {"id": 20, "head_sha": "a" * 40}), result())
        with self.assertRaises(ValueError):
            benchmarks.baseline("owner/repo", {"id": 10, "head_sha": "a" * 40})
        api.return_value[0]["artifacts"][0]["expired"] = True
        self.assertIsNone(benchmarks.baseline("owner/repo", {"id": 20}))

    @patch("benchmarks.comment")
    @patch("benchmarks.baseline", return_value=None)
    @patch("benchmarks.previous_run", return_value=None)
    def test_report_saves_summary_and_uses_optional_pr(self, previous, baseline, comment):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "results.json"
            summary = Path(directory) / "summary.md"
            source.write_text(json.dumps(result()))
            environment = dict(
                GITHUB_SHA="a" * 40, GITHUB_RUN_ID="20", GITHUB_REPOSITORY="owner/repo",
                GITHUB_REF_NAME="feature/bench", GITHUB_SERVER_URL="https://github.com",
                GITHUB_STEP_SUMMARY=str(summary), BENCH_PR="",
            )
            with patch.dict(os.environ, environment):
                benchmarks.report(source)
                self.assertIn("establishes a baseline", summary.read_text())
                self.assertEqual(comment.call_count, 0)
                os.environ["BENCH_PR"] = "12"
                benchmarks.report(source)
                comment.assert_called_once_with(
                    "owner/repo", "12", "a" * 40,
                    benchmarks.render(result(), None, None, "https://github.com/owner/repo"),
                )

    @patch("benchmarks.api")
    def test_posts_report_to_matching_open_pr(self, api):
        api.return_value = dict(state="open", head=dict(repo=dict(full_name="owner/repo"), sha="a" * 40))
        benchmarks.comment("owner/repo", "12", "a" * 40, "Benchmark report")
        api.assert_called_with(
            "repos/owner/repo/issues/12/comments", "--method", "POST", "--input", "-",
            payload={"body": "Benchmark report"},
        )

    @patch("benchmarks.api")
    def test_rejects_stale_closed_fork_and_invalid_pr_targets(self, api):
        pull = dict(state="open", head=dict(repo=dict(full_name="owner/repo"), sha="a" * 40))
        invalid = [
            dict(state="closed", head=pull["head"]),
            dict(state="open", head=dict(repo=None, sha="a" * 40)),
            dict(state="open", head=dict(repo=dict(full_name="fork/repo"), sha="a" * 40)),
            dict(state="open", head=dict(repo=dict(full_name="owner/repo"), sha="b" * 40)),
        ]
        for value in invalid:
            with self.subTest(pull=value):
                api.reset_mock()
                api.return_value = value
                with self.assertRaises(ValueError):
                    benchmarks.comment("owner/repo", "12", "a" * 40, "report")
                self.assertEqual(api.call_count, 1)
        with self.assertRaises(ValueError):
            benchmarks.comment("owner/repo", "12; echo unsafe", "a" * 40, "report")


class LocalBenchmarks(unittest.TestCase):
    def test_duration_uses_readable_units(self):
        for value, expected in [(355, "355.0 ns"), (1000, "1.000 µs"), (134638, "134.638 µs"), (1_000_000, "1.000 ms")]:
            with self.subTest(value=value):
                self.assertEqual(bench_local.duration(value), expected)

    def test_table_aligns_columns_and_reports_all_results(self):
        measurements = {
            "edit-undo/100k-lines": dict(min_ns=200, median_ns=237, max_ns=482),
            "display/50-rows-in-100k-lines": dict(min_ns=133000, median_ns=134638, max_ns=136000),
            "swiper/two-queries-in-100k-lines": dict(min_ns=10490000, median_ns=10650000, max_ns=10860000),
        }
        output = bench_local.table(measurements)
        lines = output.splitlines()
        self.assertIn("237.0 ns", lines[4])
        self.assertIn("134.638 µs", lines[5])
        self.assertIn("10.650 ms", lines[6])
        self.assertEqual(len(lines[2]), len(lines[4]))
        self.assertEqual(len(lines[4]), len(lines[5]))
        self.assertEqual(len(lines[5]), len(lines[6]))
        self.assertIn("21 warmed-up batch averages", output)

    def test_reads_tsv_and_preserves_result_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "results.tsv"
            source.write_text("edit-undo/100k-lines\t1000\t900\t1100\n")
            self.assertEqual(benchmarks.read_measurements(source), result()["benchmarks"])
            for invalid in ["", "bad row", "edit-undo/100k-lines\tnan\t900\t1100\n", source.read_text() * 2]:
                with self.subTest(invalid=invalid):
                    source.write_text(invalid)
                    with self.assertRaises(ValueError):
                        benchmarks.read_measurements(source)

    @patch("bench_local.subprocess.run")
    def test_runs_both_suites_and_retains_combined_results(self, run):
        def cargo(command, **kwargs):
            suite = command[-1]
            self.assertEqual(command, ["cargo", "bench", "--locked", "-p", f"aiditor-{suite}", "--bench", suite])
            self.assertEqual(kwargs["cwd"], benchmarks.ROOT)
            self.assertEqual(kwargs["env"]["BENCH_LOCAL_TEST"], "inherited")
            Path(kwargs["env"]["BENCH_OUTPUT"]).write_text(f"{suite}/example\t1000\t900\t1100\n")
            return subprocess.CompletedProcess(command, 0, stdout="raw benchmark output\n")

        run.side_effect = cargo
        output = io.StringIO()
        with patch.dict(os.environ, BENCH_LOCAL_TEST="inherited"), redirect_stdout(output):
            self.assertEqual(bench_local.main([]), 0)
        self.assertIn("aiditor core and service benchmarks", output.getvalue())
        self.assertIn("core/example", output.getvalue())
        self.assertIn("services/example", output.getvalue())
        self.assertEqual(run.call_count, 2)
        self.assertIn("1.000 µs", output.getvalue())

    def test_gui_percentiles_use_individual_samples_and_require_actual_floods(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ui.tsv"
            rows = []
            for scenario in ("baseline", "yes", "cat"):
                for metric in ("input-to-paint", "paint-gap"):
                    rows += [f"{scenario}\t{metric}\t{i * 1000}\n" for i in range(1, 121)]
                rows.append(f"{scenario}\toutput-bytes\t{0 if scenario == 'baseline' else 65536}\n")
            path.write_text("".join(rows))
            report = bench_local.ui_table(path)
            self.assertIn("baseline / input-to-paint | 120 | 60.000 µs | 114.000 µs | 119.000 µs | 120.000 µs", report)
            path.write_text("".join(rows).replace("yes\toutput-bytes\t65536", "yes\toutput-bytes\t0"))
            with self.assertRaisesRegex(ValueError, "Missing terminal flood output"):
                bench_local.ui_table(path)
            path.write_text("".join(rows[1:]))
            with self.assertRaisesRegex(ValueError, "Incomplete GUI benchmark"):
                bench_local.ui_table(path)

    @patch("bench_local.subprocess.run")
    def test_gui_run_uses_its_own_report_schema(self, run):
        def cargo(command, **kwargs):
            self.assertEqual(command[:8], ["cargo", "run", "--locked", "--release", "-p", "aiditor", "--", "--bench-ui"])
            rows = []
            for scenario in ("baseline", "yes", "cat"):
                for metric in ("input-to-paint", "paint-gap"):
                    rows += [f"{scenario}\t{metric}\t1000000\n"] * 120
                rows += [f"{scenario}\toutput-bytes\t65536\n"]
            Path(command[-1]).write_text("".join(rows))
            return subprocess.CompletedProcess(command, 0, stdout="")
        run.side_effect = cargo
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()) as report:
            output = Path(directory) / "samples.tsv"
            self.assertEqual(bench_local.main(["--suite", "ui", "--output", str(output)]), 0)
            self.assertIn("yes / input-to-paint | 120", report.getvalue())
            self.assertIn("cat\tpaint-gap\t1000000", output.read_text())

    @patch("bench_local.subprocess.run")
    def test_core_only_output_can_be_read_by_ci(self, run):
        def cargo(command, **kwargs):
            Path(kwargs["env"]["BENCH_OUTPUT"]).write_text("core/example\t1000\t900\t1100\n")
            return subprocess.CompletedProcess(command, 0, stdout="")
        run.side_effect = cargo
        with tempfile.TemporaryDirectory() as directory, redirect_stdout(io.StringIO()):
            output = Path(directory) / "results.tsv"
            self.assertEqual(bench_local.main(["--suite", "core", "--output", str(output)]), 0)
            self.assertEqual(benchmarks.read_measurements(output)["core/example"]["median_ns"], 1000)
            self.assertEqual(run.call_count, 1)

    @patch("bench_local.subprocess.run")
    def test_cargo_failure_preserves_exit_status_and_diagnostics(self, run):
        for returncode, expected in [(101, 101), (-9, 137)]:
            with self.subTest(returncode=returncode):
                run.return_value = subprocess.CompletedProcess(["cargo"], returncode, stdout="benchmark failed\n")
                error = io.StringIO()
                with redirect_stderr(error):
                    self.assertEqual(bench_local.main([]), expected)
                self.assertIn("benchmark failed", error.getvalue())

    @patch("bench_local.subprocess.run", side_effect=FileNotFoundError("cargo unavailable"))
    def test_missing_cargo_has_readable_error(self, run):
        error = io.StringIO()
        with redirect_stderr(error):
            self.assertEqual(bench_local.main([]), 1)
        self.assertIn("Benchmark run failed: cargo unavailable", error.getvalue())


if __name__ == "__main__":
    unittest.main()
