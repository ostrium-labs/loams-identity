# ADR 0028: Loams Access principals (install, device) and device trust bootstrap

- Status: Accepted (coordinator on the owner's "approve all", 2026-11-02)
- Date: 2026-11-02

## Context

On 2026-11-02 the owner decided that **Loams Access** is this service plus OpenFGA plus the Loams
Meter, in every home's platform base. The three share one tenant tree,
`org → project → {install, device, user, agent}`, and one id scheme. Creating or deleting a
subject creates or deletes its identity, its OpenFGA tuples and its Meter subject together, from
one operator reconcile or one API call. Each home runs its own identity provider; installs and
devices get machine identities; and the mTLS trust root for devices comes from the customer's own
home (Loams design 86 §4.2, §4.3 and §7; design 83 §6).

The specification was written before that decision. The table below lists each Loams need, where
the specification covers it today, and the gap.

| # | Loams need (source) | Covered today | Gap | Resolution |
|---|---|---|---|---|
| 1 | Installs and devices are principals with machine identities (86 D2633) | Machine users with purpose `service_account` or `agent` ([13](../../spec/13-machine-identity.md) §1); `principal` has three values ([15](../../spec/15-loams-integration.md) §2.1, [08](../../spec/08-tokens.md) §2) | No kind for an install or a device, and no credential rules for a principal an attacker can touch | Two new purposes and `principal` values, `install` and `device`, holding machine keys only (MACH-57, MACH-59, MACH-61) |
| 2 | Each install and device sits under one Loams project (86 D2633) | `loams_project_ref`, opaque, optional and changeable, on agents and service accounts (LOAMS-19); the identity org is the Loams org (TEN-6) | Not required, and can change | Required and fixed for installs and devices (MACH-57). No identity project is involved, so owner decision 10 and LOAMS-19a stand |
| 3 | One id scheme across identity, OpenFGA and the Meter (86 D2633) | `sub` is the user id, and identity-sync uses it in tuples (15 §3.6) | Not stated for installs and devices | Loams uses the machine user's id as the install's or device's id in tokens, tuples and the Meter (LOAMS-29) |
| 4 | Create and remove from one operator reconcile or one API call (86 D2633) | `POST /orgs/{org_id}/machine-users`, then a second call to add a machine key; `Idempotency-Key` for 24 hours (API-11) | Two calls before an identity is usable, and no durable key that a level-based reconciler can use to find what it created | The first machine key goes in the create call; an optional `loams_resource_ref`, unique per org, turns a repeated create into "here is the existing one" (MACH-58, MACH-60, API-27) |
| 5 | Device credential bootstrap: QR code, BLE, one-time join token (86 D2618, D2619) | Machine keys registered through the management API; the server never generates private keys (MACH-6) | How the device's key reaches this service at setup | No new endpoint here. The signed-in setup app sends the device's public key to Loams Access, which creates the device with that key in one call (MACH-60, LOAMS-29). The one-time join token stays on the Loams side |
| 6 | A short-lived device mTLS certificate whose trust root is the customer's home (86 D2619, D2630; 83 §6.2 to §6.4) | Nothing. [20](../../spec/20-later-standards.md) §11 puts mutual-TLS client authentication out of scope | Who issues device certificates, and how the issuer knows which device it is talking to | No CA in this service. The design-83 CA service in the same home issues the certificate, and authenticates the device or install with an access token from this service whose audience is the CA (LOAMS-30) |
| 7 | Revoking a device stops its certificate (83 §6.5, D2312) | `user.deactivated` and `user.removed` reach the sync feed and Loams' deny set (LOAMS-6, MACH-56) | No rule that connects them to the CA's deny list | Consumer rule: deny-list the certificate identity; device and install renewals present a fresh token (LOAMS-31) |
| 8 | identity-sync for the new principals (86 D2633) | The tuple table covers agents and service accounts (15 §3.6) | No rows for installs and devices | Rows added; `user.created` carries both references ([04](../../spec/04-event-catalog.md) §3.3) |
| 9 | One `io.loams.dev.access.*` audit stream (86 D2633) | The sync feed uses `dev.loams.identity.*` (LOAMS-10); audit records (LOAMS-22) | How the two relate | This service keeps its own types. Loams Access builds its stream from this service's feed, OpenFGA and the Meter, and keeps each event's `source` and `id` (LOAMS-32) |
| 10 | Identity lives in each home; `auth.loams.dev` serves loams.dev only, and a deploy never needs it (86 D2630) | LOAMS-23 (hosted instance for "the console and the site"), LOAMS-24 (a self-hosted install may trust `auth.loams.dev` to sign people in), LOAMS-25 (compose profile) | **Direct conflict** with LOAMS-24, and with LOAMS-23's console wording | LOAMS-24 withdrawn, LOAMS-23 amended, LOAMS-28 added |
| 11 | Official Zitadel behind the same port until this service passes conformance; the switch is a configuration change plus a data migration (86 D2632) | LOAMS-26 sets the gate for `auth.loams.dev` only | The gate for homes; no migration specification | LOAMS-26 extended to homes. The data migration is left open (see Consequences) |
| 12 | A laptop joins by signing in through Desktop (86 D2618) | Authorization code with PKCE for native clients ([07](../../spec/07-oauth-core.md)); device flow (MACH-47) | None for the human sign-in | No change. The laptop's own identity is created as in row 5 |

Owner decision 10 (one identity project `loams` per instance; Loams projects stay in the control
plane and OpenFGA; identity projects and role assignments never become tuples), LOAMS-6a and
LOAMS-19a are not in conflict with design 86 and are unchanged. The Loams project of an install or
device is the opaque `loams_project_ref`, the same mechanism agents already use, not an identity
project.

## Decision

1. **Two new machine-user purposes, `install` and `device`**, with matching `principal` claim
   values (MACH-57, MACH-61). An install is the machine identity of one listing installed in a home
   by the operator; a device is the machine identity of one laptop, instrument or sensor running
   `loams-node`.
2. **Bound to one Loams project, for life.** `loams_project_ref` is required at creation and never
   changes (MACH-57). Moving a device between projects is remove and create.
3. **Machine keys only.** Installs and devices cannot hold client secrets, PATs or trust policies
   (MACH-59, SEC-41). The private key is generated on the device or by the operator and never sent
   to this service (MACH-6).
4. **One call creates a usable identity.** The create request carries the first public key
   (`initial_machine_key`) and an optional `loams_resource_ref` that is unique per org. A create
   with a taken reference answers `409` with the existing id, so a reconciler can create or find
   in one call (MACH-58, MACH-60, API-27). `DELETE /users/{id}` remains the one call that removes.
5. **No certificate authority and no enrolment endpoint in this service.** The design-83 CA service
   in the same home is the trust root and issues device and install certificates. It authenticates
   the request with an access token from this service, minted by the JWT bearer grant against the
   device's machine key, with the CA's resource identifier as audience (LOAMS-30). The one-time
   join token of design 86 D2619 (tailnet pre-auth key and CA join material) is created and checked
   on the Loams side.
6. **Revocation reaches certificates through the existing sync feed.** Loams deny-lists the
   certificate identity of a deactivated or removed install or device, and the CA MUST require a
   fresh access token for an active principal when it renews their certificates, so identity
   state is checked at least once per certificate lifetime even if the feed lags (LOAMS-31).
   Reactivation restores the tuples, the Meter subject and the CA entry; the deactivation and
   reactivation events carry the purpose and both references so Loams can do this without
   earlier state (LOAMS-33).
7. **The Access audit stream is built by Loams.** This service keeps `dev.loams.identity.*` and the
   LOAMS-22 record shape. Loams Access maps them into `io.loams.dev.access.*` (LOAMS-32).
8. **Identity per home.** LOAMS-24 is withdrawn; LOAMS-23 is narrowed to loams.dev itself; LOAMS-28
   says that a home runs its own instance and never depends on `auth.loams.dev`. LOAMS-26's gate
   applies to homes as well.

The new work lands in Phase 3 (M3.1a and M3.9a), so Phases 1 and 2 stay as planned.

## Alternatives considered

- **Reuse `service_account` for installs and devices, with a tag.** No new values, but Loams could
  not tell an install from a service account in a token without a lookup, the OpenFGA type would
  have to come from somewhere else, and service accounts may hold client secrets and PATs. A device
  must not.
- **One `workload` purpose with a sub-kind.** Fewer purposes, but the sub-kind would need its own
  claim, and design 86 names installs and devices as separate kinds in the tenant tree.
- **A CA inside this service** (identity signs device certificates and is their trust root). One
  fewer round trip for devices, but it duplicates design 83's per-home PKI (root custody, deny
  list, rotation commands), adds X.509 issuance and CSR parsing to an internet-facing service, and
  could not exist on the interim Zitadel path, so Loams would need design 83's CA for devices
  anyway.
