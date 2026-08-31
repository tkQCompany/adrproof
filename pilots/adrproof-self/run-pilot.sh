#!/usr/bin/env bash
set -euo pipefail

pilot_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
adrproof_root="$(cd "$pilot_root/../.." && pwd)"
output=""
work_parent="${TMPDIR:-/tmp}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --output) output="${2:?}"; shift 2 ;;
    --work-parent) work_parent="${2:?}"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[[ -n "$output" ]] || { echo "usage: run-pilot.sh --output NEW_DIRECTORY [--work-parent DIR]" >&2; exit 2; }
[[ ! -e "$output" ]] || { echo "output already exists: $output" >&2; exit 2; }
mkdir -p "$output/baseline-state" "$output/mutation-state" "$work_parent"
output="$(cd "$output" && pwd)"

target="$work_parent/adrproof-self-pilot-target"
CARGO_TARGET_DIR="$target" cargo build --offline --manifest-path "$adrproof_root/Cargo.toml"
bin="$target/debug/adrproof"

"$bin" check --project-root "$adrproof_root" --spec-root "$pilot_root/specs" \
  --state-root "$output/baseline-state" --json >"$output/baseline.json"
jq -e '.verdict == "SAT" and .evidence_status == "PASS"' "$output/baseline.json" >/dev/null

mutated="$output/mutated-project"
mkdir -p "$mutated/src"
cp "$adrproof_root/Cargo.toml" "$adrproof_root/Cargo.lock" "$mutated/"
cp "$adrproof_root/src/lib.rs" "$mutated/src/lib.rs"
patch --quiet --no-backup-if-mismatch -d "$mutated" -p1 <"$pilot_root/mutations/add-direct-tokio.patch"

set +e
"$bin" check --project-root "$mutated" --spec-root "$pilot_root/specs" \
  --state-root "$output/mutation-state" --json >"$output/mutation.json"
mutation_exit=$?
set -e
[[ "$mutation_exit" -eq 1 ]]
jq -e '.verdict == "UNSAT" and .evidence_status == "FAIL"' "$output/mutation.json" >/dev/null
jq -e '.conflicts | any(.adr_id == "ADR-SELF-001" and .clause_id == "C1")' "$output/mutation.json" >/dev/null

"$bin" bundle create --output "$output/evidence-bundle" \
  --project-root "$adrproof_root" --spec-root "$pilot_root/specs" \
  --state-root "$output/baseline-state" --json >"$output/bundle-create.json"
"$bin" bundle verify "$output/evidence-bundle" --json >"$output/bundle-verify.json"
jq -e '.valid == true' "$output/bundle-verify.json" >/dev/null

jq -n \
  --slurpfile baseline "$output/baseline.json" \
  --slurpfile mutation "$output/mutation.json" \
  --slurpfile bundle "$output/bundle-verify.json" \
  '{schema_version:"adrproof-self-pilot-v1",phase:"8A",result:"PASS",baseline:$baseline[0],falsification:{mutation:"direct tokio dependency",result:$mutation[0]},bundle:$bundle[0],authority:"Deterministic Cargo metadata, ADRLogic, Z3, and offline bundle integrity for the ADRProof root package.",does_not_prove:["source-level absence of async calls","transitive dependency absence","runtime behavior"]}' \
  >"$output/summary.json"

echo "ADRProof self-pilot Phase 8A: PASS"
echo "Summary: $output/summary.json"
