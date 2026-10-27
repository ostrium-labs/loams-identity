//! Reusable implementation contracts (ARCH-6). Run against a fresh, isolated adapter.
//! Control hooks are explicit so real adapters may use a test clock or fixture configuration.
//! Assertions deliberately fail on contract violations; operational failures return typed errors.
use crate::*;

pub fn clock(clock: &impl Clock, mut advance: impl FnMut(u64), mut jump: impl FnMut(Timestamp)) {
    let wall = clock.now();
    let monotonic = clock.monotonic();
    assert_eq!(clock.now(), wall);
    advance(17);
    assert_eq!(clock.now(), wall + 17);
    assert_eq!(clock.monotonic(), monotonic + 17);
    jump(wall - 5);
    assert_eq!(clock.now(), wall - 5);
    assert_eq!(clock.monotonic(), monotonic + 17);
}

pub fn rng(mut factory: impl FnMut() -> Box<dyn Rng>) -> Result<(), PortError> {
    let mut first = factory();
    let mut second = factory();
    let mut a = [0; 64];
    let mut b = [0; 64];
    first.fill(&mut [])?;
    first.fill(&mut a)?;
    second.fill(&mut b)?;
    assert_eq!(a, b, "same seed must produce same bytes");
    assert!(a.iter().any(|b| *b != 0));
    first.fill(&mut b)?;
    assert_ne!(a, b, "the stream must advance");
    Ok(())
}

pub async fn keys(store: &mut dyn KeyStore) -> Result<(), PortError> {
    let mut handles = std::collections::BTreeSet::new();
    for algorithm in [Algorithm::EdDsa, Algorithm::Es256, Algorithm::Rs256] {
        let public = store.generate("instance-a", algorithm).await?;
        assert_eq!(public.algorithm, algorithm);
        assert_eq!(public.handle.instance, "instance-a");
        assert!(handles.insert(public.handle.clone()));
        assert_eq!(store.public_jwk(&public.handle).await?, public);
        for member in ["d", "p", "q", "dp", "dq", "qi", "k"] {
            assert!(
                !public.jwk.contains_key(member),
                "private JWK member exported"
            );
        }
        let signature = store.sign(&public.handle, algorithm, b"message").await?;
        assert!(!signature.expose().is_empty());
        assert_eq!(
            signature.expose(),
            store
                .sign(&public.handle, algorithm, b"message")
                .await?
                .expose()
        );
        assert_ne!(
            signature.expose(),
            store
                .sign(&public.handle, algorithm, b"different")
                .await?
                .expose()
        );
        let other = if algorithm == Algorithm::EdDsa {
            Algorithm::Es256
        } else {
            Algorithm::EdDsa
        };
        assert_eq!(
            store.sign(&public.handle, other, b"message").await.err(),
            Some(PortError::AlgorithmMismatch)
        );
        let mut wrong_tenant = public.handle.clone();
        wrong_tenant.instance = "instance-b".into();
        assert_eq!(
            store.sign(&wrong_tenant, algorithm, b"message").await.err(),
            Some(PortError::KeyNotFound)
        );
        store.destroy(&public.handle).await?;
        store.destroy(&public.handle).await?;
        assert_eq!(
            store
                .sign(&public.handle, algorithm, b"message")
                .await
                .err(),
            Some(PortError::KeyDestroyed)
        );
        // Public verification material remains available after private key erasure.
        assert_eq!(store.public_jwk(&public.handle).await?, public);
    }
    let data = store.new_data_key("instance-a", "index").await?;
    let plain = store.unwrap_data_key(&data.wrapped).await?;
    assert_eq!(plain.expose().len(), 32);
    assert_ne!(plain.expose(), data.wrapped.expose());
    assert_eq!(
        plain.expose(),
        store.unwrap_data_key(&data.wrapped).await?.expose()
    );
    let mac = store.hmac(&data.handle, b"email-index").await?;
    assert_eq!(mac.expose().len(), 32);
    assert_eq!(
        mac.expose(),
        store.hmac(&data.handle, b"email-index").await?.expose()
    );
    assert_ne!(
        mac.expose(),
        store.hmac(&data.handle, b"other-index").await?.expose()
    );
    store.destroy_data_key(&data.handle).await?;
    store.destroy_data_key(&data.handle).await?;
    assert_eq!(
        store.unwrap_data_key(&data.wrapped).await.err(),
        Some(PortError::KeyDestroyed)
    );
    assert_eq!(
        store.hmac(&data.handle, b"email-index").await.err(),
        Some(PortError::KeyDestroyed)
    );
    Ok(())
}