- **An enrolment token issued by this service** (a one-time bearer token the device redeems to
  register its own key). Lets a device enrol without a signed-in app, but it is one more bearer
  secret carried over BLE and the tailnet, one more public endpoint and one more ephemeral
  aggregate, and it duplicates design 83's join token. The setup app already holds a signed-in
  human and the device's verified public key.
- **Verify device attestation here.** Design 86 has no manufacturer certificate chain; the setup
  app checks the device's key against the QR code's fingerprint. Nothing for this service to
  verify.
- **Certificate-bound access tokens for devices** ([RFC 8705]). Would stop a copied token being
  used without the device's certificate key. Deferred: tokens live 15 minutes, and 20 §11 keeps
  RFC 8705 out of scope until asked.
- **Caller-chosen ids for "one id scheme".** Would let Loams create the three records in any
  order, but a create that collides with an id in another org becomes an existence oracle across
  orgs (TEN-17), and ids come from `Clock` and `Rng` (DM-1, ADR 0013). `loams_resource_ref` gives
  the same idempotency without the oracle, because it is unique per org only.
- **Emit `io.loams.dev.access.*` from this service.** The Access stream joins three engines, and
  the interim Zitadel must feed it too, so the mapping belongs in Loams' adapter, not here.
- **A changeable `loams_project_ref` for devices.** Convenient, but the parent tuple would have to
  be replaced and usage already metered under the old project would be attributed ambiguously.

