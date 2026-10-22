# Contributing to loams-identity

Thanks for your interest. The project is in Phase A (specifications), so the most useful
contributions now are reviews of [spec/](spec/00-index.md), corrections with a source, and
questions about anything ambiguous.

## Clean-room rules (required)

loams-identity covers similar ground to Zitadel, which is licensed AGPL-3.0 from v3. To keep this
project Apache-2.0 and independent, every contributor follows these rules
([ADR 0001](docs/adr/0001-clean-room-process.md)):

1. **Never read Zitadel's source code**, or the source of its forks and ports, and never look at
   decompiled Zitadel binaries. This includes browsing its repository on GitHub, GitHub code
   search results that show its files, and copies of its code anywhere else.
2. **Never use material that quotes or walks through its internals**: blog posts, issues, pull
   requests, design notes or reports written by someone who read the source.
3. **Do not reproduce internals you happen to know.** If you already know how Zitadel implements
   something, design it from the standards instead, and say so in the pull request.
4. **Allowed sources:** IETF RFCs and drafts, OpenID Foundation specifications, W3C and FIDO
   specifications, NIST and OWASP guidance, the documentation of the crates we use, and (for
   specification writers only) Zitadel's public product documentation, for observable behaviour.
5. **Cite your sources.** A change to `spec/` updates the file's Sources section and
   [PROVENANCE.md](PROVENANCE.md). A non-obvious decision gets an ADR in `docs/adr/` with a
   "Derived from" section.
6. **Implementers work from `spec/` and the standards**, not from Zitadel's documentation. If the
   spec is unclear, open an issue; do not resolve it by looking at another implementation.
7. If you are unsure whether something you know comes from a forbidden source, say so in the pull
   request. We will redesign it and note it in the provenance log.

These rules support, but do not replace, legal review.

## Developer Certificate of Origin (DCO)

Every commit must be signed off under the
[Developer Certificate of Origin 1.1](https://developercertificate.org/). Signing off certifies
that you wrote the change, or otherwise have the right to submit it under this project's licence,
and, here, that you followed the clean-room rules above. Add it with `git commit -s`:

```text
Signed-off-by: Your Name <you@example.com>
```

The name and email must match the commit author. The `DCO sign-off` check rejects pull requests
with unsigned commits. To fix one, amend with `git commit --amend -s` (or `git rebase --signoff`
for several commits) and force-push your branch.

## Pull requests

- Open an issue before large changes (a new spec file, a change of direction, or more than a few
  hundred lines).
- Keep each pull request to one subject.
- Markdown must pass the lint and link checks in CI.

## Dependencies (from Phase 1)

Only crates under Apache-2.0, MIT, BSD, ISC or MPL-2.0 (plus the scoped exceptions in
[ADR 0022](docs/adr/0022-licence-policy-and-supply-chain.md)). No GPL, AGPL or LGPL. A crate that
compiles C code needs an entry in the C-dependency register. `cargo deny check` enforces the
licence list.

## Code review

Every pull request gets one AI reviewer, never two. In this repository it is
[Greptile](https://www.greptile.com), configured in [`.greptile/`](.greptile/config.json); it
reviews pull requests into `main` when they are opened (comment `@greptileai` to re-run it).
CodeRabbit's automatic review is off ([`.coderabbit.yaml`](.coderabbit.yaml)); it runs only if
someone comments `@coderabbitai review`. A maintainer reviews after the bot. Answer each bot
comment; it is fine to disagree and say why.

## Licence of contributions

Unless you explicitly state otherwise, any contribution you intentionally submit for inclusion in
loams-identity, as defined in the Apache-2.0 licence, is licensed under [Apache-2.0](LICENSE),
without any additional terms or conditions.

## Writing style

Plain, clear English. Cite a source for every fact about a standard or another project. When
something is a goal rather than a measurement, say so.
