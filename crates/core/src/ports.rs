//! I/O boundaries from architecture §3–4. Store ports arrive with M1.3; P2 ports later.
use std::{collections::BTreeMap, fmt, future::Future, pin::Pin};
use zeroize::{Zeroize, Zeroizing};

/// UTC nanoseconds since the Unix epoch. Signed to support simulated backwards jumps.
pub type Timestamp = i64;
/// Object-safe, executor-independent asynchronous operations; never starts background work.
///
/// Effect timing: a real adapter performs its effect when the future is polled to completion.
/// A future that is dropped before completion may or may not have taken effect, so a caller must
/// treat that outcome as unknown. The simulated fakes apply effects at call time, before the
/// future is polled, and so cannot reveal a caller that depends on a dropped future doing nothing.
pub type Operation<'a, T> = Pin<Box<dyn Future<Output = Result<T, PortError>> + Send + 'a>>;

/// Stable error classes. Never carry provider responses, credentials, or personal data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PortError {
    #[error("dependency unavailable")]
    Unavailable,
    #[error("operation timed out")]
    Timeout,
    #[error("invalid input")]
    InvalidInput,
    #[error("key not found")]
    KeyNotFound,
    #[error("key destroyed")]
    KeyDestroyed,
    #[error("key algorithm mismatch")]
    AlgorithmMismatch,
    #[error("capacity exceeded")]
    Capacity,
    #[error("unexpected simulated operation")]
    UnexpectedOperation,
}

/// Secret-bearing data zeroizes on drop and always redacts Debug (ARCH-14).
#[derive(Clone)]
pub struct SecretBytes(Zeroizing<Vec<u8>>);
impl SecretBytes {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }
    pub fn expose(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretBytes([REDACTED])")
    }
}

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
    /// Nanoseconds since this clock's origin; wall-clock jumps must never affect it.
    fn monotonic(&self) -> u64;
}
pub trait Rng: Send + Sync {
    /// Fill the entire buffer, or return an error. Production sources must be cryptographic.
    fn fill(&self, bytes: &mut [u8]) -> Result<(), PortError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Algorithm {
    EdDsa,
    Es256,
    Rs256,
}
/// Opaque, instance-scoped key identity; never private material. Key handles are secret class
/// (DM-5), so Debug redacts the id and the id is zeroized on drop (ARCH-14), the same rule as
/// `SessionId`. Clones are independent copies and are each wiped when dropped.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyHandle {
    pub instance: String,
    pub id: String,
}
impl Drop for KeyHandle {
    fn drop(&mut self) {
        self.id.zeroize();
    }
}
impl fmt::Debug for KeyHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyHandle")
            .field("instance", &self.instance)
            .field("id", &"[REDACTED]")
            .finish()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    pub handle: KeyHandle,
    pub algorithm: Algorithm,
    /// Public key-material JWK members only (for example `kty`, `crv`, `x`, `y`, `n`, `e`).
    /// Never `kid`, `use` or `alg`: core mints the `kid` from `Rng` (KEY-4, 16 bytes, base64url,
    /// never derived from key material or the handle) and adds `kid`, `use` and `alg` when it
    /// builds the JWKS. Simulated keys use an explicit simulation-only key type.
    pub jwk: BTreeMap<String, String>,
}
#[derive(Debug)]
pub struct DataKey {
    pub handle: KeyHandle,
    pub wrapped: SecretBytes,
}
/// Purpose of the data key that backs login-name and email uniqueness indexes (DM-6, ADR 0018).
pub const DATA_KEY_PURPOSE_INDEX: &str = "index";
/// KEY-10: algorithm mismatch fails; destroyed keys can never sign or unwrap again.
/// Lifecycle policy belongs to core commands; private material stays inside the implementation.
///
/// Handle kinds are not interchangeable. `sign` and `public_jwk` on a data-key handle, and `hmac`
/// on a signing-key handle or on a data key whose purpose is not `DATA_KEY_PURPOSE_INDEX`, fail
/// with `AlgorithmMismatch`. A handle the store never issued (or issued for another instance) is
/// `KeyNotFound`, for every method including `destroy` and `destroy_data_key`.
/// `unwrap_data_key` on bytes the store did not issue (tampered, truncated, empty) fails with
/// `KeyNotFound`, or `InvalidInput` when the implementation can tell the envelope is malformed.
pub trait KeyStore: Send + Sync {
    fn generate<'a>(&'a self, instance: &'a str, algorithm: Algorithm) -> Operation<'a, PublicKey>;
    fn sign<'a>(
        &'a self,
        key: &'a KeyHandle,
        algorithm: Algorithm,
        message: &'a [u8],
    ) -> Operation<'a, SecretBytes>;
    fn public_jwk<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, PublicKey>;
    fn destroy<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, ()>;
    fn new_data_key<'a>(&'a self, instance: &'a str, purpose: &'a str) -> Operation<'a, DataKey>;
    fn unwrap_data_key<'a>(&'a self, wrapped: &'a SecretBytes) -> Operation<'a, SecretBytes>;
    fn destroy_data_key<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, ()>;
    /// Uses a data-key handle created with `DATA_KEY_PURPOSE_INDEX`; any other data-key purpose
    /// and any signing-key handle fail with `AlgorithmMismatch`.
    fn hmac<'a>(&'a self, key: &'a KeyHandle, message: &'a [u8]) -> Operation<'a, SecretBytes>;
}

