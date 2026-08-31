# Native test evidence and portable bundles

ADRProof can import a normalized native test report as evidence instead of
treating a successful shell command as an unstructured CI side effect.
Definitions live in `native-tests/checks/*.json` below the specification root.
They declare the exact command and working directory, pass/skip thresholds,
required named tests, authority, soundness boundary and fingerprinted inputs.

The currently supported report schema is `nextest-summary-v1`. Import is
fail-closed: PASS requires the declared schema, command and directory; a PASS
runner result; zero failed tests; a non-empty execution; thresholds; and every
required named test observed as PASS. The original report hash, runner version,
configuration hash and semantic input fingerprints are retained in immutable
`native-test-evidence`. Later input or definition changes compute as STALE and
cannot satisfy a parent gate.

```bash
adrproof native-test import CHECK-ID --report report.json \
  --project-root PROJECT --spec-root SPEC --state-root STATE --json
adrproof native-test status CHECK-ID \
  --project-root PROJECT --spec-root SPEC --state-root STATE --json
```

`diagnose` correlates missing, failed and stale scenario, native-test, model,
model-validation, correspondence and parent evidence. Its explanations do not
override the individual verifier verdicts.

## Bundles

`bundle create` copies non-temporary files from the state root into `data/` and
writes `bundle.json` with sorted relative paths, byte lengths and SHA-256 hashes.
It rejects symbolic links. The output directory must not already exist.

```bash
adrproof bundle create --output evidence-bundle \
  --project-root PROJECT --spec-root SPEC --state-root STATE --json
adrproof bundle verify evidence-bundle --json
```

Verification is offline and rejects unsafe manifest paths, missing, extra,
modified or duplicate files. A valid bundle establishes integrity of the copied
ADRProof ledger only. It does not replay tests, solvers or model checkers, and it
does not make historic evidence CURRENT against a different checkout.
