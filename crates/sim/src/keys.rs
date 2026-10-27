//! Opaque deterministic key model for M1.2. No output is a usable JOSE signature or JWK.
//! Real software signing, sealing and interoperability belong to M1.7.
use crate::{
    SeededRng,
    basics::{digest, hex},
};
use hmac::{Hmac, Mac};
use loams_identity_core::*;
use sha2::Sha256;
use std::collections::BTreeMap;

struct SigningKey {
    public: PublicKey,
    secret: Option<SecretBytes>,
}
struct WrappedKey {
    handle: KeyHandle,
    secret: Option<SecretBytes>,
}
pub struct SimKeyStore {
    rng: SeededRng,
    signing: BTreeMap<KeyHandle, SigningKey>,
    data: BTreeMap<Vec<u8>, WrappedKey>,
}
impl SimKeyStore {
    pub fn new(seed: [u8; 32]) -> Self {
        Self {
            rng: SeededRng::new(seed),
            signing: BTreeMap::new(),
            data: BTreeMap::new(),
        }
    }
    fn bytes(&mut self) -> Result<Vec<u8>, PortError> {
        let mut bytes = vec![0; 32];
        self.rng.fill(&mut bytes)?;
        Ok(bytes)
    }
    fn handle(&mut self, instance: &str) -> Result<KeyHandle, PortError> {
        if instance.is_empty() {
            return Err(PortError::InvalidInput);
        }
        Ok(KeyHandle {
            instance: instance.into(),
            id: hex(&self.bytes()?),
        })
    }
}
impl KeyStore for SimKeyStore {
    fn generate<'a>(
        &'a mut self,
        instance: &'a str,
        algorithm: Algorithm,
    ) -> Operation<'a, PublicKey> {
        Box::pin(async move {
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
        })
    }
    fn sign<'a>(
        &'a mut self,
        key: &'a KeyHandle,
        algorithm: Algorithm,
        message: &'a [u8],
    ) -> Operation<'a, SecretBytes> {
        Box::pin(async move {
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
        })
    }
    fn public_jwk<'a>(&'a self, key: &'a KeyHandle) -> Operation<'a, PublicKey> {
        Box::pin(async move {
            Ok(self
                .signing
                .get(key)
                .ok_or(PortError::KeyNotFound)?
                .public
                .clone())
        })
    }
    fn destroy<'a>(&'a mut self, key: &'a KeyHandle) -> Operation<'a, ()> {
        Box::pin(async move {
            self.signing
                .get_mut(key)
                .ok_or(PortError::KeyNotFound)?
                .secret = None;
            Ok(())
        })
    }
    fn new_data_key<'a>(
        &'a mut self,
        instance: &'a str,
        purpose: &'a str,
    ) -> Operation<'a, DataKey> {
        Box::pin(async move {
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
        })
    }
    fn unwrap_data_key<'a>(&'a self, wrapped: &'a SecretBytes) -> Operation<'a, SecretBytes> {
        Box::pin(async move {
            self.data
                .get(wrapped.expose())
                .ok_or(PortError::KeyNotFound)?
                .secret
                .clone()
                .ok_or(PortError::KeyDestroyed)
        })
    }
    fn destroy_data_key<'a>(&'a mut self, key: &'a KeyHandle) -> Operation<'a, ()> {
        Box::pin(async move {
            let entry = self
                .data
                .values_mut()
                .find(|entry| &entry.handle == key)
                .ok_or(PortError::KeyNotFound)?;
            entry.secret = None;
            Ok(())
        })
    }
    fn hmac<'a>(&'a self, key: &'a KeyHandle, message: &'a [u8]) -> Operation<'a, SecretBytes> {
        Box::pin(async move {
            let entry = self
                .data
                .values()
                .find(|entry| &entry.handle == key)
                .ok_or(PortError::KeyNotFound)?;
            let secret = entry.secret.as_ref().ok_or(PortError::KeyDestroyed)?;
            let mut mac = Hmac::<Sha256>::new_from_slice(secret.expose())
                .map_err(|_| PortError::InvalidInput)?;
            mac.update(message);
            Ok(SecretBytes::new(mac.finalize().into_bytes().to_vec()))
        })
    }
}
