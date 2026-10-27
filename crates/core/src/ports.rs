//! I/O boundaries from architecture §3–4. Store ports arrive with M1.3; P2 ports later.
use std::{collections::BTreeMap, fmt, future::Future, pin::Pin};
use zeroize::{Zeroize, Zeroizing};

/// UTC nanoseconds since the Unix epoch. Signed to support simulated backwards jumps.
pub type Timestamp = i64;
/// Object-safe, executor-independent asynchronous operations; never starts background work.
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
pub trait Rng: Send {
    /// Fill the entire buffer, or return an error. Production sources must be cryptographic.
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), PortError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Algorithm {
    EdDsa,
    Es256,
    Rs256,
}
/// Opaque, instance-scoped key identity; never private material.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KeyHandle {
    pub instance: String,
    pub id: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicKey {
    pub handle: KeyHandle,
    pub algorithm: Algorithm,
    /// Public JWK members only. Simulated keys use an explicit simulation-only key type.
    pub jwk: BTreeMap<String, String>,
}
#[derive(Debug)]
pub struct DataKey {
    pub handle: KeyHandle,
    pub wrapped: SecretBytes,
}
/// KEY-10: algorithm mismatch fails; destroyed keys can never sign or unwrap again.
/// Lifecycle policy belongs to core commands; private material stays inside the implementation.
pub trait KeyStore: Send {
    fn generate<'a>(
        &'a mut self,
        instance: &'a str,
        algorithm: Algorithm,
    ) -> Operation<'a, PublicKey>;
    fn sign<'a>(
        &'a mut self,
        key: &'a KeyHandle,
        algorithm: Algorithm,
        message: &'a [u8],
    ) -> Operation<'a, SecretBytes>;
    fn public_jwk<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, PublicKey>;
    fn destroy<'a>(&'a mut self, key: &'a KeyHandle) -> Operation<'a, ()>;
    fn new_data_key<'a>(
        &'a mut self,
        instance: &'a str,
        purpose: &'a str,
    ) -> Operation<'a, DataKey>;
    fn unwrap_data_key<'a>(&'a self, wrapped: &'a SecretBytes) -> Operation<'a, SecretBytes>;
    fn destroy_data_key<'a>(&'a mut self, key: &'a KeyHandle) -> Operation<'a, ()>;
    /// Uses an index-purpose data-key handle; rejects signing-key handles.
    fn hmac<'a>(&'a self, key: &'a KeyHandle, message: &'a [u8]) -> Operation<'a, SecretBytes>;
}

/// Namespacing is mandatory even for derived, non-authoritative data.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    pub instance: String,
    pub namespace: String,
    pub key: String,
}
pub trait Cache: Send {
    /// Expired entries (expires_at <= Clock::now) are misses.
    fn get<'a>(&'a mut self, key: &'a CacheKey) -> Operation<'a, Option<SecretBytes>>;
    fn put<'a>(
        &'a mut self,
        key: CacheKey,
        value: SecretBytes,
        expires_at: Timestamp,
    ) -> Operation<'a, ()>;
    fn remove<'a>(&'a mut self, key: &'a CacheKey) -> Operation<'a, ()>;
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
pub trait HttpClient: Send {
    fn execute<'a>(&'a mut self, request: HttpRequest) -> Operation<'a, HttpResponse>;
}
#[derive(Debug, Clone)]
pub struct Mail {
    pub instance: String,
    pub to: SecretBytes,
    pub template: String,
    pub parameters: BTreeMap<String, SecretBytes>,
}
pub trait Mailer: Send {
    fn send<'a>(&'a mut self, mail: Mail) -> Operation<'a, ()>;
}
/// Serialized export; typed CloudEvents and envelopes arrive in M1.3/M1.14.
#[derive(Debug, Clone)]
pub struct ExportedEvent {
    pub instance: String,
    pub id: String,
    pub payload: SecretBytes,
}
/// Success acknowledges delivery; retries can duplicate an event with the same id.
pub trait EventSink: Send {
    fn deliver<'a>(&'a mut self, event: ExportedEvent) -> Operation<'a, ()>;
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
/// Deny by default. Batch output preserves input order; lists are sorted and unique.
pub trait Authorizer: Send {
    fn check<'a>(&'a mut self, permission: Permission) -> Operation<'a, bool>;
    fn batch_check<'a>(&'a mut self, permissions: Vec<Permission>) -> Operation<'a, Vec<bool>>;
    fn list_objects<'a>(
        &'a mut self,
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
pub trait PasswordHasher: Send {
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
