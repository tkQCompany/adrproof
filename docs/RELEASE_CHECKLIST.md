# Release checklist

The ordered stable-release procedure is
[`STABLE_0_2_RUNBOOK.md`](STABLE_0_2_RUNBOOK.md). This checklist is its compact
review surface.

## Every prerelease and release

- [ ] Working tree is clean and the release commit is reviewed.
- [ ] `cargo fmt --all -- --check` passes.
- [ ] `cargo clippy --locked --all-targets --all-features -- -D warnings` passes.
- [ ] `cargo test --locked --all-targets` passes.
- [ ] External-provider conformance and CLI tests pass.
- [ ] `cargo audit --deny warnings` passes with a current RustSec database.
- [ ] `cargo deny check licenses` accepts every resolved dependency license.
- [ ] Every repository JSON document parses.
- [ ] All tracked Markdown link targets and clean-checkout quick-start commands
  pass their CI smoke tests.
- [ ] Public files contain no customer code, names, paths, evidence, patches, or
  integration lock data.
- [ ] `CHANGELOG.md`, `ROADMAP.md`, package version, and `Cargo.lock` agree.
- [ ] CI passes on Linux, macOS, and Windows.
- [ ] CodeQL completes for Rust and Python with no release-blocking alert.
- [ ] Workflow policy confirms explicit permissions and full commit pins for
  every third-party GitHub Action.
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
  byte-for-byte identical, as are their release manifests.
- [ ] The generated archive, `.sha256` file, and release manifest are attached
  to the matching stable GitHub Release; no project-produced binary is attached.
- [ ] Supported platforms and known limitations agree with
  `docs/SUPPORTED_PLATFORMS.md` and current CI.
- [ ] The `0.2.0` GitHub milestone is complete.
