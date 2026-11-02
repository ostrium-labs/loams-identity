//! Server-rendered login, consent, MFA and logout pages.
//!
//! Specification: spec/11-login-and-passwords.md; ADR 0019.
//!
//! Reaches the outside world only through the traits in `loams-identity-core` (ARCH-2).
//! The determinism lint (ARCH-7, ADR 0016) applies to this crate.

// Test code is exempt from the determinism lint (ADR 0016).
#![cfg_attr(test, allow(clippy::disallowed_methods, clippy::disallowed_types))]
