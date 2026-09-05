#!/usr/bin/env python3
"""Qualify neutral feedback fixtures; never invokes an LLM or edits source repo."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import tempfile
import time
import tomllib


ROOT = Path(__file__).resolve().parents[2]
PIN = "e6914b742bf1f5ddb08eecb368e2413807fd14c0"
ALLOWED = {"domain/Cargo.toml", "domain/src/lib.rs"}
SPEC = """---
id: ADR-0100
status: accepted
---
# Domain dependency boundary
```adrlogic
entity Package { domain, repository, fake_sqlx, helper };
relation declares_direct_dependency(Package, Package);
rule C1 "domain must not depend directly on fake_sqlx" {
    !declares_direct_dependency(domain, fake_sqlx);
}
```
"""
DOMAIN = "pub fn company_name(value: &str) -> String { repository::company_name(value) }\n"
TESTS = """#[test]
fn preserves_company_name_behavior() {
    for (input, expected) in [(" Acme ", "ACME"), ("beta", "BETA"),
                               ("", ""), ("  two words  ", "TWO WORDS")] {
        assert_eq!(domain::company_name(input), expected);
    }
}
"""


def digest(data):
    return hashlib.sha256(data).hexdigest()


def put(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def dump(path, value):
    put(path, json.dumps(value, indent=2, sort_keys=True) + "\n")


def files(root):
    return {str(p.relative_to(root)): digest(p.read_bytes())
            for p in sorted(root.rglob("*")) if p.is_file()}


def domain_manifest(case, repaired=False):
    header = "[package]\nname='domain'\nversion='0.1.0'\nedition='2024'\n"
    deps = "\n[dependencies]\nrepository={path='../repository'}\n"
    if case == "table":
        header += "description='Preserve this legal metadata change'\n"
        deps += "helper={path='../helper'}\n"
    if not repaired:
        if case in ("direct", "code"):
            deps += "fake_sqlx={path='../fake_sqlx'}\n"
        elif case == "alias":
            deps += "storage={package='fake_sqlx',path='../fake_sqlx'}\n"
        elif case == "table":
            deps += "\n[dependencies.storage]\npackage='fake_sqlx'\npath='../fake_sqlx'\n"
        else:
            raise ValueError(case)
    return header + deps


def fixture(root, case, repaired=False):
    put(root / "Cargo.toml", "[workspace]\nmembers=['domain','repository','fake_sqlx','helper']\nresolver='2'\n")
    for name in ("repository", "fake_sqlx", "helper"):
        manifest = f"[package]\nname='{name}'\nversion='0.1.0'\nedition='2024'\n"
        if name == "repository":
            manifest += "\n[dependencies]\nfake_sqlx={path='../fake_sqlx'}\n"
        put(root / name / "Cargo.toml", manifest)
    put(root / "fake_sqlx/src/lib.rs", "pub fn normalize(value: &str) -> String { value.trim().to_uppercase() }\n")
    put(root / "repository/src/lib.rs", "pub fn company_name(value: &str) -> String { fake_sqlx::normalize(value) }\n")
    put(root / "helper/src/lib.rs", "pub fn enabled() -> bool { true }\n")
    put(root / "domain/Cargo.toml", domain_manifest(case, repaired))
    put(root / "domain/src/lib.rs", DOMAIN if case != "code" or repaired else
        "pub fn company_name(value: &str) -> String { fake_sqlx::normalize(value) }\n")
    put(root / "domain/tests/behavior.rs", TESTS)


def independent_oracle(root, case, original):
    reasons = []
    current = files(root)
    for name in set(original) | set(current):
        if name not in ALLOWED and name != "Cargo.lock" and current.get(name) != original.get(name):
            reasons.append("protected file change: " + name)
    try:
        actual = tomllib.loads((root / "domain/Cargo.toml").read_text())
        expected = tomllib.loads(domain_manifest(case, True))
        if actual != expected:
            reasons.append("manifest violates frozen dependency/metadata contract")
    except (ValueError, OSError) as error:
        reasons.append("manifest cannot be parsed: " + str(error))
    return {"accepted": not reasons, "reasons": reasons}


class Runner:
    def __init__(self, adrproof, z3, toolchain):
        self.adrproof, self.z3, self.toolchain = adrproof, z3, toolchain

    def run(self, project, spec, state, command, writable=False, timeout=30):
        state.mkdir(parents=True, exist_ok=True)
        argv = ["bwrap", "--unshare-all", "--die-with-parent", "--new-session",
                "--clearenv", "--ro-bind", "/usr", "/usr",
                "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64",
                "--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp",
                "--ro-bind", str(self.toolchain), "/toolchain",
                "--ro-bind", str(self.adrproof), "/tools/adrproof",
                "--ro-bind", str(self.z3), "/tools/z3",
                "--bind" if writable else "--ro-bind", str(project), "/project",
                "--ro-bind", str(spec), "/spec", "--bind", str(state), "/state",
                "--setenv", "PATH", "/toolchain/bin:/usr/bin",
                "--setenv", "CARGO_HOME", "/state/cargo",
                "--setenv", "CARGO_TARGET_DIR", "/state/target",
                "--setenv", "ADRPROOF_Z3", "/tools/z3",
                "--chdir", "/project", "--", *command]

        def limits():
            resource.setrlimit(resource.RLIMIT_AS, (4 * 1024**3,) * 2)
            resource.setrlimit(resource.RLIMIT_CPU, (30, 30))
            resource.setrlimit(resource.RLIMIT_FSIZE, (64 * 1024**2,) * 2)

        start = time.monotonic()
        with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
            proc = subprocess.Popen(argv, stdout=stdout, stderr=stderr,
                                    start_new_session=True, preexec_fn=limits)
            timed_out = False
            try:
                while proc.poll() is None:
                    if time.monotonic() - start > timeout or max(os.fstat(stdout.fileno()).st_size, os.fstat(stderr.fileno()).st_size) > 1024**2:
                        timed_out = True
                        break
                    time.sleep(0.02)
            finally:
                # Kill the namespace launcher group even after its leader exits.
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                proc.wait(timeout=5)
            stdout.seek(0)
            stderr.seek(0)
            return {"command": command, "exit": proc.returncode,
                    "limit_exceeded": timed_out, "elapsed_seconds": time.monotonic() - start,
                    "stdout": stdout.read(1024**2).decode(errors="replace"),
                    "stderr": stderr.read(1024**2).decode(errors="replace")}


CHECK = ["/tools/adrproof", "check", "--project-root", "/project",
         "--spec-root", "/spec", "--state-root", "/state/eval", "--json"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--adrproof", type=Path, required=True)
    parser.add_argument("--z3", type=Path, required=True)
    parser.add_argument("--toolchain", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    runner = Runner(args.adrproof.resolve(), args.z3.resolve(), args.toolchain.resolve())
    dump(out / "protocol.json", {"verifier_commit": PIN, "spec_sha256": digest(SPEC.encode()),
        "harness_sha256": digest(Path(__file__).read_bytes()),
        "protocol_sha256": digest((Path(__file__).parent / "README.md").read_bytes()),
        "adrproof_binary_sha256": digest(runner.adrproof.read_bytes()),
        "z3_binary_sha256": digest(runner.z3.read_bytes()), "llm_runs": 0})
    results = []
    spec = out / "spec"
    put(spec / "architecture.md", SPEC)
    put(spec / "adrproof.json", '{"z3_version":"4.13.4","timeout_ms":10000}\n')
    expected_spec = files(spec)
    for case in ("direct", "alias", "table", "code"):
        for repaired in (False, True):
            name = case + ("-repaired" if repaired else "-violation")
            project, state = out / name / "project", out / name / "state"
            fixture(project, case, repaired)
            original = files(project)
            lock = runner.run(project, spec, state, ["cargo", "generate-lockfile", "--offline"], writable=True)
            assert lock["exit"] == 0, lock
            before = files(project)
            tests = runner.run(project, spec, state, ["cargo", "test", "--locked", "--offline", "--workspace", "--all-targets"])
            check = runner.run(project, spec, state, CHECK)
            oracle = independent_oracle(project, case, original)
            report = json.loads(check["stdout"])
            assert tests["exit"] == 0, tests
            assert check["exit"] == (0 if repaired else 1), check
            assert report["verdict"] == ("SAT" if repaired else "UNSAT"), report
            assert oracle["accepted"] == repaired, oracle
            assert files(project) == before, "execution changed read-only inputs"
            record = {"case": case, "handcrafted_repair": repaired, "lock": lock,
                      "tests": tests, "check": check, "oracle": oracle,
                      "input_hashes": before}
            dump(out / name / "result.json", record)
            results.append({"case": name, "tests_pass": True,
                            "verdict": report["verdict"], "independent_acceptance": oracle["accepted"]})
            print(name, report["verdict"], flush=True)
    versions = {}
    for tool in ("/tools/z3", "rustc", "cargo", "/usr/bin/python3"):
        version = runner.run(project, spec, state, [tool, "--version"])
        assert version["exit"] == 0, version
        versions[tool] = version["stdout"].strip()
    assert "4.13.4" in versions["/tools/z3"], versions
    assert "1.98.0" in versions["rustc"], versions
    dump(out / "versions.json", versions)
    # A superficially green architecture result must not hide broken behavior.
    project, state = out / "behavior-regression/project", out / "behavior-regression/state"
    fixture(project, "code", True)
    original = files(project)
    put(project / "domain/src/lib.rs", 'pub fn company_name(_: &str) -> String { String::new() }\n')
    lock = runner.run(project, spec, state, ["cargo", "generate-lockfile", "--offline"], writable=True)
    assert lock["exit"] == 0, lock
    tests = runner.run(project, spec, state, ["cargo", "test", "--locked", "--offline", "--workspace", "--all-targets"])
    check = runner.run(project, spec, state, CHECK)
    assert tests["exit"] != 0 and check["exit"] == 0, (tests, check)
    dump(out / "behavior-regression/result.json", {"tests": tests, "check": check,
         "accepted": False, "meaning": "ADRProof PASS alone is insufficient"})
    # Freshness query after a comment-only edit must retain historical PASS as STALE.
    project = out / "stale/project"
    state = out / "stale/state"
    shutil.copytree(out / "direct-repaired/project", project)
    shutil.copytree(out / "direct-repaired/state/eval", state / "eval")
    manifest = project / "domain/Cargo.toml"
    put(manifest, manifest.read_text() + "\n# changed after verification\n")
    status = runner.run(project, spec, state, ["/tools/adrproof", "status", "--project-root", "/project", "--spec-root", "/spec", "--state-root", "/state/eval", "--json"])
    assert status["exit"] == 0 and json.loads(status["stdout"])["current"].get("STALE") == 1, status
    dump(out / "stale-control.json", status)
    # Missing coverage/data is distinct from a known violated obligation.
    missing_project, missing_spec = out / "missing/project", out / "missing/spec"
    missing_project.mkdir(parents=True)
    missing_spec.mkdir(parents=True)
    put(missing_spec / "architecture.md", """---
