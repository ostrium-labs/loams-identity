# ADR 0018: Crypto-shredding of personal data in the event log

- Status: Proposed
- Date: 2026-10-21

## Context

The event log is immutable and kept forever, but users have a right to erasure (for example under
the GDPR). Rewriting history breaks event sourcing and audit.

## Decision

- Each human user gets a data key (256-bit, wrapped by `KeyStore`). Personal fields in that user's
  events are encrypted with AES-256-GCM under it, with the event id as associated data (DM-6).
- Erasure destroys the data key and appends `user.erased`; projections drop or scrub the user's
  personal fields.
- Unique indexes over personal values (login names, verified emails) store keyed HMACs, not the
  values, and are released on erasure.
- Instance-level personal fields (for example typed login names in observations for unknown
  users) use an instance data key that rotates every 90 days, with old keys destroyed after the
  observation retention period.
- The AEAD comes from RustCrypto (`aes-gcm`); nonces from `Rng`.

## Alternatives considered

- **Keep personal data out of events** (only in a mutable side table). Simpler erasure, but events
  are no longer a complete record, and projections cannot be rebuilt from the log alone.
- **Rewrite events on erasure.** Breaks immutability and audit.

## Consequences

Phase 1 must ship the data-key machinery even before any erasure API, so later erasure is
possible for every event ever written. Reading personal fields costs an unwrap per user, which the
`Cache` absorbs (unwrapped keys cached in memory for a short time, zeroized on eviction).

## Derived from

General crypto-shredding practice; [NIST SP 800-38D] (GCM); `aes-gcm` crate documentation.

[NIST SP 800-38D]: https://csrc.nist.gov/pubs/sp/800/38/d/final
