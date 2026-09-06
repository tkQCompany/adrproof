# Stable 0.2 release runbook

> **HISTORICAL — INACTIVE.** Superseded by
> [ADRP-0009](adr/0009-long-term-maturity-and-project-independence.md).
> The first stable product target is 1.0 after sustained use in multiple projects.
> Do not execute the former 0.2 promotion instructions below. They are retained
> only as historical release-engineering context, not current authority.
> Follow the [framework plan](../AKTUALNY_PLAN_RAMOWY.md) and
> [release checklist](RELEASE_CHECKLIST.md); a release-specific 1.0 procedure
> must be reviewed before any version bump, tag or publication.

This runbook turns the stable-release gate into an ordered, auditable procedure.
It does not authorize a release: the maintainer owns the GO decision, push, tag,
GitHub Release, and attached artifacts. ADRProof is never published to crates.io.

Stable promotion has no calendar deadline or minimum waiting period. It depends
on demonstrated usefulness and specification-language maturity, not just green
tests or elapsed time. No community participation is assumed, and absence of
reported defects is not evidence of successful use. Development and separately
approved integration/CI adoption may continue before stable promotion.

## Preconditions

1. The maintainer has accepted a maturity review based on sustained use across
   real project changes. It records practical benefits, useful and misleading
   CI findings, missed violations, specification limitations, and rule-maintenance
   costs reported by the user and approved integration controllers. The syntax
   and semantics of supported specification languages are mature enough for a
   compatibility commitment, with evolution and migration expectations documented.
   A single successful pilot or a green test suite is not sufficient evidence.
2. No release-blocking defect is open against external-provider protocol v1,
   provider-check report v1, its diagnostic families, or supported portability
   behavior.
3. The private integration controller has approved exact ADRProof and consuming
   project commit identities for a repeated, isolated pilot.
4. The repeated pilot has passed without changing the consuming project's
   active checkout and has produced a sanitized result suitable for the public
   release record.
5. The `0.2.0` GitHub milestone contains no open release blocker.

If any precondition fails, stop. Do not reinterpret a v1 schema or weaken a test
to obtain a release.

## Prepare the release commit

1. Start from a clean `main` synchronized with `origin/main`.
2. Confirm that changes since `0.2.0-beta.1` are compatible fixes,
   documentation, tests, or release engineering only.
3. Set the Cargo package version to `0.2.0` and update `Cargo.lock` without
   publishing or contacting a registry unnecessarily.
4. Move the accumulated changelog entries into a dated `0.2.0` section.
5. Update `ROADMAP.md`, supported-platform documentation, `SECURITY.md` supported
   versions, and the release record using
   [`releases/BETA_REVIEW_TEMPLATE.md`](releases/BETA_REVIEW_TEMPLATE.md).
6. Run:

   ```sh
   cargo fmt --all -- --check
   cargo clippy --locked --all-targets --all-features -- -D warnings
   cargo test --locked --all-targets
   cargo audit --deny warnings
   cargo deny check licenses
   actionlint
   python3 scripts/check-markdown-links.py
   scripts/test-source-release.sh
   ```

7. Commit the reviewed release candidate with its development-provenance
   trailer. The maintainer pushes the commit.
8. Require all protected-branch CI and code-scanning checks to pass for that
   exact commit.

## Tag and reproduce source artifacts

1. The maintainer creates annotated tag `0.2.0` at the accepted release commit
   and pushes that exact tag.
2. In two clean directories, run:

   ```sh
   first="$(mktemp -d)"
   second="$(mktemp -d)"
   ./scripts/build-source-release.sh 0.2.0 "$first"
   ./scripts/build-source-release.sh 0.2.0 "$second"
   cmp "$first/adrproof-0.2.0-source.tar.gz" \
     "$second/adrproof-0.2.0-source.tar.gz"
   cmp "$first/adrproof-0.2.0-source.tar.gz.sha256" \
     "$second/adrproof-0.2.0-source.tar.gz.sha256"
   cmp "$first/adrproof-0.2.0-release-manifest.json" \
     "$second/adrproof-0.2.0-release-manifest.json"
   ```

3. Verify that the manifest records the tag's full commit and tree identities.
4. Verify the archive with its checksum file and inspect its file listing.
5. Attach the source archive, checksum, and release manifest to the matching
   GitHub Release. Do not attach project-produced binaries or a `.crate` file.

## Publish and close

1. The maintainer publishes the GitHub Release as stable.
2. Confirm that the Release points to tag `0.2.0` and exposes only the intended
   source artifacts plus GitHub's automatic source snapshots.
3. Record the public CI run, tag target, archive digest, repeated-pilot result,
   known limitations, and GO decision in the sanitized release record.
4. Close the `0.2.0` milestone.
5. Open the `0.2.x` maintenance line. Protocol v1 and report v1 remain frozen;
   incompatible machine-readable changes proceed under a new version.

At no point does this procedure run `cargo publish` or upload to crates.io.
