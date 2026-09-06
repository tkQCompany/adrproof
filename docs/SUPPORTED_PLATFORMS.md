# Supported platforms

This document states what the ADRProof project continuously verifies for the
0.2 release line. It distinguishes the full product test surface from the
portable external-provider protocol surface.

## Support matrix

| Environment | Continuously verified scope | Support level |
| --- | --- | --- |
| `ubuntu-latest` GitHub runner | Formatting, Clippy, full Rust test suite, external-provider process tests, dependency audit, and reproducible source archive | Primary |
| `macos-latest` GitHub runner | Library regressions, CLI help, requirement inventory, reference-provider CLI tests, response conformance and a native provider process invocation | Tested library and provider surfaces |
| `windows-latest` GitHub runner | Platform-neutral library regressions, CLI help, requirement inventory, response conformance and a native provider process invocation | Tested core and provider surfaces; no POSIX backends |

The runner labels identify the environments exercised by CI; they do not
promise a particular CPU architecture or operating-system release beyond the
images currently supplied under those labels.

### Confirmed expansion and next-run additions

Expanded library and CLI jobs passed for the exact pushed commit
`b321057ce3bf9bfc80d2ddbfa137026cea58a96f`: [CI](https://github.com/tkQCompany/adrproof/actions/runs/34032643386)
and [CodeQL](https://github.com/tkQCompany/adrproof/actions/runs/34032643378),
confirmed on 2026-09-06. All platform job steps passed except the intentionally
macOS-only reference-provider step, skipped on Windows. POSIX shell-backed library
fixtures remain Unix-only. The pinned minimum Rust version is used, not a
floating stable toolchain. Passing stub-backed fixtures does not qualify every
real external verifier installation.

This closes the rerun requirement after CI #20 exposed canonical Cargo path and
Windows evidence-filename defects. The requirement-inventory suite subsequently
passed on both portable runners for
`75b246411da6e3ba9c1d23e7f9ca572ad71e883a`: [CI](https://github.com/tkQCompany/adrproof/actions/runs/34036762730)
and [CodeQL](https://github.com/tkQCompany/adrproof/actions/runs/34036762711).
The formalization-review suite also passed on both portable runners for
`0cfdef9672c4d98f0411b69cc5fb40e94c2894dd`: [CI](https://github.com/tkQCompany/adrproof/actions/runs/34038211681)
and [CodeQL](https://github.com/tkQCompany/adrproof/actions/runs/34038211728).
The required-set gate suite passed on both portable runners for
`1042a56a6c6458cbfc666df18f2e8514d7d75033`: [CI](https://github.com/tkQCompany/adrproof/actions/runs/34039678722)
and [CodeQL](https://github.com/tkQCompany/adrproof/actions/runs/34039678829).
Snapshot controls now extend that suite using actual Cargo metadata and a native
fixture provider. Their own post-push result is required; earlier CI does not
qualify those additions or a real isolated producer environment.

## Toolchain and external programs

- Rust `1.98.0` is the minimum and pinned CI toolchain for the 0.2 line.
- Z3 `4.13.4` is required for ADRLogic consistency checks unless the project
  configuration deliberately selects another accepted executable and version.
- Cargo must be available when a checked project uses the built-in Cargo fact
  provider.
- External providers are trusted, explicitly configured executables. Their own
  interpreters and runtime dependencies must be installed by the operator.
- The neutral example provider uses Python 3 and the Python standard library;
  Python is not required when that example or another Python provider is not
  used.

## Known limitations

- The full test target set is continuously exercised on Linux. macOS and Windows
  additionally exercise the scopes above, not every real external backend,
  runtime dependency or end-to-end consumer workflow.
- ADRProof does not sandbox external providers. Timeout, output limits,
  process-tree cleanup, schema validation, declared-input validation, and
  fail-closed diagnostics constrain the protocol boundary but do not turn an
  untrusted executable into safe code.
- Filesystem and process behavior can differ outside the CI runner families.
  A portability defect with a minimal reproduction is release-relevant even
  when it does not reproduce on Linux.
- Source releases contain no prebuilt ADRProof executable. Users build from the
  pinned source with the declared Rust toolchain.

The project may expand this matrix only after adding repeatable CI evidence for
the broader claim.
