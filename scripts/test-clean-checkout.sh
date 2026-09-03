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
