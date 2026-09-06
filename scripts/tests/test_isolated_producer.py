"""Unit controls; opt-in live Linux qualification uses only disposable fixtures."""
import argparse
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("isolated_producer", ROOT / "scripts/isolated_producer.py")
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class UnitControls(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="adrproof-runner-unit-")
        self.root = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

    def test_tree_binds_content_names_modes_and_empty_directories(self):
        source = self.root / "source"
        source.mkdir()
        original = runner.tree_digest(source)
        (source / "empty").mkdir()
        self.assertNotEqual(original, runner.tree_digest(source))
        (source / "value").write_text("one")
        original = runner.tree_digest(source)
        (source / "value").write_text("two")
        self.assertNotEqual(original, runner.tree_digest(source))
        original = runner.tree_digest(source)
        (source / "value").chmod(0o700)
        self.assertNotEqual(original, runner.tree_digest(source))

    def test_source_alias_and_exclusion_are_errors(self):
        source = self.root / "source"
        source.mkdir()
        (source / "alias").symlink_to(source)
        with self.assertRaises(ValueError):
            runner.tree_digest(source)
        (source / "alias").unlink()
        (source / "target").mkdir()
        with self.assertRaises(ValueError):
            runner.tree_digest(source)

    def transfer(self):
        snapshot = self.root / "snapshot.json"
        snapshot.write_text('{"synthetic":"payload"}')
        values = dict(run_id="test-run-1", profile_sha256="a"*64, project_sha256="b"*64, spec_sha256="c"*64)
        receipt = dict(schema_version=runner.RECEIPT, snapshot_sha256=runner.file_sha(snapshot),
                       result="CAPTURED_NOT_PROOF", **values)
        path = self.root / "receipt.json"
        path.write_bytes(runner.canonical(receipt))
        return argparse.Namespace(snapshot=snapshot, receipt=path, receipt_sha256=runner.file_sha(path), **values)

    def test_transfer_rejects_payload_tampering_and_cross_run_replay(self):
        args = self.transfer()
        self.assertEqual(runner.verify_transfer(args)["result"], "TRANSFER_VERIFIED_NOT_PROOF")
        args.run_id = "different-run"
        with self.assertRaises(ValueError):
            runner.verify_transfer(args)
        args.run_id = "test-run-1"
        args.snapshot.write_text("tampered")
        with self.assertRaises(ValueError):
            runner.verify_transfer(args)

    def test_receipt_substitution_and_wrong_expected_context_fail(self):
        args = self.transfer()
        args.profile_sha256 = "d"*64
        with self.assertRaises(ValueError):
            runner.verify_transfer(args)
        args.receipt.write_text("{}")
        with self.assertRaisesRegex(ValueError, "PIN_MISMATCH"):
            runner.verify_transfer(args)

    def test_changed_profile_rejected_before_execution(self):
        path = self.root / "profile.json"
        path.write_text('{"policy":"approved"}')
        args = argparse.Namespace(profile=path, profile_sha256=runner.file_sha(path), runtime=self.root, bwrap="/unused")
        with patch.object(runner, "describe", return_value={"policy": "changed"}), patch.object(runner, "execute") as execute:
            with self.assertRaisesRegex(ValueError, "PROFILE_DRIFT"):
                runner.check_profile(args)
            execute.assert_not_called()

    def test_duplicate_fields_in_pinned_json_are_rejected(self):
        path = self.root / "ambiguous.json"
        path.write_text('{"policy": "first", "policy": "second"}')
        with self.assertRaisesRegex(ValueError, "duplicate JSON field"):
            runner.read_pinned(path, runner.file_sha(path))

    def test_source_pin_rejected_before_provider_and_no_artifacts(self):
        project, specification, runtime = [self.root / n for n in ("project", "spec", "runtime")]
        for path in (project, specification, runtime):
            path.mkdir()
        args = argparse.Namespace(runtime=runtime, project=project, spec=specification, output=self.root / "output",
                                  project_sha256="0"*64, spec_sha256=runner.tree_digest(specification), run_id="unit-1")
        with patch.object(runner, "check_profile", return_value={}), patch.object(runner, "execute") as execute:
            with self.assertRaisesRegex(ValueError, "SOURCE_PIN_MISMATCH"):
                runner.produce(args)
            execute.assert_not_called()
        self.assertFalse(args.output.exists())

    def test_nonzero_capture_never_publishes(self):
        project, specification, runtime = [self.root / n for n in ("project", "spec", "runtime")]
        for path in (project, specification, runtime):
            path.mkdir()
        args = argparse.Namespace(runtime=runtime, project=project, spec=specification, output=self.root / "output",
                                  project_sha256=runner.tree_digest(project), spec_sha256=runner.tree_digest(specification),
                                  profile_sha256="a"*64, run_id="unit-2", bwrap=sys.executable)
        with patch.object(runner, "check_profile", return_value={}), patch.object(runner, "command", return_value=[]), \
             patch.object(runner, "execute", side_effect=ValueError("PRODUCER_FAILED")):
            with self.assertRaisesRegex(ValueError, "PRODUCER_FAILED"):
                runner.produce(args)
        self.assertFalse(args.output.exists())

    def test_namespace_policy_has_no_host_bind_or_relaxation(self):
        args = argparse.Namespace(runtime=self.root, bwrap=sys.executable)
        cmd = runner.command(args, self.root, self.root, ["/tools/adrproof"])
        for option in ("--unshare-user", "--unshare-pid", "--unshare-net", "--unshare-ipc", "--unshare-uts", "--disable-userns", "--assert-userns-disabled", "--clearenv", "--new-session"):
            self.assertIn(option, cmd)
        for option in ("--share-net", "--bind", "--unshare-user-try", "--not-a-security-boundary"):
            self.assertNotIn(option, cmd)


