# ADR 0020: REST first with OpenAPI, gRPC with our own protos; compatible surface later

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

The Loams console, the Loams control plane, the CLI and operators need to manage orgs, users,
clients and settings. The owner's crate list has both `api-rest` and `api-grpc`.

## Decision

- **REST** (axum 0.8, JSON, OpenAPI 3.1 via utoipa 6) is the primary management API, under
  `/api/v1`. Errors use [RFC 9457] problem details.
- **gRPC** (tonic 0.14, prost 0.14) mirrors it with our own package `loams.identity.v1`, for the
  Loams control plane, which already speaks gRPC.
- Both call the same command and query layer in `core`, so they cannot drift; a test compares the
  operation lists.
- A **system API** for instance lifecycle (`/system/v1`) exists only on the private listener. It
  authenticates with short JWTs signed by **operator keys** (key pairs made with
  `loams-identity operator-key generate`; only the public half is registered), not with tenant
  tokens ([17](../../spec/17-management-api.md) API-4).
- A **Zitadel-API-compatible surface** generated from Zitadel's Apache-2.0 protobuf definitions is
  Phase 6 only (ADR 0001), and is a separate crate.

## Alternatives considered

- **gRPC only, with a REST gateway.** Adds a translation layer and makes curl use harder.
- **REST only.** The Loams control plane prefers gRPC.

## Consequences

Two transports to test. The OpenAPI document is published with each release.

## Derived from

[RFC 9457]; crate documentation for axum, utoipa, tonic and prost (versions checked on crates.io
2026-10-21: axum 0.8.9, utoipa 6.0.0, tonic 0.14.6, prost 0.14.4).

[RFC 9457]: https://www.rfc-editor.org/rfc/rfc9457
