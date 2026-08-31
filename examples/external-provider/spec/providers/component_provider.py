#!/usr/bin/env python3
"""Reference ADRProof external-provider v1 implementation."""

import json
import sys
from pathlib import Path


def provenance(source: str) -> dict:
    return {
        "kind": "deterministically_extracted",
        "source": source,
        "span": None,
        "extractor": "component-provider.py@1.0.0",
    }


request = json.load(sys.stdin)
manifest_name = request["parameters"].get("manifest", "component.json")
manifest_path = Path(request["project_root"]) / manifest_name
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
source = f"project:{manifest_name}"

facts = []
for component in sorted(
    manifest["components"], key=lambda item: (item["name"], item["kind"])
):
    name = component["name"]
    kind = component["kind"]
    facts.append(
        {
            "id": f"component-manifest:component-kind:{name}:{kind}",
            "relation": "component_kind",
            "arguments": [name, kind],
            "value": True,
            "attributes": {},
            "provenance": provenance(source),
        }
    )

response = {
    "schema_version": "adrproof-external-provider-response-v1",
    "provider": {"id": "component-manifest", "version": "1.0.0"},
    "inputs": [source],
    "artifacts": [
        {
            "id": source,
            "kind": "component_manifest",
            "provenance": provenance(source),
        }
    ],
    "facts": facts,
    "coverage": [
        {
            "relation": "component_kind",
            "provider": "component-manifest",
            "world": "closed",
            "scope": {"kind": "global"},
            "qualifiers": {"manifest": manifest_name},
            "statement": "all component name/kind entries in the configured manifest are enumerated",
            "diagnostics": [],
        }
    ],
    "diagnostics": [],
}
json.dump(response, sys.stdout, sort_keys=True, separators=(",", ":"))
