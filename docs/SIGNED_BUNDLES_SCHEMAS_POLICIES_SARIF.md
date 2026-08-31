# Signed bundles, schemas, waiver policies, and SARIF

ADRProof evidence bundles may be signed with Ed25519:

```text
adrproof bundle create --output BUNDLE --signing-key PRIVATE_KEY ...
adrproof bundle verify BUNDLE --public-key TRUSTED_PUBLIC_KEY --require-signature
```

Keys are 32 raw bytes or 64 hexadecimal characters. `bundle.sig.json` binds the
exact `bundle.json` bytes, identifies the public key by SHA-256, and carries an
unpadded-base64 signature. Supplying a trusted public key is what turns
cryptographic self-consistency into signer authentication. Private keys are
never copied into a bundle or printed.

Portable JSON Schemas live in `schemas/`. Their identifiers are immutable;
incompatible formats require a new identifier and file. Runtime readers reject
unknown bundle, signature, native-report, and policy versions.

`adrproof diagnose --policy POLICY.json` accepts only explicit waivers with a
finding ID, owner, reason, and Unix expiry. A waiver does not rewrite the
underlying verifier status: a waived FAIL remains a FAIL in the finding and the
top-level diagnostic result becomes `WAIVED_ATTENTION`. Expired, duplicate, or
unmatched policy entries keep the diagnostic gate closed.

`adrproof diagnose --sarif report.sarif` writes SARIF 2.1.0. Accepted waivers
appear as external suppressions while the original finding remains embedded in
SARIF properties.
