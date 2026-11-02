# C-dependency register

Every crate in `Cargo.lock` that compiles C, C++ or assembly, or links a system library, has an
entry here: why we accept it and how we would replace it
([ADR 0022](adr/0022-licence-policy-and-supply-chain.md)). A pull request that adds such a crate
adds its entry in the same pull request.

## How to find them

A crate that builds native code usually has a build dependency on `cc`, `cmake`, `bindgen` or
`pkg-config`, or sets `links` in its manifest. These commands list them:

```sh
cargo tree --workspace --all-features -e build -i cc
cargo tree --workspace --all-features -e build -i cmake
cargo tree --workspace --all-features -e build -i bindgen
cargo tree --workspace --all-features -e build -i pkg-config
cargo metadata --format-version 1 | jq -r '.packages[] | select(.links != null) | "\(.name) \(.version) links=\(.links)"'
```

A command that reports that the package is not found means no crate in the graph depends on it.

## Format

| Field | Content |
|---|---|
| Crate | Name and the version range in `Cargo.lock`. |
| Native code | What it compiles or links (for example the AWS-LC C and assembly sources, or the system OpenSSL). |
| Pulled in by | The workspace crate and the direct dependency that bring it in. |
| Why accepted | The reason, and the ADR or owner decision that accepts it. |
| Replacement | How we would remove it, and what that would cost. |
| Added | The pull request that added the entry. |

## Register

No entries. The workspace has no third-party dependencies yet (milestone M1.1).

| Crate | Native code | Pulled in by | Why accepted | Replacement | Added |
|---|---|---|---|---|---|

## Expected entries

ADR 0022 expects these when the crates that need them arrive. They are not in `Cargo.lock` yet.

- `aws-lc-sys`: RSA signing and the rustls crypto provider (ADR 0006, milestone M1.7).
- `openssl-sys`: through `webauthn-rs`, Phase 2, accepted in ADR 0007.
- `ring`, if any dependency pulls it in.

SAML's libxml2 and xmlsec1 are not accepted (ADR 0011).