/// Namespacing is mandatory even for derived, non-authoritative data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    pub instance: String,
    pub namespace: String,
    pub key: String,
}
pub trait Cache: Send + Sync {
    /// Expired entries are misses. An entry written with `ttl_ns` expires once `Clock::monotonic`
    /// has advanced `ttl_ns` past the write (ARCH-11), so a wall-clock jump never revives it;
    /// `ttl_ns == 0` stores nothing and removes any existing entry; a ttl that overflows monotonic
    /// time is `Capacity`. Time-to-live is capped by
    /// the callers' own limits (ADR 0021).
    fn get<'a>(&'a self, key: &'a CacheKey) -> Operation<'a, Option<SecretBytes>>;
    fn put<'a>(&'a self, key: CacheKey, value: SecretBytes, ttl_ns: u64) -> Operation<'a, ()>;
    fn remove<'a>(&'a self, key: &'a CacheKey) -> Operation<'a, ()>;
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    /// Redacted because URLs can contain query credentials or personal data.
    pub url: SecretBytes,
    pub headers: BTreeMap<String, SecretBytes>,
    pub body: SecretBytes,
    pub max_response_bytes: usize,
    pub connect_timeout_ns: u64,
    pub timeout_ns: u64,
    pub allow_private_network: bool,
}
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, SecretBytes>,
    pub body: SecretBytes,
}
/// One request, no redirects. Runtime validates and pins resolved addresses (LOAMS-21).
pub trait HttpClient: Send + Sync {
    fn execute<'a>(&'a self, request: HttpRequest) -> Operation<'a, HttpResponse>;
}
#[derive(Debug, Clone)]
pub struct Mail {
    pub instance: String,
    pub to: SecretBytes,
    pub template: String,
    pub parameters: BTreeMap<String, SecretBytes>,
}
pub trait Mailer: Send + Sync {
    fn send<'a>(&'a self, mail: Mail) -> Operation<'a, ()>;
}
/// Serialized export; typed CloudEvents and envelopes arrive in M1.3/M1.14.
#[derive(Debug, Clone)]
pub struct ExportedEvent {
    pub instance: String,
    pub id: String,
    pub payload: SecretBytes,
}
/// Success acknowledges delivery; retries can duplicate an event with the same id.
pub trait EventSink: Send + Sync {
    fn deliver<'a>(&'a self, event: ExportedEvent) -> Operation<'a, ()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrincipalKind {
    User,
    ServiceAccount,
    Agent,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Subject {
    pub kind: PrincipalKind,
    pub id: String,
    pub org: String,
    pub act: Vec<Subject>,
    // A session is a bearer-adjacent identifier and must not appear in Debug.
    pub session: Option<SessionId>,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SessionId(pub String);
impl Drop for SessionId {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}
impl fmt::Debug for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionId([REDACTED])")
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Permission {
    pub instance: String,
    pub subject: Subject,
    pub relation: String,
    pub object: String,
}
/// Deny by default. Lists are sorted and unique.
///
/// `batch_check` returns exactly one result per input, in input order. An implementation that
/// cannot produce that returns `Err`, never a short or padded vector (callers using `.all()` would
/// otherwise fail open).
///
/// The session does not take part in authorization matching: a grant on a subject applies
/// whether or not `Subject::session` is set, and the actor chain is matched without sessions too.
pub trait Authorizer: Send + Sync {
    fn check<'a>(&'a self, permission: Permission) -> Operation<'a, bool>;
    fn batch_check<'a>(&'a self, permissions: Vec<Permission>) -> Operation<'a, Vec<bool>>;
    fn list_objects<'a>(
        &'a self,
        instance: &'a str,
        subject: &'a Subject,
        relation: &'a str,
        object_type: &'a str,
    ) -> Operation<'a, Vec<String>>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordProfile {
    Standard,
    Small,
}
#[derive(Debug)]
pub struct PasswordCheck {
    pub matches: bool,
    pub needs_rehash: bool,
}
pub trait PasswordHasher: Send + Sync {
    /// Salt must be exactly 16 bytes from Rng (ADR 0009); this method hides no random draw.
    fn hash<'a>(
        &'a self,
        password: &'a SecretBytes,
        salt: &'a [u8],
        profile: PasswordProfile,
    ) -> Operation<'a, SecretBytes>;
    fn verify<'a>(
        &'a self,
        password: &'a SecretBytes,
        encoded: &'a SecretBytes,
        profile: PasswordProfile,
    ) -> Operation<'a, PasswordCheck>;
}
