//! HTTP handlers for the OAuth 2.0 and OpenID Connect endpoints.
//!
//! Specification: spec/07-oauth-core.md, spec/08-tokens.md, spec/09-oidc.md.
//!
//! Reaches the outside world only through the traits in `loams-identity-core` (ARCH-2).
//! The determinism lint (ARCH-7, ADR 0016) applies to this crate.

// Test code is exempt from the determinism lint (ADR 0016).
#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]
