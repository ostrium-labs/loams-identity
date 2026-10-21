# ADR 0024: In-process rate limits and store-backed lockout with timed locks

- Status: Accepted (owner, 2026-10-21)
- Date: 2026-10-21

## Context

Login, OTP, token and reset endpoints need throttling. Lockout protects accounts from guessing
but lets an attacker lock victims out. NIST SP 800-63B requires limiting consecutive failed
attempts on a single account (no more than 100) and suggests alternatives to hard lockout.

## Decision

- **Per-IP and per-client rate limits** in process with `governor` 0.10 (MIT), keyed by instance
  and IP (or client id), configured per endpoint. Multi-node deployments get approximate limits
  (per node); the gateway in front can add global limits.
- **Per-account failure counters** in the `MetaStore` ephemeral space (shared across nodes),
  separate for passwords and OTPs.
- After 3 consecutive password failures, each further attempt for that account waits an
  increasing delay (1s, 2s, 4s ... up to 30s), applied before the hash so it costs no CPU.
- At `password_failure_threshold` (default 10) the password factor is locked for `lock_duration`
  (default 15m): `user.locked` with `until`. At `otp_failure_threshold` (default 5) the OTP factor
  locks the same way. Locks expire by time or by admin unlock, or by a successful password reset.
- `admin_unlock_only` (default false) makes locks permanent until an admin unlocks.
- Independently of the timed locks, 100 consecutive failures without a success lock the factor
  until an admin unlock or a password reset, the upper bound NIST SP 800-63B sets
  ([11](../../spec/11-login-and-passwords.md)).
- Unknown login names get the same counters, delays and timing as real ones, so the lockout
  behaviour does not reveal which names exist.
- A lock on the password factor does not block passkey sign-in (DM-13), so a victim with a passkey
  is not locked out.
- Responses during a lock are the same as for a wrong password (SEC-31); the user learns of the
  lock only through a notification email.

## Alternatives considered

- **Permanent lock after N failures, admin unlock.** Strong against guessing, but a cheap
  denial-of-service against any known login name. Kept as an option.
- **Rate limits only, no account lockout.** Distributed guessing from many IPs defeats per-IP
  limits.

## Consequences

The threshold numbers are configurable per instance and org. Benchmarks must include the delay
path.

## Derived from

[NIST SP 800-63B] (rate limiting of failed attempts), [OWASP Authentication Cheat Sheet],
`governor` documentation, crates.io metadata checked 2026-10-21. Zitadel's public docs describe
separate password and OTP attempt limits with admin unlock ([Default settings]); our timed-lock
default and the delay ladder are our own choice.

[NIST SP 800-63B]: https://pages.nist.gov/800-63-4/sp800-63b.html
[OWASP Authentication Cheat Sheet]: https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html
[Default settings]: https://zitadel.com/docs/guides/manage/console/default-settings