def fixture_runtime(destination):
    """Copies installed TRUSTED tools for a local test, never candidate binaries."""
    for path in ("project", "spec", "state", "tmp", "proc", "dev", "tools", "usr/bin", "lib", "lib64", "usr/lib"):
        (destination / path).mkdir(parents=True, exist_ok=True)
    (destination / "probe.py").write_text("")
    sysroot = Path(subprocess.check_output(["rustc", "--print", "sysroot"], text=True).strip())
    python = Path(sys.executable).resolve()
    tools = [(ROOT / "target/debug/adrproof", "tools/adrproof"),
             (sysroot / "bin/cargo", "tools/cargo"), (sysroot / "bin/rustc", "tools/rustc"),
             (python, "usr/bin/python3"), (Path("/usr/bin/env"), "usr/bin/env")]
    scanned = set()

    def dependencies(path):
        resolved = path.resolve()
        if resolved in scanned:
            return
        scanned.add(resolved)
        # ldd is restricted to installed trusted tools/libraries, not PR input.
        output = subprocess.check_output(["ldd", str(resolved)], text=True)
        for match in re.findall(r"/[^\s()]+", output):
            source = Path(match)
            require_path = destination / source.relative_to("/")
            # Rust's relative RPATH is /tools/../lib in the runtime.
            if source.is_relative_to(sysroot):
                require_path = destination / "lib" / source.name
            require_path.parent.mkdir(parents=True, exist_ok=True)
            if not require_path.exists():
                shutil.copy2(source, require_path)
            dependencies(source)

    for source, name in tools:
        shutil.copy2(source, destination / name)
        dependencies(source)
    stdlib = Path(subprocess.check_output([str(python), "-I", "-c", "import sysconfig; print(sysconfig.get_path('stdlib'))"], text=True).strip())
    target = destination / "usr/lib" / stdlib.name
    shutil.copytree(stdlib, target, ignore=shutil.ignore_patterns("__pycache__", "site-packages", "test", "tests"))
    for extension in target.rglob("*.so"):
        dependencies(stdlib / extension.relative_to(target))


def tracked_example(relative, destination):
    """Export tracked neutral fixture files, never local ignored evidence/builds."""
    destination.mkdir()
    files = subprocess.check_output(["git", "-C", str(ROOT), "ls-files", "-z", "--", relative]).split(b"\0")
    for value in files:
        if value:
            source = Path(os.fsdecode(value))
            target = destination / source.relative_to(relative)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT / source, target)


