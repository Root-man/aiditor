import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import benchmarks


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


if __name__ == "__main__":
    unittest.main()