## Consequences

- **Loams-side changes** (recorded in 15 so they are not missed): the `Subject` kind gains
  `Install` and `Device`, and Loams' verifier must accept the new `principal` values before this
  service issues them (LOAMS-27); the CA gets a device and install issue path that checks a token
  (LOAMS-30) and requires a fresh token at renewal (LOAMS-31); identity-sync feeds the CA's deny
  list (LOAMS-31); Loams Access orders the three creates, cleans up orphans (LOAMS-29) and
  restores tuples and the Meter subject on reactivation (LOAMS-33).
- **Key rotation.** MACH-7 caps a machine key at one year (default 90 days). Loams Access rotates
  install and device keys through the management API before they expire. A device should use a
  rotatable key for its machine identity, separate from any fixed key whose fingerprint is printed
  on its QR code.
- **Permission breadth.** Loams Access needs `user.manage` in each org it manages, which also
  covers human users. *Decided by the coordinator, 2026-11-02:* accepted for Phase 3; a narrower
  permission for install and device machine users is parked.
- **Certificate revocation latency** is the sync feed's delivery time, bounded by the 24-hour leaf
  lifetime and the token check at renewal.
- **Data migration from the interim identity provider.** Design 86 D2632 says moving a home from
  Zitadel to this service is a configuration change plus a data migration. OpenFGA tuples and
  Meter subjects are keyed by subject ids, so either the ids survive the move or Loams re-keys.
  *Decided by the coordinator, 2026-11-02:* deferred to its own ADR, which must be accepted before
  any home switches. This ADR does not decide it.
- **Naming.** Design 86 names the Access stream `io.loams.dev.access.*`; this service's feed uses
  `dev.loams.identity.*` (LOAMS-10). *Decided by the coordinator, 2026-11-02:* keep both
  namespaces.
- Owner decision 10, LOAMS-6a and LOAMS-19a are unchanged.

## Derived from

- Loams design 86 §4.2, §4.3 and §7, and its decision rows D2617 to D2634, as Loams-side
  requirements.
- Loams design 83 §1 and §2 (summary, decision rows, scope), §6.1 to §6.7 (identities, CA, CA
  service, join flow, revocation, rotation, external CAs) and §7.3 (peer identity versus
  principal), as Loams-side requirements.
- [RFC 7523] (JWT bearer grant), [RFC 7638] (JWK thumbprint), [RFC 8707] (resource indicators),
  [RFC 9068] (JWT access tokens), [RFC 8705] (considered and deferred), [CloudEvents] 1.0.
- First principles for the credential rules of physically exposed principals. No other identity
  provider's source code or documentation was used.

[RFC 7523]: https://www.rfc-editor.org/rfc/rfc7523
[RFC 7638]: https://www.rfc-editor.org/rfc/rfc7638
[RFC 8705]: https://www.rfc-editor.org/rfc/rfc8705
[RFC 8707]: https://www.rfc-editor.org/rfc/rfc8707
[RFC 9068]: https://www.rfc-editor.org/rfc/rfc9068
[CloudEvents]: https://cloudevents.io/