/// Fixture time starts at zero. `advance` changes the injected clock by the given nanoseconds.
pub async fn cache(cache: &mut dyn Cache, mut advance: impl FnMut(u64)) -> Result<(), PortError> {
    let key = CacheKey {
        instance: "a".into(),
        namespace: "jwks".into(),
        key: "issuer".into(),
    };
    assert!(cache.get(&key).await?.is_none());
    cache
        .put(key.clone(), SecretBytes::new(b"old".to_vec()), 10)
        .await?;
    cache
        .put(key.clone(), SecretBytes::new(b"new".to_vec()), 10)
        .await?;
    assert_eq!(
        cache.get(&key).await?.map(|v| v.expose().to_vec()),
        Some(b"new".to_vec())
    );
    let mut other = key.clone();
    other.instance = "b".into();
    assert!(cache.get(&other).await?.is_none());
    other = key.clone();
    other.namespace = "metadata".into();
    assert!(cache.get(&other).await?.is_none());
    advance(9);
    assert!(cache.get(&key).await?.is_some());
    advance(1);
    assert!(
        cache.get(&key).await?.is_none(),
        "expiry boundary must be a miss"
    );
    cache
        .put(key.clone(), SecretBytes::new(vec![1]), 10)
        .await?;
    assert!(
        cache.get(&key).await?.is_none(),
        "expired writes must not resurrect data"
    );
    cache
        .put(key.clone(), SecretBytes::new(vec![1]), 100)
        .await?;
    cache.remove(&key).await?;
    cache.remove(&key).await?;
    assert!(cache.get(&key).await?.is_none());
    Ok(())
}

