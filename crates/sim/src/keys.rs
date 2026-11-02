//! Opaque deterministic key model for M1.2. No output is a usable JOSE signature or JWK.
//! Real software signing, sealing and interoperability belong to M1.7.
use crate::{
    SeededRng,
    basics::{digest, hex, lock},
};
use hmac::{Hmac, Mac};
use loams_identity_core::*;
use sha2::Sha256;
use std::{collections::BTreeMap, sync::Mutex};

struct SigningKey {
    public: PublicKey,
    secret: Option<SecretBytes>,
}
struct WrappedKey {
    handle: KeyHandle,
    secret: Option<SecretBytes>,
}
struct State {
    rng: SeededRng,
    signing: BTreeMap<KeyHandle, SigningKey>,
    data: BTreeMap<Vec<u8>, WrappedKey>,
}
impl State {
    fn bytes(&self) -> Result<Vec<u8>, PortError> {
        let mut bytes = vec![0; 32];
        self.rng.fill(&mut bytes)?;
        Ok(bytes)
    }
    fn handle(&self, instance: &str) -> Result<KeyHandle, PortError> {
        if instance.is_empty() {
            return Err(PortError::InvalidInput);
        }
        Ok(KeyHandle {
            instance: instance.into(),
            id: hex(&self.bytes()?),
        })
    }
    fn generate(&mut self, instance: &str, algorithm: Algorithm) -> Result<PublicKey, PortError> {
        let handle = self.handle(instance)?;
        let secret = SecretBytes::new(self.bytes()?);
        let public = PublicKey {
            handle: handle.clone(),
            algorithm,
            jwk: BTreeMap::from([
                ("kty".into(), "SIMULATION-ONLY".into()),
                ("kid".into(), handle.id.clone()),
                (
                    "public".into(),
                    hex(&digest(&[b"SIM-PUBLIC", secret.expose()])),
                ),
            ]),
        };
        self.signing.insert(
            handle,
            SigningKey {
                public: public.clone(),
                secret: Some(secret),
            },
        );
        Ok(public)
    }
    fn sign(
        &self,
        key: &KeyHandle,
        algorithm: Algorithm,
        message: &[u8],
    ) -> Result<SecretBytes, PortError> {
        let entry = self.signing.get(key).ok_or(PortError::KeyNotFound)?;
        let secret = entry.secret.as_ref().ok_or(PortError::KeyDestroyed)?;
        if entry.public.algorithm != algorithm {
            return Err(PortError::AlgorithmMismatch);
        }
        Ok(SecretBytes::new(digest(&[
            b"SIM-SIGNATURE",
            secret.expose(),
            message,
        ])))
    }
    fn new_data_key(&mut self, instance: &str, purpose: &str) -> Result<DataKey, PortError> {
        if purpose.is_empty() {
            return Err(PortError::InvalidInput);
        }
        let handle = self.handle(instance)?;
        let secret = SecretBytes::new(self.bytes()?);
        // Model a sealed opaque reference. It contains no data key and becomes useless on erasure.
        let wrapped = self.bytes()?;
        self.data.insert(
            wrapped.clone(),
            WrappedKey {
                handle: handle.clone(),
                secret: Some(secret),
            },
        );
        Ok(DataKey {
            handle,
            wrapped: SecretBytes::new(wrapped),
        })
    }
}
/// All state sits behind one lock, so concurrent callers see a single ordered operation stream.
pub struct SimKeyStore(Mutex<State>);
impl SimKeyStore {
    pub fn new(seed: [u8; 32]) -> Self {
        Self(Mutex::new(State {
            rng: SeededRng::new(seed),
            signing: BTreeMap::new(),
            data: BTreeMap::new(),
        }))
    }
}
impl KeyStore for SimKeyStore {
    fn generate<'a>(&'a self, instance: &'a str, algorithm: Algorithm) -> Operation<'a, PublicKey> {
        let result = lock(&self.0).generate(instance, algorithm);
        Box::pin(async move { result })
    }
    fn sign<'a>(
        &'a self,
        key: &'a KeyHandle,
        algorithm: Algorithm,
        message: &'a [u8],
    ) -> Operation<'a, SecretBytes> {
        let result = lock(&self.0).sign(key, algorithm, message);
        Box::pin(async move { result })
    }
    fn public_jwk<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, PublicKey> {
        let result = lock(&self.0)
            .signing
            .get(key)
            .map(|entry| entry.public.clone())
            .ok_or(PortError::KeyNotFound);
        Box::pin(async move { result })
    }
    fn destroy<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, ()> {
        let result = lock(&self.0)
            .signing
            .get_mut(key)
            .map(|entry| entry.secret = None)
            .ok_or(PortError::KeyNotFound);
        Box::pin(async move { result })
    }
    fn new_data_key<'a>(&'a self, instance: &'a str, purpose: &'a str) -> Operation<'a, DataKey> {
        let result = lock(&self.0).new_data_key(instance, purpose);
        Box::pin(async move { result })
    }
    fn unwrap_data_key<'a>(&'a self, wrapped: &'a SecretBytes) -> Operation<'a, SecretBytes> {
        let result = lock(&self.0)
            .data
            .get(wrapped.expose())
            .ok_or(PortError::KeyNotFound)
            .and_then(|entry| entry.secret.clone().ok_or(PortError::KeyDestroyed));
        Box::pin(async move { result })
    }
    fn destroy_data_key<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, ()> {
        let result = lock(&self.0)
            .data
            .values_mut()
            .find(|entry| &entry.handle == key)
            .map(|entry| entry.secret = None)
            .ok_or(PortError::KeyNotFound);
        Box::pin(async move { result })
    }
    fn hmac<'a>(&'a self, key: &'a KeyHandle, message: &'a [u8]) -> Operation<'a, SecretBytes> {
        let result = (|| {
            let state = lock(&self.0);
            let entry = state
                .data
                .values()
                .find(|entry| &entry.handle == key)
                .ok_or(PortError::KeyNotFound)?;
            let secret = entry.secret.as_ref().ok_or(PortError::KeyDestroyed)?;
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.expose())
                .map_err(|_| PortError::InvalidInput)?;
            mac.update(message);
            Ok(SecretBytes::new(mac.finalize().into_bytes().to_vec()))
        })();
        Box::pin(async move { result })
    }
}