@unittest.skipUnless(os.environ.get("ADRPROOF_RUN_BWRAP_TESTS") == "1", "live namespaces require explicit opt-in; not qualified by unit tests")
class LiveQualification(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="adrproof-runner-live-")
        cls.root = Path(cls.temporary.name)
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.runtime = cls.root / "runtime"
        fixture_runtime(cls.runtime)
        cls.profile = cls.root / "profile.json"
        cls.profile.write_bytes(runner.canonical(runner.describe(cls.runtime, "/usr/bin/bwrap")))
        cls.args = argparse.Namespace(runtime=cls.runtime, bwrap="/usr/bin/bwrap", profile=cls.profile,
                                      profile_sha256=runner.file_sha(cls.profile))

    def test_actual_namespace_boundary_and_timeout(self):
        result = runner.qualify(self.args)
        self.assertTrue(all(result["checks"].values()))
        print(json.dumps(result, sort_keys=True))

    def test_actual_snapshot_and_protected_transfer(self):
        project, specification = self.root / "project", self.root / "spec"
        tracked_example("examples/external-provider/project", project)
        tracked_example("examples/external-provider/spec", specification)
        args = argparse.Namespace(**vars(self.args), project=project, spec=specification,
                                  project_sha256=runner.tree_digest(project), spec_sha256=runner.tree_digest(specification),
                                  output=self.root / "output", run_id="synthetic-live-1")
        result = runner.produce(args)
        transfer = argparse.Namespace(receipt=args.output / "receipt.json", receipt_sha256=result["receipt_sha256"],
                                      snapshot=args.output / "snapshot.json", run_id=args.run_id,
                                      profile_sha256=args.profile_sha256, project_sha256=args.project_sha256, spec_sha256=args.spec_sha256)
        self.assertEqual(runner.verify_transfer(transfer)["result"], "TRANSFER_VERIFIED_NOT_PROOF")
        # Read-only core validates the externally transferred snapshot with no provider execution.
        output = subprocess.run([str(ROOT / "target/debug/adrproof"), "gate", "prepare-snapshot", "--project-root", str(project),
                                 "--spec-root", str(specification), "--state-root", str(self.root / "review-state"),
                                 "--snapshot", str(transfer.snapshot), "--snapshot-sha256", result["snapshot_sha256"],
                                 "--backend-version", "synthetic", "--timeout-ms", "1000", "--json"], capture_output=True, text=True)
        # Neutral example has no reviewed inventory: fact transport must not manufacture approval.
        self.assertEqual(output.returncode, 2)
        self.assertNotIn("SNAPSHOT_STALE", output.stdout)
        self.assertIn("requirements.json", output.stdout.lower())
        print(json.dumps({"result": result["result"], "transfer": "verified", "missing_inventory_gate": "rejected"}))

    def test_actual_cargo_metadata_inside_runtime(self):
        project, specification = self.root / "cargo-project", self.root / "cargo-spec"
        tracked_example("examples/rust-workspace-architecture", project)
        specification.mkdir()
        (specification / "architecture.md").write_text('---\nid: RUNNER-1\nstatus: accepted\n---\n\n```adrlogic\nbool boundary;\nrule C1 "boundary" { boundary; }\n```\n')
        args = argparse.Namespace(**vars(self.args), project=project, spec=specification,
                                  project_sha256=runner.tree_digest(project), spec_sha256=runner.tree_digest(specification),
                                  output=self.root / "cargo-output", run_id="synthetic-cargo-1")
        result = runner.produce(args)
        snapshot = json.loads((args.output / "snapshot.json").read_bytes())
        self.assertTrue(any(fact["relation"] == "package" for fact in snapshot["model"]["facts"].values()))
        self.assertEqual(result["result"], "CAPTURED_NOT_PROOF")
        print(json.dumps({"cargo_metadata": "captured_in_namespace", "project_digest_unchanged": True}))

    def test_modified_runtime_is_rejected(self):
        marker = self.runtime / "runtime-drift.txt"
        marker.write_text("synthetic drift")
        try:
            with self.assertRaisesRegex(ValueError, "PROFILE_DRIFT"):
                runner.check_profile(self.args)
        finally:
            marker.unlink()


if __name__ == "__main__":
    unittest.main()
