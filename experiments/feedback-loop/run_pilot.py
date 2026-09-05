#!/usr/bin/env python3
"""Local nine-trial study, not an ADRProof feature or autonomous repair service."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import tomllib
import uuid

from qualify import ALLOWED, CHECK, PIN, SPEC, digest, domain_manifest, dump, files, fixture, independent_oracle, put

HERE = Path(__file__).resolve().parent
MODEL = "gpt-5.6-sol"
SCHEMA = {"type": "object", "properties": {
    "manifest": {"type": "string"}, "source": {"type": "string"}},
    "required": ["manifest", "source"], "additionalProperties": False}
INSTRUCTION = """Repair the supplied Rust workspace's implementation to satisfy the
immutable architecture specification while preserving all existing behavior.
The domain package must not depend directly on fake_sqlx. Preserve its name,
version, edition, mandatory repository dependency, workspace membership and all
other packages/tests. Do not weaken rules or move the dependency to another kind.
You may edit only domain/Cargo.toml and domain/src/lib.rs. No new dependencies,
files or package settings. Return their COMPLETE new contents using the supplied
JSON schema (manifest, source), no markdown. Do not call tools; all available
source and feedback are in this message. Up to three proposals are allowed.
PASS only covers the verified obligations, not overall program correctness.
Conflict sets are not necessarily unique root causes or ready-made repairs.
"""


def safety(proposal):
    if not isinstance(proposal, dict) or set(proposal) != {"manifest", "source"}:
        raise ValueError("invalid structured proposal")
    if any(not isinstance(v, str) or len(v.encode()) > 16384 or "\0" in v for v in proposal.values()):
        raise ValueError("invalid file contents or size")
    parsed = tomllib.loads(proposal["manifest"])
    original = tomllib.loads(domain_manifest("code"))
    if set(parsed) - {"package", "dependencies"} or parsed.get("package") != original["package"]:
        raise ValueError("unsafe package configuration")
    deps = parsed.get("dependencies", {})
    if not isinstance(deps, dict) or any(k not in original["dependencies"] or v != original["dependencies"][k] for k, v in deps.items()):
        raise ValueError("dependency outside execution allowlist")


def decode_events(raw):
    events = [json.loads(line) for line in raw.splitlines() if line.strip()]
    messages, usages = [], []
    for event in events:
        kind = event["type"]
        if kind.startswith("item."):
            item = event["item"]
            if item["type"] not in {"agent_message", "reasoning"}:
                raise ValueError("forbidden tool/event: " + item["type"])
            if kind == "item.completed" and item["type"] == "agent_message":
                messages.append(item["text"])
        elif kind == "turn.completed":
            usages.append(event["usage"])
        elif kind not in {"thread.started", "turn.started"}:
            raise ValueError("transport event: " + kind)
    if not messages or len(usages) != 1:
        raise ValueError("missing response or usage")
    proposal = json.loads(messages[-1])
    safety(proposal)
    return proposal, usages[0]


def process(argv, timeout, prompt=None):
    start = time.monotonic()
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr, tempfile.TemporaryFile() as stdin:
        if prompt is not None:
            stdin.write(prompt.encode())
            stdin.seek(0)
        proc = subprocess.Popen(argv, stdin=stdin, stdout=stdout, stderr=stderr, start_new_session=True)
        exceeded = False
        try:
            while proc.poll() is None:
                if time.monotonic() - start >= timeout or max(os.fstat(stdout.fileno()).st_size, os.fstat(stderr.fileno()).st_size) > 2**20:
                    exceeded = True
                    break
                time.sleep(0.02)
        finally:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            proc.wait(timeout=5)
        stdout.seek(0)
        stderr.seek(0)
        return {"exit": proc.returncode, "limit_exceeded": exceeded,
                "elapsed_seconds": time.monotonic() - start,
                "stdout": stdout.read(2**20).decode(errors="replace"),
                "stderr": stderr.read(2**20).decode(errors="replace")}


def scope(argv, timeout):
    unit = "adrproof-feedback-" + uuid.uuid4().hex
    prefix = ["systemd-run", "--user", "--pipe", "--wait", "--quiet", "--collect",
              "--unit", unit, "-p", "MemoryMax=2G", "-p", "MemorySwapMax=0",
              "-p", "TasksMax=128", "-p", "KillMode=control-group",
              "-p", "LimitFSIZE=67108864", "-p", "LimitCORE=0",
              "-p", "RuntimeMaxSec=" + str(max(1, int(timeout))), "--"]
    try:
        return process(prefix + argv, timeout + 2)
    finally:
        subprocess.run(["systemctl", "--user", "stop", unit], capture_output=True, timeout=5)


class Box:
    def __init__(self, args):
        self.args = args

    def run(self, project, spec, state, command, mode="tests", timeout=30):
        state.mkdir(parents=True, exist_ok=True)
        argv = ["bwrap", "--unshare-all", "--die-with-parent", "--new-session",
                "--clearenv", "--ro-bind", "/usr", "/usr",
                "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib", "/lib64",
                "--proc", "/proc", "--dev", "/dev", "--size", "67108864", "--tmpfs", "/tmp",
                "--ro-bind", str(self.args.toolchain), "/toolchain",
                "--ro-bind", str(self.args.adrproof), "/tools/adrproof",
                "--ro-bind", str(self.args.z3), "/tools/z3",
                "--bind" if mode == "lock" else "--ro-bind", str(project), "/project",
                "--ro-bind", str(spec), "/spec"]
        argv += (["--bind", str(state), "/state"] if mode == "verify" else
                 ["--size", "536870912", "--tmpfs", "/state"])
        argv += ["--setenv", "PATH", "/toolchain/bin:/usr/bin",
                 "--setenv", "CARGO_HOME", "/state/cargo",
                 "--setenv", "CARGO_TARGET_DIR", "/state/target",
                 "--setenv", "ADRPROOF_Z3", "/tools/z3",
                 "--chdir", "/project", "--", *command]
        return scope(argv, timeout)


def evaluate(box, project, spec, state, original, deadline):
    def run(command, mode):
        remaining = deadline - time.monotonic() - 3
        if remaining <= 0:
            raise TimeoutError("trial budget exhausted")
        return box.run(project, spec, state, command, mode, min(30, remaining))
    oracle = independent_oracle(project, "code", original)
    lock = run(["cargo", "generate-lockfile", "--offline"], "lock")
    if lock["exit"] != 0:
        raise RuntimeError("lock generation failed")
    before = files(project)
    tests = run(["cargo", "test", "--locked", "--offline", "--workspace", "--all-targets"], "tests")
    check = run(CHECK, "verify")
    if check["exit"] not in (0, 1, 3) or check["limit_exceeded"]:
        raise RuntimeError("verifier infrastructure failed")
    assert files(project) == before
    report = json.loads(check["stdout"])
    architectural_pass = report["verdict"] == "SAT" and report["evidence_status"] == "PASS" and check["exit"] == 0
    accepted = oracle["accepted"] and tests["exit"] == 0 and not tests["limit_exceeded"] and architectural_pass
    return {"oracle": oracle, "lock": lock, "tests": tests, "check": check,
            "accepted": accepted, "regression": tests["exit"] != 0,
            "false_pass": architectural_pass and not accepted,
            "input_hashes": before,
            "model": json.loads((state / "eval/project-model.json").read_text()),
            "ledger": json.loads((state / "eval/proof-ledger.json").read_text())}


def feedback(result, arm):
    # Explicit allowlist: NEVER serialize the independent oracle into prompts.
    visible = {"compiler_tests": {k: result["tests"][k] for k in ("exit", "stdout", "stderr")}}
    report = json.loads(result["check"]["stdout"])
    if arm != "A":
        visible["architecture"] = {k: report[k] for k in ("verdict", "evidence_status")}
    if arm == "C":
        visible["diagnostics"] = {"check": report, "project_model": result["model"], "ledger": result["ledger"]}
    return visible


def transport(prompt, destination, timeout):
    with tempfile.TemporaryDirectory(prefix="adrproof-feedback-transport-") as temporary:
        cwd = Path(temporary)
        schema = cwd / "schema.json"
        dump(schema, SCHEMA)
        argv = ["codex", "exec", "--ignore-user-config", "--ephemeral", "--skip-git-repo-check",
                "--sandbox", "read-only", "--json", "--output-schema", str(schema),
                "-m", MODEL, "-c", 'model_reasoning_effort="high"', "-C", str(cwd)]
        for feature in ("shell_tool", "apps", "hooks", "multi_agent", "browser_use", "view_image", "image_generation"):
            argv += ["--disable", feature]
        argv += ["-"]
        put(destination / "prompt.txt", prompt)
        result = process(argv, timeout, prompt)
        dump(destination / "transport.json", result)
        put(destination / "events.jsonl", result["stdout"])
        if result["exit"] or result["limit_exceeded"]:
            raise RuntimeError("model transport failed")
        proposal, usage = decode_events(result["stdout"])
        dump(destination / "proposal.json", proposal)
        return proposal, usage, result["elapsed_seconds"]


def main():
    parser = argparse.ArgumentParser()
    for name in ("adrproof", "z3", "toolchain", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--execute-nine", action="store_true")
    args = parser.parse_args()
    for name in ("adrproof", "z3", "toolchain", "output"):
        setattr(args, name, getattr(args, name).resolve())
    out = args.output
    out.mkdir(parents=True, exist_ok=False)
    box = Box(args)
    spec, baseline = out / "spec", out / "baseline/project"
    put(spec / "architecture.md", SPEC)
    put(spec / "adrproof.json", '{"z3_version":"4.13.4","timeout_ms":10000}\n')
    fixture(baseline, "code")
    original, protected_spec = files(baseline), files(spec)
    version = subprocess.check_output(["codex", "--version"], text=True).strip()
    assert version == "codex-cli 0.153.2", version
    protocol = {"verifier_commit": PIN, "model": MODEL, "effort": "high", "cli": version,
                "order": list("ABC" + "BCA" + "CAB"), "max_proposals": 3,
                "trial_seconds": 600, "requested_runs": 9 if args.execute_nine else 0,
                "source_hashes": {p.name: digest(p.read_bytes()) for p in (HERE / "qualify.py", HERE / "run_pilot.py", HERE / "PILOT-9.md")},
                "binaries": {k: digest(getattr(args, k).read_bytes()) for k in ("adrproof", "z3")},
                "spec": protected_spec, "initial": original, "schema": SCHEMA,
                "created_unix": time.time()}
    dump(out / "protocol.json", protocol)
    # Check actual kernel settings, not merely successful property parsing.
    probe = "from pathlib import Path; import json; c=Path('/sys/fs/cgroup'+Path('/proc/self/cgroup').read_text().strip().split('::')[1]); print(json.dumps({k:(c/k).read_text().strip() for k in ['memory.max','memory.swap.max','pids.max']}))"
    limits = scope(["/usr/bin/python3", "-c", probe], 10)
    dump(out / "limits.json", limits)
    assert limits["exit"] == 0 and json.loads(limits["stdout"]) == {"memory.max": "2147483648", "memory.swap.max": "0", "pids.max": "128"}, limits
    versions = {}
    for executable in ("rustc", "cargo", "/tools/z3"):
        version_result = box.run(baseline, spec, out / "preflight/state", [executable, "--version"])
        assert version_result["exit"] == 0, version_result
        versions[executable] = version_result["stdout"].strip()
    assert "1.98.0" in versions["rustc"] and "4.13.4" in versions["/tools/z3"]
    dump(out / "versions.json", versions)
    isolation_code = """import os, errno, json
