# Supported platforms

This document states what the ADRProof project continuously verifies for the
0.2 release line. It distinguishes the full product test surface from the
portable external-provider protocol surface.

## Support matrix

| Environment | Continuously verified scope | Support level |
| --- | --- | --- |
| `ubuntu-latest` GitHub runner | Formatting, Clippy, full Rust test suite, external-provider process tests, dependency audit, and reproducible source archive | Primary |
| `macos-latest` GitHub runner | External-provider v1 response conformance and a real `provider check --json` process invocation | Protocol surface |
| `windows-latest` GitHub runner | External-provider v1 response conformance and a real `provider check --json` process invocation | Protocol surface |

The runner labels identify the environments exercised by CI; they do not
promise a particular CPU architecture or operating-system release beyond the
images currently supplied under those labels.

### Pending coverage expansion

The next CI run additionally exercises all library tests and CLI help on macOS
and Windows, plus the reference-provider CLI suite on macOS. POSIX shell-backed
library fixtures are explicitly Unix-only; parser, dependency, migration, and
evidence-freshness regressions now compile on Windows too. The pinned minimum
Rust version is used for these jobs, not a floating stable toolchain.

The first expanded run (CI #20) passed on Linux but failed on macOS and Windows:
canonical Cargo paths lost relevant fingerprints, and colon-bearing evidence
filenames failed on Windows. Both now have corrections and local regressions;
a remote rerun is still required. Keep the support levels above until the
expanded jobs pass for the published correction commit.
Windows still does not exercise the POSIX execution backends.

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

- The complete historical feature suite is continuously exercised on Linux.
  macOS and Windows CI currently guarantee the external-provider v1 protocol
  surface, not every ADRProof backend and evidence workflow.
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