pub fn http_request() -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url: SecretBytes::new(b"https://example.test/keys".to_vec()),
        headers: Default::default(),
        body: SecretBytes::new(vec![]),
        max_response_bytes: 16,
        connect_timeout_ns: 5,
        timeout_ns: 10,
        allow_private_network: false,
    }
}
/// Fixture returns 200/`ok`, then a Timeout, then a body larger than 16 bytes.
pub async fn http(client: &mut dyn HttpClient) -> Result<(), PortError> {
    let response = client.execute(http_request()).await?;
    assert_eq!(response.status, 200);
    assert_eq!(response.body.expose(), b"ok");
    assert_eq!(
        client.execute(http_request()).await.err(),
        Some(PortError::Timeout)
    );
    assert_eq!(
        client.execute(http_request()).await.err(),
        Some(PortError::Capacity)
    );
    Ok(())
}
pub fn mail() -> Mail {
    Mail {
        instance: "a".into(),
        to: SecretBytes::new(b"recipient@example.test".to_vec()),
        template: "verify-email".into(),
        parameters: Default::default(),
    }
}
/// Fixture accepts one message, then returns Unavailable. Implementation verifies delivery count.
pub async fn mailer(mailer: &mut dyn Mailer) -> Result<(), PortError> {
    mailer.send(mail()).await?;
    assert_eq!(
        mailer.send(mail()).await.err(),
        Some(PortError::Unavailable)
    );
    Ok(())
}
pub fn exported(id: &str) -> ExportedEvent {
    ExportedEvent {
        instance: "a".into(),
        id: id.into(),
        payload: SecretBytes::new(b"event".to_vec()),
    }
}
/// Fixture fails the first delivery, then accepts a retry and another delivery of the same id.
pub async fn sink(sink: &mut dyn EventSink) -> Result<(), PortError> {
    assert_eq!(
        sink.deliver(exported("1")).await.err(),
        Some(PortError::Unavailable)
    );
    sink.deliver(exported("1")).await?;
    sink.deliver(exported("1")).await?;
    Ok(())
}
pub fn permission(object: &str) -> Permission {
    Permission {
        instance: "a".into(),
        subject: Subject {
            kind: PrincipalKind::User,
            id: "user".into(),
            org: "org".into(),
            act: vec![],
            session: None,
        },
        relation: "read".into(),
        object: object.into(),
    }
}
/// Fixture grants only `permission("org:1")` and `permission("org:2")`.
pub async fn authorizer(authorizer: &mut dyn Authorizer) -> Result<(), PortError> {
    let yes = permission("org:1");
    let no = permission("org:3");
    assert!(authorizer.check(yes.clone()).await?);
    assert!(!authorizer.check(no.clone()).await?);
    let mut other = yes.clone();
    other.instance = "other".into();
    assert!(!authorizer.check(other).await?);
    let mut other = yes.clone();
    other.subject.org = "other".into();
    assert!(!authorizer.check(other).await?);
    let mut other = yes.clone();
    other.relation = "write".into();
    assert!(!authorizer.check(other).await?);
    let mut delegated = yes.clone();
    delegated.subject.act.push(no.subject.clone());
    assert!(
        !authorizer.check(delegated).await?,
        "actor chain must not be discarded"
    );
    assert_eq!(
        authorizer
            .batch_check(vec![yes.clone(), no, yes.clone()])
            .await?,
        vec![true, false, true]
    );
    assert!(authorizer.batch_check(vec![]).await?.is_empty());
    assert_eq!(
        authorizer
            .list_objects("a", &yes.subject, "read", "org")
            .await?,
        vec!["org:1", "org:2"]
    );
    assert!(
        authorizer
            .list_objects("other", &yes.subject, "read", "org")
            .await?
            .is_empty()
    );
    assert!(
        authorizer
            .list_objects("a", &yes.subject, "read", "user")
            .await?
            .is_empty()
    );
    Ok(())
}
pub async fn passwords(hasher: &dyn PasswordHasher) -> Result<(), PortError> {
    let password = SecretBytes::new(b"correct horse".to_vec());
    let wrong = SecretBytes::new(b"wrong".to_vec());
    let salt = [7; 16];
    let hash = hasher
        .hash(&password, &salt, PasswordProfile::Small)
        .await?;
    assert_eq!(
        hash.expose(),
        hasher
            .hash(&password, &salt, PasswordProfile::Small)
            .await?
            .expose()
    );
    assert_ne!(
        hash.expose(),
        hasher
            .hash(&password, &[8; 16], PasswordProfile::Small)
            .await?
            .expose()
    );
    let check = hasher
        .verify(&password, &hash, PasswordProfile::Small)
        .await?;
    assert!(check.matches);
    assert!(!check.needs_rehash);
    let check = hasher
        .verify(&password, &hash, PasswordProfile::Standard)
        .await?;
    assert!(check.matches);
    assert!(check.needs_rehash);
    let check = hasher
        .verify(&wrong, &hash, PasswordProfile::Standard)
        .await?;
    assert!(!check.matches);
    assert!(!check.needs_rehash);
    assert_eq!(
        hasher
            .hash(&password, &[7; 15], PasswordProfile::Small)
            .await
            .err(),
        Some(PortError::InvalidInput)
    );
    assert_eq!(
        hasher
            .verify(
                &password,
                &SecretBytes::new(b"malformed".to_vec()),
                PasswordProfile::Small
            )
            .await
            .err(),
        Some(PortError::InvalidInput)
    );
    Ok(())
}
