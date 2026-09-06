#!/usr/bin/env bash
set -euo pipefail

script_directory=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repository_root=$(git -C "$script_directory" rev-parse --show-toplevel)
commit=$(git -C "$repository_root" rev-parse HEAD)
test_root=$(mktemp -d)
trap 'rm -rf "$test_root"' EXIT

git clone --quiet --no-hardlinks "$repository_root" "$test_root/checkout"
git -C "$test_root/checkout" checkout --quiet --detach "$commit"
cd "$test_root/checkout"

python3 scripts/check-markdown-links.py
cargo run --locked -- --help > "$test_root/help.txt"
grep -Fq 'Usage: adrproof' "$test_root/help.txt"

cargo run --locked -- facts examples/rust-workspace-architecture --json \
  > "$test_root/facts.json"
jq -e 'type == "array" and length > 0' "$test_root/facts.json" >/dev/null

if cargo run --locked -- inventory --spec-root examples/requirement-inventory --json \
  > "$test_root/inventory.json"; then
  echo "error: neutral inventory must expose its unmapped requirement" >&2
  exit 1
else
  inventory_exit=$?
fi
test "$inventory_exit" -eq 3
jq -e \
  '.schema_version == "adrproof-inventory-report-v1alpha1"
    and .result == "INCOMPLETE"
    and .review_status == "NOT_ASSESSED"
    and .verification_status == "NOT_RUN"
    and .gaps[0].requirement == "REQ-recovery"' \
  "$test_root/inventory.json" >/dev/null

cargo run --locked -- review prepare REQ-boundary \
  --spec-root examples/requirement-inventory --state-root "$test_root/review-state" --json \
  > "$test_root/review-draft.json"
jq -e '.schema_version == "adrproof-formalization-review-v1alpha1"
  and .decision == "draft" and .reviewer == null and .rationale == null' \
  "$test_root/review-draft.json" >/dev/null
if cargo run --locked -- review status \
  --spec-root examples/requirement-inventory --state-root "$test_root/review-state" --json \
  > "$test_root/review-status.json"; then
  echo "error: preparing a draft must not approve a requirement" >&2
  exit 1
else
  review_exit=$?
fi
test "$review_exit" -eq 3
test ! -e "$test_root/review-state"
jq -e '.result == "INCOMPLETE" and .verification_status == "NOT_RUN"' \
  "$test_root/review-status.json" >/dev/null

# The unreviewed example must not become an approved gate baseline.
if cargo run --locked -- gate prepare --project-root examples/external-provider/project \
  --spec-root examples/requirement-inventory --state-root "$test_root/gate-state" \
  --backend-version 'documentation-only' --timeout-ms 1000 --json > "$test_root/gate.json"; then
  echo "error: unreviewed requirements must not prepare a required set" >&2
  exit 1
else
  gate_exit=$?
fi
test "$gate_exit" -eq 2
test ! -e "$test_root/gate-state"
jq -e '.schema_version == "adrproof-gate-report-v1alpha1" and .result == "ERROR"' \
  "$test_root/gate.json" >/dev/null

mkdir -p "$test_root/state"
cargo run --locked -- provider check component-manifest --json \
  --project-root examples/external-provider/project \
  --spec-root examples/external-provider/spec \
  --state-root "$test_root/state" > "$test_root/provider.json"
jq -e \
  '.schema_version == "adrproof-provider-check-report-v1"
    and .result == "PASS"
    and .providers[0].provider.id == "component-manifest"' \
  "$test_root/provider.json" >/dev/null

if [[ -n $(git status --porcelain) ]]; then
  git status --short >&2
  echo "error: documented commands changed the clean checkout" >&2
  exit 1
fi

echo "Clean-checkout documentation smoke test passed for $commit"