try:
    open('/project/domain/src/lib.rs', 'w')
    raise AssertionError('project writable')
except OSError as e:
    assert e.errno == errno.EROFS
assert os.statvfs('/state').f_blocks * os.statvfs('/state').f_frsize == 536870912
assert os.statvfs('/tmp').f_blocks * os.statvfs('/tmp').f_frsize == 67108864
assert not os.path.exists('/home/progger')
children = 0
while children < 140:
    try:
        pid = os.fork()
    except BlockingIOError:
        break
    if pid == 0:
        import time
        time.sleep(20)
        os._exit(0)
    children += 1
assert 1 <= children < 128, children
print(json.dumps({'read_only': True, 'tmpfs_limits': True, 'task_limit_reached_at': children}), flush=True)
"""
    isolation = box.run(baseline, spec, out / "preflight/state", ["/usr/bin/python3", "-c", isolation_code])
    dump(out / "isolation.json", isolation)
    assert isolation["exit"] == 0 and isolation["elapsed_seconds"] < 10, isolation
    initial = evaluate(box, baseline, spec, out / "baseline/state", original, time.monotonic() + 180)
    dump(out / "baseline/result.json", initial)
    assert not initial["accepted"] and initial["tests"]["exit"] == 0 and json.loads(initial["check"]["stdout"])["verdict"] == "UNSAT"
    print("Baseline and kernel resource limits qualified", flush=True)
    if not args.execute_nine:
        return
    results = []
    source = {p: (baseline / p).read_text() for p in sorted(original)}
    for index, arm in enumerate(protocol["order"]):
        trial = out / f"trial-{index + 1}-{arm}"
        start = time.monotonic()
        deadline = start + 600
        history, current, attempts = [], source.copy(), []
        visible = feedback(initial, arm)
        failure = None
        print(f"Trial {index + 1}/9 {arm} started", flush=True)
        try:
            for iteration in range(1, 4):
                step = trial / f"proposal-{iteration}"
                prompt = INSTRUCTION + "\n" + json.dumps({"specification": SPEC, "current_source": current,
                         "previous_proposals_and_feedback": history, "current_feedback": visible,
                         "proposal_number": iteration}, sort_keys=True)
                proposal, usage, model_seconds = transport(prompt, step, max(1, deadline - time.monotonic() - 3))
                candidate = step / "project"
                fixture(candidate, "code")
                put(candidate / "domain/Cargo.toml", proposal["manifest"])
                put(candidate / "domain/src/lib.rs", proposal["source"])
                result = evaluate(box, candidate, spec, step / "state", original, deadline)
                assert files(spec) == protected_spec
                dump(step / "evaluation.json", result)
                attempts.append({k: result[k] for k in ("accepted", "regression", "false_pass")}
                                | {"iteration": iteration, "usage": usage, "model_seconds": model_seconds,
                                   "tool_seconds": sum(result[k]["elapsed_seconds"] for k in ("lock", "tests", "check"))})
                dump(trial / "attempts.json", attempts)
                if result["accepted"]:
                    break
                history.append({"proposal": proposal, "feedback_before_proposal": visible})
                visible = feedback(result, arm)
                current.update({"domain/Cargo.toml": proposal["manifest"], "domain/src/lib.rs": proposal["source"]})
        except (ValueError, RuntimeError, TimeoutError, AssertionError, OSError) as error:
            failure = type(error).__name__ + ": " + str(error)
        row = {"trial": index + 1, "arm": arm, "repeat": index // 3 + 1, "attempts": attempts,
               "accepted": bool(attempts and attempts[-1]["accepted"]),
               "elapsed_seconds": time.monotonic() - start, "failure": failure}
        results.append(row)
        dump(trial / "result.json", row)
        dump(out / "summary.json", {"results": results, "complete": len(results) == 9, "monetary_cost": None})
        print(f"Trial {index + 1}/9 {arm}: accepted={row['accepted']} proposals={len(attempts)} failure={failure}", flush=True)
        if failure:
            print("Stopping matrix on protocol/infrastructure failure; no replacement trial", flush=True)
            break
    dump(out / "artifact-hashes.json", files(out))


if __name__ == "__main__":
    main()
