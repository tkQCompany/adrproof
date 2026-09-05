"""Protocol guard tests; no network, LLM calls, or candidate execution."""
import json
import unittest

from qualify import DOMAIN, domain_manifest
from run_pilot import decode_events, feedback, safety


class ProtocolTests(unittest.TestCase):
    def test_accepts_original_and_repair_without_giving_architecture_hint(self):
        for repaired in (False, True):
            safety({"manifest": domain_manifest("code", repaired), "source": DOMAIN})

    def test_rejects_extra_paths_and_package_hooks(self):
        with self.assertRaises(ValueError):
            safety({"manifest": domain_manifest("code"), "source": DOMAIN, "../x": ""})
        with self.assertRaises(ValueError):
            safety({"manifest": domain_manifest("code").replace("[package]", "[package]\nbuild='hook.rs'"), "source": DOMAIN})
        with self.assertRaises(ValueError):
            safety({"manifest": domain_manifest("code").replace("../repository", "/private"), "source": DOMAIN})

    def test_verifier_and_oracle_feedback_do_not_leak(self):
        result = {"tests": {"exit": 0, "stdout": "TEST", "stderr": ""},
                  "check": {"stdout": json.dumps({"verdict": "UNSAT", "evidence_status": "FAIL", "conflicts": ["DETAIL"]})},
                  "model": {"facts": "DETAIL"}, "ledger": {},
                  "oracle": {"accepted": False, "reasons": ["SECRET_ORACLE"]}}
        a, b, c = [json.dumps(feedback(result, arm)) for arm in "ABC"]
        for text in (a, b, c):
            self.assertNotIn("SECRET_ORACLE", text)
        self.assertNotIn("UNSAT", a)
        self.assertIn("UNSAT", b)
        self.assertNotIn("DETAIL", b)
        self.assertIn("DETAIL", c)

    def test_structured_message_requires_usage_and_disallows_tool_events(self):
        proposal = {"manifest": domain_manifest("code", True), "source": DOMAIN}
        events = [{"type": "item.completed", "item": {"type": "agent_message", "text": json.dumps(proposal)}},
                  {"type": "turn.completed", "usage": {"input_tokens": 7, "output_tokens": 3}}]
        encode = lambda records: "\n".join(json.dumps(e) for e in records)
        self.assertEqual(decode_events(encode(events))[0], proposal)
        with self.assertRaises(ValueError):
            decode_events(encode(events[:-1]))
        for kind in ("command_execution", "mcp_tool_call", "web_search", "file_change"):
            with self.assertRaises(ValueError):
                decode_events(encode([{"type": "item.started", "item": {"type": kind}}, *events]))


if __name__ == "__main__":
    unittest.main()
