# ADR 0001: Clean-room process and provenance wall

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

loams-identity covers roughly the functional scope of Zitadel, which is licensed AGPL-3.0 from
v3. This project must stay Apache-2.0. Copying or paraphrasing Zitadel's code, or deriving our
design from it, could make our code a derivative work. A clean room is only as good as its
process.

## Decision

1. **Two roles, two sessions.** A spec-writer (Phase A) writes `spec/*.md` in its own words from
   standards and from Zitadel's public product documentation (observable behaviour only). An
   implementer (Phase B onward) works from `spec/` and the standards alone, and does not read
   Zitadel's documentation.
2. **Forbidden inputs for everyone:** Zitadel's source code and its forks, ports, decompiled
   binaries, code search results over them, and any text that walks through or quotes its
   internals (blog posts, issues, design documents, reports by people who read the source).
   Zitadel's Apache-2.0 protobuf API definitions may be used only in Phase 6, only as an external
   contract for a compatible API surface.
3. **Allowed inputs:** IETF RFCs and drafts, OpenID Foundation specifications, W3C and FIDO
   specifications, NIST and OWASP guidance, crate documentation, Zitadel's public product
   documentation (spec-writer only), and the Loams-side contracts in Loams designs 19 and 76.
4. **Provenance.** Every spec file ends with a Sources section, and `PROVENANCE.md` lists the
   sources per file and per ADR, plus a log of anything uncertain and how it was redesigned.
5. **Our own names.** Endpoint paths, claim names, scope names, role names, event names and API
   package names are chosen by us and differ from Zitadel's, so the observable surface does not
   reproduce its design choices beyond what the standards fix.
6. **Contributors** agree to the same rules (`CONTRIBUTING.md`), and the DCO sign-off is their
   certification that they have the right to submit the change.

## Alternatives considered

- **Fork Zitadel.** Rejected: AGPL-3.0, and the owner wants a Rust implementation.
- **Port Zitadel to Rust.** Rejected: a port is a derivative work.
- **Single role in one session.** Weaker evidence of independence. The owner asked for separate
  sessions.

## Consequences

- Some behaviour Zitadel users expect (claim names, scope names, API shapes) differs. Phase 6 may
  add a compatibility surface generated from the Apache-2.0 protos.
- The implementer may hit gaps in `spec/`. They fill them from the standards with the most
  conservative option and record it, and never look at Zitadel to resolve them.
- This process does not replace legal review.

## Derived from

The owner's mission prompt (2026-10-21), clean-room rules. General clean-room practice.