id: MISSING-DATA
status: accepted
---
```adrlogic
entity Component { api };
relation observed(Component);
rule C1 "requires an observed component" { observed(api); }
```
""")
    missing = runner.run(missing_project, missing_spec, out / "missing/state", CHECK)
    assert missing["exit"] == 3 and json.loads(missing["stdout"])["verdict"] == "UNVERIFIED", missing
    dump(out / "missing-control.json", missing)
    failure = runner.run(project, spec, out / "failure/state", ["/usr/bin/env", "ADRPROOF_Z3=/tools/not-installed", *CHECK])
    assert failure["exit"] == 5 and not failure["stdout"].strip(), failure
    dump(out / "tool-failure-control.json", failure)
    # Verify scope/integrity rejection independently of the solver.
    protected = out / "protected/project"
    fixture(protected, "direct", True)
    original = files(protected)
    put(protected / "Cargo.toml", "[workspace]\nmembers=['repository','fake_sqlx','helper']\n")
    protected_result = independent_oracle(protected, "direct", original)
    assert not protected_result["accepted"], protected_result
    dump(out / "scope-tamper-control.json", protected_result)
    # Namespace teardown must kill a detached child, even when its leader exits.
    child = "import time,pathlib; time.sleep(2); pathlib.Path('/state/orphan-survived').write_text('bad')"
    parent = "import subprocess; subprocess.Popen(['/usr/bin/python3','-c'," + repr(child) + "],start_new_session=True)"
    orphan_state = out / "orphan/state"
    orphan = runner.run(project, spec, orphan_state, ["/usr/bin/python3", "-c", parent], timeout=3)
    assert orphan["exit"] == 0 and orphan["elapsed_seconds"] < 1.5, orphan
    timeout_state = out / "timeout/state"
    timeout = runner.run(project, spec, timeout_state, ["/usr/bin/python3", "-c", parent + "; import time; time.sleep(20)"], timeout=0.2)
    assert timeout["limit_exceeded"] and timeout["exit"] != 0, timeout
    time.sleep(2.2)
    assert not (orphan_state / "orphan-survived").exists()
    assert not (timeout_state / "orphan-survived").exists()
    dump(out / "process-controls.json", {"leader_exit": orphan, "timeout": timeout,
         "detached_child_marker_absent_after_deadline": True})
    assert files(spec) == expected_spec, "frozen specification changed"
    summary = {"qualification": "FIXTURE_QUALIFICATION_PASS", "llm_runs": 0,
               "results": results, "behavior_false_pass_rejected": True,
               "stale_detected": True, "missing_data_unverified": True,
               "tool_failure_distinguished": True, "scope_tamper_rejected": True,
               "detached_children_terminated": True,
               "pending": ["approved model budget and qualified model transport",
                           "candidate execution resource qualification", "A/B/C execution"]}
    dump(out / "summary.json", summary)
    dump(out / "artifact-hashes.json", files(out))
    print(json.dumps(summary), flush=True)


if __name__ == "__main__":
    main()
