use loams_identity_core::*;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

/// Lock a fake's state, recovering it after a test-driver panic poisoned the lock.
/// Fakes never hold a guard across an `.await`: they lock, compute, unlock, then return.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[derive(Default)]
struct Time {
    wall: Timestamp,
    monotonic: u64,
}

/// Clones share time, letting caches and scripted services observe the driver's clock.
#[derive(Clone, Default)]
pub struct SimClock(Arc<Mutex<Time>>);
impl SimClock {
    pub fn new(wall: Timestamp) -> Self {
        Self(Arc::new(Mutex::new(Time { wall, monotonic: 0 })))
    }
    fn time(&self) -> MutexGuard<'_, Time> {
        // The state contains only scalar timestamps, so a poisoned lock is safe to recover.
        lock(&self.0)
    }
    /// Advance both clocks atomically, or change neither on overflow.
    pub fn advance(&self, ns: u64) -> Result<(), PortError> {
        let mut time = self.time();
        let wall = i128::from(time.wall) + i128::from(ns);
        let wall = i64::try_from(wall).map_err(|_| PortError::Capacity)?;
        let monotonic = time.monotonic.checked_add(ns).ok_or(PortError::Capacity)?;
        time.wall = wall;
        time.monotonic = monotonic;
        Ok(())
    }
    /// A wall-clock jump never changes monotonic time.
    pub fn jump(&self, wall: Timestamp) {
        self.time().wall = wall;
    }
}
impl Clock for SimClock {
    fn now(&self) -> Timestamp {
        self.time().wall
    }
    fn monotonic(&self) -> u64 {
        self.time().monotonic
    }
}

/// Reproducible ChaCha20 stream. Never use a fixed seed in a production random source.
/// Clones of an `Arc<SeededRng>` draw from one shared stream, in call order.
pub struct SeededRng(Mutex<ChaCha20Rng>);
impl SeededRng {
    pub fn new(seed: [u8; 32]) -> Self {
        Self(Mutex::new(ChaCha20Rng::from_seed(seed)))
    }
}
impl Rng for SeededRng {
    fn fill(&self, bytes: &mut [u8]) -> Result<(), PortError> {
        lock(&self.0).fill_bytes(bytes);
        Ok(())
    }
}

/// Effects apply at call time, not on poll (see `Operation`). Expiry uses `Clock::monotonic`.
pub struct MemoryCache {
    clock: SimClock,
    entries: Mutex<BTreeMap<CacheKey, (u64, SecretBytes)>>,
}
impl MemoryCache {
    pub fn new(clock: SimClock) -> Self {
        Self {
            clock,
            entries: Mutex::new(BTreeMap::new()),
        }
    }
}
impl Cache for MemoryCache {
    fn get<'a>(&'a self, key: &'a CacheKey) -> Operation<'a, Option<SecretBytes>> {
        let result = {
            let mut entries = lock(&self.entries);
            if entries
                .get(key)
                .is_some_and(|(deadline, _)| *deadline <= self.clock.monotonic())
            {
                entries.remove(key);
            }
            entries.get(key).map(|(_, value)| value.clone())
        };
        Box::pin(async move { Ok(result) })
    }
    fn put<'a>(&'a self, key: CacheKey, value: SecretBytes, ttl_ns: u64) -> Operation<'a, ()> {
        let result = {
            let mut entries = lock(&self.entries);
            match self.clock.monotonic().checked_add(ttl_ns) {
                None => Err(PortError::Capacity),
                Some(_) if ttl_ns == 0 => {
                    entries.remove(&key);
                    Ok(())
                }
                Some(deadline) => {
                    entries.insert(key, (deadline, value));
                    Ok(())
                }
            }
        };
        Box::pin(async move { result })
    }
    fn remove<'a>(&'a self, key: &'a CacheKey) -> Operation<'a, ()> {
        lock(&self.entries).remove(key);
        Box::pin(async move { Ok(()) })
    }
}

/// Length-framed digest for simulation identifiers and fake credentials, never real signatures.
pub(crate) fn digest(parts: &[&[u8]]) -> Vec<u8> {
    let mut hash = Sha256::new();
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    hash.finalize().to_vec()
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    text
}

/// Fast fake only. The SIM-PASSWORD format is deliberately incompatible with production PHC.
#[derive(Default)]
pub struct FakePasswordHasher;
impl FakePasswordHasher {
    fn encode(password: &SecretBytes, salt: &[u8], profile: PasswordProfile) -> SecretBytes {
        let tag = match profile {
            PasswordProfile::Standard => "standard",
            PasswordProfile::Small => "small",
        };
        let hashed = digest(&[b"SIM-PASSWORD", tag.as_bytes(), salt, password.expose()]);
        SecretBytes::new(format!("SIM-PASSWORD${tag}${}${}", hex(salt), hex(&hashed)).into_bytes())
    }
}
impl PasswordHasher for FakePasswordHasher {
    fn hash<'a>(
        &'a self,
        password: &'a SecretBytes,
        salt: &'a [u8],
        profile: PasswordProfile,
    ) -> Operation<'a, SecretBytes> {
        Box::pin(async move {
            if salt.len() != 16 {
                return Err(PortError::InvalidInput);
            }
            Ok(Self::encode(password, salt, profile))
        })
    }
    fn verify<'a>(
        &'a self,
        password: &'a SecretBytes,
        encoded: &'a SecretBytes,
        profile: PasswordProfile,
    ) -> Operation<'a, PasswordCheck> {
        Box::pin(async move {
            let invalid = || PortError::InvalidInput;
            let text = std::str::from_utf8(encoded.expose()).map_err(|_| invalid())?;
            let parts: Vec<_> = text.split('$').collect();
            if parts.len() != 4
                || parts[0] != "SIM-PASSWORD"
                || parts[2].len() != 32
                || parts[3].len() != 64
            {
                return Err(invalid());
            }
            let original = match parts[1] {
                "standard" => PasswordProfile::Standard,
                "small" => PasswordProfile::Small,
                _ => return Err(invalid()),
            };
            let salt: Result<Vec<u8>, _> = parts[2]
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    let s = std::str::from_utf8(pair).map_err(|_| invalid())?;
                    u8::from_str_radix(s, 16).map_err(|_| invalid())
                })
                .collect();
            let expected = Self::encode(password, &salt?, original);
            let matches = expected.expose() == encoded.expose();
            Ok(PasswordCheck {
                matches,
                needs_rehash: matches && original != profile,
            })
        })
    }
}
