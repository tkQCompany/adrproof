#!/usr/bin/env python3
"""Reconcile completed local logs; does not call models or change study criteria."""
import argparse
import json
from pathlib import Path
import statistics

from qualify import digest, dump, files, independent_oracle
from run_pilot import INSTRUCTION, decode_events, feedback


def audit(root):
    protocol = json.loads((root / "protocol.json").read_text())
    summary = json.loads((root / "summary.json").read_text())
    baseline = json.loads((root / "baseline/result.json").read_text())
    assert summary["complete"] and len(summary["results"]) == 9
    assert files(root / "spec") == protocol["spec"]
    for name, expected in json.loads((root / "artifact-hashes.json").read_text()).items():
        assert digest((root / name).read_bytes()) == expected, name
    for name, expected in protocol["source_hashes"].items():
        assert digest((Path(__file__).parent / name).read_bytes()) == expected, name
    source = {name: (root / "baseline/project" / name).read_text() for name in protocol["initial"]}
    assert {name: digest(text.encode()) for name, text in source.items()} == protocol["initial"]
    rows = []
    for trial, expected_arm in zip(summary["results"], protocol["order"], strict=True):
        assert trial["arm"] == expected_arm and trial["failure"] is None
        current, history = source.copy(), []
        visible = feedback(baseline, trial["arm"])
        for attempt in trial["attempts"]:
            step = root / f"trial-{trial['trial']}-{trial['arm']}" / f"proposal-{attempt['iteration']}"
            raw = (step / "events.jsonl").read_text()
            proposal, usage = decode_events(raw)
            assert usage == attempt["usage"]
            prompt = (step / "prompt.txt").read_text()
            assert prompt.startswith(INSTRUCTION + "\n")
            body = json.loads(prompt[len(INSTRUCTION) + 1:])
            assert body["current_source"] == current
            assert body["current_feedback"] == visible
            assert body["previous_proposals_and_feedback"] == history
            assert digest(body["specification"].encode()) == protocol["spec"]["architecture.md"]
            result = json.loads((step / "evaluation.json").read_text())
            assert files(step / "project") == result["input_hashes"]
            assert (step / "project/domain/Cargo.toml").read_text() == proposal["manifest"]
            assert (step / "project/domain/src/lib.rs").read_text() == proposal["source"]
            oracle = independent_oracle(step / "project", "code", protocol["initial"])
            assert oracle == result["oracle"]
            check = json.loads(result["check"]["stdout"])
            passed = check["verdict"] == "SAT" and check["evidence_status"] == "PASS" and result["check"]["exit"] == 0
            independently_valid = oracle["accepted"] and result["tests"]["exit"] == 0 and not result["tests"]["limit_exceeded"]
            assert attempt["accepted"] == (passed and independently_valid)
            assert attempt["false_pass"] == (passed and not independently_valid)
            assert attempt["regression"] == (result["tests"]["exit"] != 0)
            history.append({"proposal": proposal, "feedback_before_proposal": visible})
            visible = feedback(result, trial["arm"])
            current.update({"domain/Cargo.toml": proposal["manifest"], "domain/src/lib.rs": proposal["source"]})
        rows.append(trial)
    groups = {}
    for arm in "ABC":
        group = [row for row in rows if row["arm"] == arm]
        attempts = [a for row in group for a in row["attempts"]]
        groups[arm] = {"trials": len(group), "successes": sum(row["accepted"] for row in group),
                       "proposals": len(attempts), "regressions": sum(a["regression"] for a in attempts),
                       "false_passes": sum(a["false_pass"] for a in attempts),
                       "wall_seconds": sum(row["elapsed_seconds"] for row in group),
                       "median_wall_seconds": statistics.median(row["elapsed_seconds"] for row in group),
                       "model_seconds": sum(a["model_seconds"] for a in attempts),
                       "tool_seconds": sum(a["tool_seconds"] for a in attempts),
                       "usage": {key: sum(a["usage"][key] for a in attempts) for key in attempts[0]["usage"]}}
    return {"audit": "PASS", "groups": groups, "monetary_cost": None,
            "total_usage": {key: sum(group["usage"][key] for group in groups.values()) for key in groups["A"]["usage"]},
            "protocol_sha256": digest((root / "protocol.json").read_bytes()),
            "artifact_manifest_sha256": digest((root / "artifact-hashes.json").read_bytes()),
            "audit_script_sha256": digest(Path(__file__).read_bytes())}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("run", type=Path)
    args = parser.parse_args()
    result = audit(args.run)
    dump(args.run / "audit.json", result)
    print(json.dumps(result, indent=2))
