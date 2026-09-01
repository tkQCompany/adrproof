# Release checklist

## Every prerelease and release

- [ ] Working tree is clean and the release commit is reviewed.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --locked --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --locked --all-targets` passes.
- [ ] External-provider conformance and CLI tests pass.
- [ ] `cargo audit --deny warnings` passes with a current RustSec database.
- [ ] Every repository JSON document parses.
- [ ] Public files contain no customer code, names, paths, evidence, patches, or
  integration lock data.
- [ ] `CHANGELOG.md`, `ROADMAP.md`, package version, and `Cargo.lock` agree.
- [ ] CI passes on Linux, macOS, and Windows.
- [ ] `Cargo.toml` still declares `publish = false`; no crates.io publication is
  attempted or configured.
- [ ] The maintainer performs the push and creates the release/tag.

## Beta gate

- [ ] Protocol v1, request/response schemas, check-report schema, exit behavior,
  and diagnostic families are frozen.
- [ ] A private commit-pinned cross-project pilot is complete.
- [ ] Every pilot defect has a neutral public reproduction or is classified as
  private-provider/project-specific.
- [ ] Documentation has been followed from a clean checkout.

## Stable 0.2 gate

- [ ] No release-blocking protocol defect remains.
- [ ] Only compatible fixes were made after beta.
- [ ] The source archive is generated twice from the release tag with
  `scripts/build-source-release.sh`; both archives and checksum files are
  byte-for-byte identical.
- [ ] The generated archive and its `.sha256` file are attached to the matching
  stable GitHub Release; no project-produced binary is attached.
- [ ] Supported platforms and known limitations agree with
  `docs/SUPPORTED_PLATFORMS.md` and current CI.
- [ ] The `0.2.0` GitHub milestone is complete.
