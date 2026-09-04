# Security policy

## Supported versions

ADRProof is currently pre-1.0. Security fixes are applied to the latest release
and the default branch. Older development snapshots are not supported.

The currently published line is 0.2.0-beta.1; stable 0.2.0 has not been issued.
On release of stable 0.2.0, the latest 0.2.x replaces the beta as the supported
release. A new minor line requires an explicit update to this support policy;
there is no implied long-term support commitment for older snapshots.

## Reporting a vulnerability

Do not disclose a suspected vulnerability, credential, private evidence bundle,
or customer-specific input in a public issue.

Use GitHub private vulnerability reporting at:

<https://github.com/tkQCompany/adrproof/security/advisories/new>

Include the affected version or commit, impact, reproduction steps, and any
suggested mitigation. If private vulnerability reporting is unavailable, open a
public issue requesting a private contact channel without including sensitive
details.

Security reports will be acknowledged as soon as practical. Response and release
timelines depend on severity, reproducibility, and maintainer availability.

## Running external providers

Only configure providers whose code and distribution you trust. ADRProof
validates and bounds their protocol but does not sandbox filesystem, network, or
system access. Run untrusted provider code in a separate OS/container sandbox.
