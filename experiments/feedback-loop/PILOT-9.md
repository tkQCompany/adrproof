# Nine-session pilot protocol — frozen before repair calls

User-approved scope: the `code` case only, three repetitions of A/B/C, in order
ABC / BCA / CAB. This supersedes the proposed 36-session matrix and 8,000-token
budget for this run, not for future experiments. The model is `gpt-5.6-sol`,
reasoning `high`, through local Codex CLI 0.153.2. This pins a requested model
identifier, not a publicly attested immutable model snapshot.

Each trial starts from the identical broken fixture and receives its complete
source, immutable specification, allowed-file contract and initial compiler/test
feedback. A gets no verifier output; B gets verdict and evidence status; C gets
those plus the actual check report, project model and evidence ledger. No arm
gets the independent architecture oracle. A conflict set is not a repair recipe.

Maximum three proposals and 600 seconds per trial, including model and tool
time. Stop on independently accepted repair plus fresh verifier SAT/PASS,
budget exhaustion, protocol violation, or infrastructure failure. The stop
decision is not sent back as an oracle diagnostic. After an unsuccessful
proposal, only the arm's permitted feedback is sent. No human repair hints or
manual patch adjustments. Fresh ephemeral calls use explicit replay of previous
proposals and permitted feedback within a trial; there is no cross-trial replay.

The model returns only complete contents of two allowed files as structured
JSON. Execution tools, apps, hooks, multi-agent, browser and image capabilities
are disabled; any emitted non-message/reasoning item invalidates the trial.
The CLI starts in an empty temporary directory, not in the source repository.
Raw event logs, prompts, usage, outputs and evaluations remain local in ignored
`dist/`. No credentials are read, copied or passed to candidate execution.

Candidate safety: only the two predefined file paths, at most 16 KiB each; no
new files/dependencies/package configuration. Manifests may retain or remove
the fixture's existing dependencies, but not change their definitions. Safety
rejection stops the trial and is not an oracle hint. Compilation and tests run
offline in a read-only project namespace, with disposable 512 MiB state and
64 MiB temporary filesystems. A transient user systemd service enforces 2 GiB
aggregate memory, zero swap, 128 tasks and a step deadline; namespace and service
teardown terminate descendants. Verifier evidence is written separately by the
trusted frozen verifier. This is a local experimental runner, not a production
CI runner qualification or a complete malicious-code security audit.

The independent oracle checks the full frozen file set and exact parsed
dependency/metadata contract. Unchanged functional tests check behavior.
Architectural SAT with failed independent acceptance or tests is a false PASS
for the repair objective (not a claim that ADRProof promised functional safety).
Record intermediate regressions as well as final successes. Functional tests
are visible and fixed, not a comprehensive hidden security suite.

Record all three repeats, proposals, wall/tool/model time and raw input, cached,
output and reasoning token fields. There is no hard token ceiling; monetary cost
is unavailable for this subscription transport. Abort infrastructure faults,
preserve failed records and do not silently substitute successful runs.

Exploratory signal: C beats A by at least one accepted repair out of three,
without additional regressions/false PASS; C versus B tests added diagnostic
value. This is only a signal for a larger preregistered study, not statistical
confirmation. Equal 3/3 first-attempt success is a ceiling/inconclusive result.
C no better than A/B with greater token/time cost weakens the practical benefit
on this fixture. Do not change the task, prompt or thresholds after results.

Transport follows [official non-interactive documentation](https://learn.chatgpt.com/docs/non-interactive-mode):
ephemeral execution, ignored user configuration, JSONL usage and structured output.
