#!/usr/bin/env python3
"""Neutral Linux producer/transport harness. See docs/ISOLATED_PRODUCER.md."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import resource
import signal
import socket
import subprocess
import sys
import tempfile
from pathlib import Path

PROFILE = "adrproof-linux-producer-profile-v1alpha1"
RECEIPT = "adrproof-producer-receipt-v1alpha1"
HERE = Path(__file__).resolve().parent
MAX_OUTPUT = 64 * 1024 * 1024
POLICY = {
    "wall_seconds": 30, "cpu_seconds": 20, "address_bytes": 2 * 1024**3,
    "output_bytes": MAX_OUTPUT, "tmpfs_bytes": 128 * 1024**2,
    "namespaces": ["mount", "user", "pid", "ipc", "net", "uts"], "uid": 1000, "gid": 1000,
    "capabilities": "none", "nested_userns": "disabled",
    "network": "private", "source_mounts": "read-only", "command": "snapshot capture",
    "environment": {"PATH": "/tools:/usr/bin", "LANG": "C", "LC_ALL": "C",
                    "TZ": "UTC", "CARGO_HOME": "/tmp/cargo", "RUSTC": "/tools/rustc",
                    "CARGO_NET_OFFLINE": "true", "PYTHONDONTWRITEBYTECODE": "1"},
}


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def valid_pin(value):
    require(re.fullmatch(r"[0-9a-f]{64}", value) is not None, "invalid SHA-256 pin")
    return value


def tree_digest(root, runtime=False):
    root = Path(root).resolve(strict=True)
    require(root.is_dir(), "tree root must be a directory")
    entries = []

    def visit(path, depth):
        require(depth <= 128 and len(entries) < 100_000, "tree limit exceeded")
        relative = path.relative_to(root).as_posix()
        info = path.lstat()
        if path.is_symlink():
            require(runtime, "source aliases are unsupported")
            kind, digest = "symlink", sha(os.readlink(path).encode())
        elif path.is_file():
            kind, digest = "file", file_sha(path)
        else:
            require(path.is_dir(), "special files are unsupported")
            kind, digest = "directory", sha(b"")
        if not runtime:
            require(path.name not in (".git", "target", ".adrproof"), "source export contains excluded state/build metadata")
        entries.append([relative, kind, info.st_mode & 0o7777, digest])
        if kind == "directory":
            for child in sorted(path.iterdir()):
                visit(child, depth + 1)
    visit(root, 0)
    return sha(canonical(entries))


def describe(runtime, bwrap):
    require(platform.system() == "Linux", "Linux required; no fallback")
    runtime, bwrap = Path(runtime).resolve(strict=True), Path(bwrap).resolve(strict=True)
    for name in ("tools/adrproof", "tools/cargo", "tools/rustc", "usr/bin/python3"):
        require((runtime / name).is_file() and not (runtime / name).is_symlink()
                and os.access(runtime / name, os.X_OK), f"runtime lacks plain executable {name}")
    require((runtime / "probe.py").is_file() and not (runtime / "probe.py").is_symlink()
            and (runtime / "probe.py").stat().st_size == 0, "runtime requires an empty plain /probe.py mount point")
    for name in ("project", "spec", "state", "tmp", "proc", "dev"):
        require((runtime / name).is_dir() and not (runtime / name).is_symlink()
                and not any((runtime / name).iterdir()), f"runtime mount point {name} must be empty and plain")
    return {"schema_version": PROFILE, "runtime_sha256": tree_digest(runtime, runtime=True),
            "supervisor_sha256": file_sha(__file__), "probe_sha256": file_sha(HERE / "producer_probe.py"),
            "python_sha256": file_sha(sys.executable), "bwrap_sha256": file_sha(bwrap),
            "kernel_release": platform.release(), "policy": POLICY}


def read_pinned(path, pin):
    valid_pin(pin)
    data = Path(path).read_bytes()
    require(sha(data) == pin, "PIN_MISMATCH")
    return decode(data)


def decode(data):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate JSON field")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def check_profile(args):
    profile = read_pinned(args.profile, args.profile_sha256)
    require(canonical(profile) == canonical(describe(args.runtime, args.bwrap)), "PROFILE_DRIFT: runtime, supervisor, kernel or policy changed")
    return profile


def limits():
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    for name, value in ((resource.RLIMIT_CPU, POLICY["cpu_seconds"]),
                        (resource.RLIMIT_AS, POLICY["address_bytes"]),
                        (resource.RLIMIT_FSIZE, POLICY["output_bytes"])):
        resource.setrlimit(name, (value, value))


def command(args, project, spec, payload, probe=False):
    cmd = [str(Path(args.bwrap).resolve(strict=True)), "--unshare-user", "--unshare-pid", "--unshare-ipc", "--unshare-net", "--unshare-uts",
           "--uid", "1000", "--gid", "1000", "--die-with-parent", "--new-session",
           "--cap-drop", "ALL", "--disable-userns", "--assert-userns-disabled", "--clearenv",
           "--hostname", "adrproof-producer", "--ro-bind", str(Path(args.runtime).resolve(strict=True)), "/",
           "--ro-bind", str(Path(project).resolve(strict=True)), "/project",
           "--ro-bind", str(Path(spec).resolve(strict=True)), "/spec",
           "--size", str(POLICY["tmpfs_bytes"]), "--tmpfs", "/state",
           "--size", str(POLICY["tmpfs_bytes"]), "--tmpfs", "/tmp",
           "--proc", "/proc", "--dev", "/dev", "--chdir", "/project"]
    if probe:
        cmd += ["--ro-bind", str(HERE / "producer_probe.py"), "/probe.py"]
    for key, value in sorted(POLICY["environment"].items()):
        cmd += ["--setenv", key, value]
    return cmd + ["--"] + payload


def execute(cmd, timeout=None):
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        proc = subprocess.Popen(cmd, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                env={"ADRPROOF_PROBE_SECRET": "synthetic-only"},
                                close_fds=True, start_new_session=True, preexec_fn=limits)
        try:
            code = proc.wait(timeout=timeout or POLICY["wall_seconds"])
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait()
            raise ValueError("RUNNER_TIMEOUT") from None
        stdout.seek(0)
        stderr.seek(0)
        output, errors = stdout.read(MAX_OUTPUT + 1), stderr.read(MAX_OUTPUT + 1)
        require(len(output) <= MAX_OUTPUT and len(errors) <= MAX_OUTPUT, "RUNNER_OUTPUT_LIMIT")
        require(code == 0, f"PRODUCER_FAILED ({code}): stderr={errors[-2000:].decode(errors='replace')} stdout={output[-4000:].decode(errors='replace')}")
        return output


def separated(*paths):
    paths = [Path(p).resolve() for p in paths]
    for i, first in enumerate(paths):
        for second in paths[i+1:]:
            require(not first.is_relative_to(second) and not second.is_relative_to(first), "overlapping runner paths")


def produce(args):
    profile = check_profile(args)
    require(re.fullmatch(r"[A-Za-z0-9_.:-]{1,160}", args.run_id) is not None, "invalid run ID")
    separated(args.runtime, args.project, args.spec, args.output)
    require(not Path(args.output).exists(), "output must be a new directory")
    for path, pin in ((args.project, args.project_sha256), (args.spec, args.spec_sha256)):
        require(tree_digest(path) == valid_pin(pin), "SOURCE_PIN_MISMATCH")
    payload = ["/tools/adrproof", "snapshot", "capture", "--project-root", "/project", "--spec-root", "/spec",
               "--state-root", "/state", "--producer-context-sha256", args.profile_sha256, "--json"]
    output = execute(command(args, args.project, args.spec, payload))
    require(describe(args.runtime, args.bwrap) == profile, "PROFILE_DRIFT after execution")
    require(tree_digest(args.project) == args.project_sha256 and tree_digest(args.spec) == args.spec_sha256,
            "SOURCE_DRIFT after execution")
    snapshot = decode(output)
    require(snapshot.get("schema_version") == "adrproof-fact-snapshot-v1alpha1"
            and snapshot.get("producer_context_sha256") == args.profile_sha256
            and isinstance(snapshot.get("model"), dict) and snapshot.get("semantic_inputs")
            and snapshot.get("tree") and snapshot.get("provider_policy"), "INVALID_CAPTURE_OUTPUT")
    receipt = {"schema_version": RECEIPT, "run_id": args.run_id, "profile_sha256": args.profile_sha256,
               "project_sha256": args.project_sha256, "spec_sha256": args.spec_sha256,
               "snapshot_sha256": sha(output), "result": "CAPTURED_NOT_PROOF"}
    directory = Path(args.output)
    directory.mkdir(mode=0o700)
    # No overwrite/reuse: interrupted publication has no trusted receipt pin.
    with (directory / "snapshot.json").open("xb") as stream:
        stream.write(output)
    data = canonical(receipt)
    with (directory / "receipt.json").open("xb") as stream:
        stream.write(data)
    return {"receipt_sha256": sha(data), "snapshot_sha256": receipt["snapshot_sha256"], "result": "CAPTURED_NOT_PROOF"}


def verify_transfer(args):
    receipt = read_pinned(args.receipt, args.receipt_sha256)
    expected = {"schema_version": RECEIPT, "run_id": args.run_id, "profile_sha256": valid_pin(args.profile_sha256),
                "project_sha256": valid_pin(args.project_sha256), "spec_sha256": valid_pin(args.spec_sha256),
                "snapshot_sha256": file_sha(args.snapshot), "result": "CAPTURED_NOT_PROOF"}
    require(receipt == expected, "TRANSFER_MISMATCH: payload, profile, sources or run identity changed")
    return {"result": "TRANSFER_VERIFIED_NOT_PROOF", "snapshot_sha256": receipt["snapshot_sha256"]}


def qualify(args):
    profile = check_profile(args)
    with tempfile.TemporaryDirectory(prefix="adrproof-producer-probe-") as temporary:
        root = Path(temporary)
        project, spec = root / "project", root / "spec"
        project.mkdir()
        spec.mkdir()
        marker = root / "host-only.txt"
        marker.write_text("synthetic host marker")
        with socket.socket() as server:
            server.bind(("127.0.0.1", 0))
            server.listen(1)
            payload = ["/usr/bin/python3", "-I", "/probe.py", str(marker), os.readlink("/proc/self/ns/net"), str(server.getsockname()[1])]
            checks = json.loads(execute(command(args, project, spec, payload, probe=True)))
        require(checks and all(value is True for value in checks.values()), "QUALIFICATION_FAILED")
        try:
            execute(command(args, project, spec, ["/usr/bin/python3", "-I", "-c", "import time; time.sleep(10)"]), timeout=0.5)
        except ValueError as error:
            require(str(error) == "RUNNER_TIMEOUT", str(error))
            checks["wall_timeout_enforced"] = True
        else:
            raise ValueError("qualification did not enforce timeout")
        require(marker.read_text() == "synthetic host marker" and not list(project.iterdir()) and not list(spec.iterdir()),
                "qualification modified protected inputs")
    require(describe(args.runtime, args.bwrap) == profile, "PROFILE_DRIFT after qualification")
    return {"result": "NEUTRAL_BOUNDARY_QUALIFIED", "profile_sha256": args.profile_sha256, "checks": checks}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest="action", required=True)
    digest = actions.add_parser("digest")
    digest.add_argument("--tree", required=True)
    for action in ("profile", "qualify", "produce", "verify-transfer"):
        sub = actions.add_parser(action)
        if action != "verify-transfer":
            sub.add_argument("--runtime", required=True)
            sub.add_argument("--bwrap", required=True)
        if action in ("qualify", "produce"):
            sub.add_argument("--profile", required=True)
        if action != "profile":
            sub.add_argument("--profile-sha256", required=True)
        if action in ("produce", "verify-transfer"):
            for option in ("run-id", "project-sha256", "spec-sha256"):
                sub.add_argument("--" + option, required=True)
        if action == "produce":
            for option in ("project", "spec", "output"):
                sub.add_argument("--" + option, required=True)
        if action == "verify-transfer":
            for option in ("receipt", "receipt-sha256", "snapshot"):
                sub.add_argument("--" + option, required=True)
    args = parser.parse_args()
    if args.action == "digest":
        result = {"tree_sha256": tree_digest(args.tree)}
    elif args.action == "profile":
        result = describe(args.runtime, args.bwrap)
    else:
        result = {"qualify": qualify, "produce": produce, "verify-transfer": verify_transfer}[args.action](args)
    print(json.dumps(result, sort_keys=True, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({"result": "ERROR", "diagnostics": [str(error)]}), file=sys.stderr)
        sys.exit(2)
