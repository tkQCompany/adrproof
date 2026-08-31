# Deterministic scenario evidence

`DeterministicFailureScenarioProvider` executes a declared command and decides
PASS or FAIL exclusively by comparing its JSON postconditions with the expected
postconditions in `spec:scenarios/<id>.json`. A textual trace is diagnostic and
never decides the result. A runner startup failure, timeout, invalid JSON, or an
explicit `infrastructure_error` is ERROR, not an invariant failure.

Scenario evidence is deliberately bounded. Its authority is one identified
scenario version, fixture, named fault point, implementation inputs, runner
version/configuration, and tool versions. It cannot establish all failures,
arbitrary instruction interleavings, or eventual liveness. Each definition must
state `covered`, `not_covered`, `authority`, and `does_not_prove`.

Evidence is immutable under `state_root/scenario-evidence/`. Current validity is
computed from the definition, runner configuration, declared project/spec input
content, provider version, and fixture. A change makes historic PASS or FAIL
STALE. Root locations do not participate in semantic input identities.

The small parent model supports only `ALL` required children. Its precedence is:

1. any ERROR -> ERROR;
2. any CURRENT FAIL -> FAIL;
3. any STALE child -> STALE;
4. missing or UNVERIFIED child -> UNVERIFIED;
5. otherwise every required child must be PASS/CURRENT -> PASS.

No quorum, OR, weighting, or policy language is provided. Parent authority must
not exceed the intersection/composition of the explicit child scopes.

CLI:

```text
adrproof scenario list [root flags]
adrproof scenario run <ID> [root flags]
adrproof scenario status [ID] [root flags]
```

The graph artifact `state_root/scenario-graph.json` connects definition and
implementation artifacts to scenario obligations and immutable evidence, and
connects parent obligations to children with typed `Requires` edges.
