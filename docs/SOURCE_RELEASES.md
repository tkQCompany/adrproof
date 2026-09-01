# Source release artifacts

ADRProof is distributed as source through GitHub. It is not published to
crates.io and does not ship project-produced binary artifacts.

GitHub automatically exposes source snapshots for every tag. Stable releases
also attach a project-produced `tar.gz` archive and a SHA-256 checksum so that a
consumer can verify a named, reproducible artifact independently of GitHub's
snapshot service.

## Build

Run the generator from any clean checkout that contains the release tag:

```sh
./scripts/build-source-release.sh 0.2.0 dist
```

The version and package name are read from `Cargo.toml` at the selected Git
reference, not from the active working tree. The archive contains only files
tracked by that reference, under an `adrproof-VERSION/` prefix. `git archive`
provides the canonical tree and file modes; `gzip -n` removes variable gzip
header metadata.

The command creates:

- `adrproof-VERSION-source.tar.gz`;
- `adrproof-VERSION-source.tar.gz.sha256`.

The archive is a source distribution, not a Cargo registry package. Running the
generator never invokes `cargo publish` or communicates with crates.io.

## Reproducibility gate

Build the same Git reference in two empty directories and compare both files:

```sh
first="$(mktemp -d)"
second="$(mktemp -d)"
./scripts/build-source-release.sh 0.2.0 "$first"
./scripts/build-source-release.sh 0.2.0 "$second"
cmp "$first/adrproof-0.2.0-source.tar.gz" \
  "$second/adrproof-0.2.0-source.tar.gz"
cmp "$first/adrproof-0.2.0-source.tar.gz.sha256" \
  "$second/adrproof-0.2.0-source.tar.gz.sha256"
```

CI applies the same two-build comparison to every proposed change. Before a
stable release, the maintainer repeats it for the annotated release tag and
uploads both generated files to the matching GitHub Release.

## Verify

On systems with GNU coreutils:

```sh
sha256sum -c adrproof-0.2.0-source.tar.gz.sha256
```

On macOS:

```sh
shasum -a 256 -c adrproof-0.2.0-source.tar.gz.sha256
```

The checksum applies to the attached project-produced archive. GitHub's
automatically generated `.zip` and `.tar.gz` snapshots are separate files and
are not expected to have this digest.
