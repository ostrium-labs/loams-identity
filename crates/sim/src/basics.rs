use loams_identity_core::*;
use rand_chacha::ChaCha20Rng;
use rand_core::{RngCore, SeedableRng};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, MutexGuard},
};

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
        // The state contains only scalar timestamps. Recover it after a test-driver panic.
        match self.0.lock() {
            Ok(time) => time,
            Err(poisoned) => poisoned.into_inner(),
        }
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
pub struct SeededRng(ChaCha20Rng);
impl SeededRng {
    pub fn new(seed: [u8; 32]) -> Self {
        Self(ChaCha20Rng::from_seed(seed))
    }
}
impl Rng for SeededRng {
    fn fill(&mut self, bytes: &mut [u8]) -> Result<(), PortError> {
        self.0.fill_bytes(bytes);
        Ok(())
    }
}

pub struct MemoryCache {
    clock: SimClock,
    entries: BTreeMap<CacheKey, (Timestamp, SecretBytes)>,
}
impl MemoryCache {
    pub fn new(clock: SimClock) -> Self {
        Self {
            clock,
            entries: BTreeMap::new(),
        }
    }
}
impl Cache for MemoryCache {
    fn get<'a>(&'a mut self, key: &'a CacheKey) -> Operation<'a, Option<SecretBytes>> {
        Box::pin(async move {
            if self
                .entries
                .get(key)
                .is_some_and(|(expiry, _)| *expiry <= self.clock.now())
            {
                self.entries.remove(key);
            }
            Ok(self.entries.get(key).map(|(_, value)| value.clone()))
        })
    }
    fn put<'a>(
        &'a mut self,
        key: CacheKey,
        value: SecretBytes,
        expires_at: Timestamp,
    ) -> Operation<'a, ()> {
        Box::pin(async move {
            if expires_at <= self.clock.now() {
                self.entries.remove(&key);
            } else {
                self.entries.insert(key, (expires_at, value));
            }
            Ok(())
        })
    }
    fn remove<'a>(&'a mut self, key: &'a CacheKey) -> Operation<'a, ()> {
        Box::pin(async move {
            self.entries.remove(key);
            Ok(())
        })
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
