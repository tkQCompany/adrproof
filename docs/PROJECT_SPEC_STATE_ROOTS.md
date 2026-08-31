# Project, specification, and state roots

ADRProof separates three filesystem roles so an independently managed formal
specification and evidence store can verify a read-only source checkout.

## Semantics

- `project_root` is read-only input for implementation artifacts. The Cargo
  provider runs there and never writes there.
- `specification_root` is read-only input for ADR Markdown and ADRLogic. It may be
  inside or outside the project.
- `state_root` is generated ADRProof state: `evidence/`, `effective.smt2`,
  `project-model.json`, and `proof-ledger.json`.

```text
adrproof check \
  --project-root /work/example-app/backend \
  --spec-root /work/example-verification/specs \
  --state-root /work/example-verification/state
```

The same roots apply to `facts`, `status`, `explain`, and `impact`. `facts` only
reads `project_root`; accepting all roots keeps invocations uniform and exposes
them in its summary.

## Precedence and compatibility

First, the legacy base is the positional root or the current directory. Explicit
project/spec flags override that base independently. Explicit state overrides the
legacy `BASE/.adrproof`. There is no root configuration file.
`adrproof.json`, when present, configures only solver version and timeout; it is
read first from the specification root and then from the project root.

Without flags, `adrproof check .` retains the legacy layout:

```text
project_root       = .
specification_root = .
state_root         = ./.adrproof
```

Explicit flags override the corresponding legacy/default value. An explicit,
potentially read-only project should always be paired with an explicit external
state root.

## Logical identities and relocation

Proof inputs use root-relative logical identities:

```text
project:crates/domain/Cargo.toml
spec:architecture/domain-boundaries.md
generated:effective.smt2
```

Physical paths are used to read bytes and appear in root diagnostics, but are not
semantic identities. Evidence fingerprints hash logical identity and content;
`state_root` is never a proof input. Consequently:

- relocating a byte-identical project or specification does not stale evidence;
- relocating/copying immutable evidence does not stale it;
- changing relevant project/specification content does stale it;
- backend version and semantic configuration changes still stale it.

Provenance and graph artifact IDs use `project:`/`spec:` namespaces. The roots in
JSON resolve those identities to the current filesystem locations.

`impact --path crates/domain/Cargo.toml` is relative to `project_root`. A spec
artifact can be selected as `--path spec:architecture/domain-boundaries.md`.
`RelevantTo` is conservative: a fact may affect a constraint. Excess STALE is
preferred to a false CURRENT result.

## Example layouts

Normal legacy project:

```text
repo/
  Cargo.toml
  src/
  docs/adr/
  .adrproof/
```

Read-only external verification:

```text
readonly-project/                 # project_root
external-verification/specs/      # specification_root
external-verification/state/      # state_root
```

CI:

```text
checkout/backend/                 # project_root
verification/specs/               # specification_root
ci-artifacts/adrproof-state/       # state_root
```

## Nesting and write safety

- A spec root inside the project is supported; both are inputs.
- An external state root is recommended.
- Explicit state equal to project or specification root is invalid.
- State inside project or specification is compatibility-supported but warns.
- Project inside state is allowed but discouraged; sibling roots are clearer.
- Legacy `.adrproof` inside the single root remains supported.

Only `state_root` reaches ADRProof write operations. Cargo runs as `cargo metadata
--format-version 1 --no-deps --offline`; ADRProof creates no file under an
explicitly separate project root.

## Facts summary

Full `facts` output remains available. A deterministic compact view is:

```text
adrproof facts --project-root PROJECT --summary
adrproof facts --project-root PROJECT --summary --json
```

It reports roots, provider/command identity, counts by relation, and precise
coverage claims. Unlisted domains are not implicitly closed.
