# ADR 0026: Login UI message catalogues in Project Fluent

- Status: Proposed
- Date: 2026-10-21

## Context

The login UI is localized ([11](../../spec/11-login-and-passwords.md) LOGIN-47): the language
comes from `ui_locales`, then `Accept-Language`, with English as the fallback. Messages need
plurals and variables, and translators need a readable format.

## Decision

- Message catalogues use [Project Fluent](https://projectfluent.org/) (`.ftl` files), loaded
  with `fluent-bundle` 0.16 (Apache-2.0 OR MIT), compiled into the binary.
- English is the only complete catalogue in Phase 1. A CI test fails if a template uses a message
  id missing from the English catalogue.
- Message texts are our own; none are copied from another product.

## Alternatives considered

- **gettext (`.po`).** Widely known, but plural and variable handling is weaker, and the Rust
  tooling is less maintained.
- **Plain TOML maps.** No plural rules.

## Consequences

Last releases of `fluent` (0.17.0) and `fluent-bundle` (0.16.0) were in May 2025. The format is
stable; if the crates stall, the catalogue files stay usable with another loader.

## Derived from

Project Fluent documentation; crates.io metadata checked 2026-10-21.
