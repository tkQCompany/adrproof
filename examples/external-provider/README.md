# External provider example

This example demonstrates the ADRProof external-provider v1 process boundary.
The project contains a neutral component manifest; the independent Python
provider converts it into `component_kind(Component, ComponentKind)` facts and a
Closed claim over that one manifest.

```sh
state="$(mktemp -d)"
cargo run -- provider check component-manifest --json \
  --project-root examples/external-provider/project \
  --spec-root examples/external-provider/spec \
  --state-root "$state"

cargo run -- facts --summary --json \
  --project-root examples/external-provider/project \
  --spec-root examples/external-provider/spec \
  --state-root "$state"

cargo run -- check \
  --project-root examples/external-provider/project \
  --spec-root examples/external-provider/spec \
  --state-root "$state"
```

The provider is intentionally small and uses only the Python standard library.
It is configured code, not a sandboxed or auto-discovered plugin.
