use loams_identity_core::{contracts::*, *};
use loams_identity_sim::*;
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

// Every M1.2 fake completes by explicit steps. No timer, runtime, thread, or OS source needed.
#[cfg(test)]
fn run<T>(future: impl Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("fake unexpectedly depends on an external wakeup"),
    }
}
#[test]
fn clock_contract() {
    let time = SimClock::default();
    clock(
        &time,
        |ns| time.advance(ns).unwrap(),
        |wall| time.jump(wall),
    );
}
#[test]
fn rng_contract() {
    rng(|| Box::new(SeededRng::new([42; 32]))).unwrap();
}
#[test]
fn key_store_contract() {
    run(keys(&mut SimKeyStore::new([42; 32]))).unwrap();
}
#[test]
fn cache_contract() {
    let time = SimClock::default();
    run(cache(&mut MemoryCache::new(time.clone()), |ns| {
        time.advance(ns).unwrap()
    }))
    .unwrap();
}
fn response(body: &[u8]) -> HttpResponse {
    HttpResponse {
        status: 200,
        headers: Default::default(),
        body: SecretBytes::new(body.to_vec()),
    }
}
fn step(connect_ns: u64, latency_ns: u64, response: Result<HttpResponse, PortError>) -> HttpStep {
    HttpStep {
        method: "GET".into(),
        url: http_request().url,
        connect_ns,
        latency_ns,
        response,
    }
}
#[test]
fn http_contract() {
    let time = SimClock::default();
    let mut client = ScriptedHttpClient::new(time.clone());
    client.script.extend([
        step(1, 1, Ok(response(b"ok"))),
        step(1, 10, Ok(response(b"ok"))),
        step(1, 1, Ok(response(&[0; 17]))),
    ]);
    run(http(&mut client)).unwrap();
    assert_eq!(client.requests.len(), 3);
    assert!(client.script.is_empty());
    assert_eq!(time.monotonic(), 14);
}
#[test]
fn mailer_contract() {
    let mut recorder = RecordingMailer::default();
    recorder
        .outcomes
        .extend([Ok(()), Err(PortError::Unavailable)]);
    run(mailer(&mut recorder)).unwrap();
    assert_eq!(recorder.attempts.len(), 2);
    assert_eq!(recorder.delivered.len(), 1);
    assert_eq!(recorder.delivered[0].to.expose(), mail().to.expose());
}
#[test]
fn event_sink_contract() {
    let mut recorder = RecordingEventSink::default();
    recorder.script.push_back(Delivery::Drop);
    run(sink(&mut recorder)).unwrap();
    assert_eq!(recorder.attempts.len(), 3);
    assert_eq!(recorder.delivered.len(), 2);
    assert!(recorder.delivered.iter().all(|e| e.id == "1"));
}
#[test]
fn authorizer_contract() {
    let mut recorder = RecordingAuthorizer::default();
    recorder
        .allowed
        .extend([permission("org:2"), permission("org:1")]);
    run(authorizer(&mut recorder)).unwrap();
    assert_eq!(recorder.checks.len(), 9);
}
#[test]
fn password_contract() {
    run(passwords(&FakePasswordHasher)).unwrap();
}

#[test]
fn delivery_faults_preserve_ids_and_reorder_explicitly() {
    let mut sink = RecordingEventSink::default();
    sink.script.extend([
        Delivery::LoseAck,
        Delivery::Duplicate,
        Delivery::Hold,
        Delivery::Hold,
    ]);
    assert_eq!(run(sink.deliver(exported("1"))), Err(PortError::Timeout));
    run(sink.deliver(exported("1"))).unwrap();
    run(sink.deliver(exported("2"))).unwrap();
    run(sink.deliver(exported("3"))).unwrap();
    assert_eq!(sink.pending(), 2);
    sink.release_reversed();
    assert_eq!(
        sink.delivered
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["1", "1", "1", "3", "2"]
    );
    assert_eq!(sink.pending(), 0);
}
#[test]
fn clock_overflow_is_atomic() {
    let time = SimClock::new(i64::MAX);
    assert_eq!(time.advance(1), Err(PortError::Capacity));
    assert_eq!(time.now(), i64::MAX);
    assert_eq!(time.monotonic(), 0);
    time.jump(i64::MIN);
    time.advance(u64::MAX).unwrap();
    assert_eq!(time.now(), i64::MAX);
    assert_eq!(time.monotonic(), u64::MAX);
    time.jump(0);
    assert_eq!(time.advance(1), Err(PortError::Capacity));
    assert_eq!(time.now(), 0);
}
#[test]
fn cache_expired_read_cannot_resurrect_after_backwards_jump() {
    let time = SimClock::default();
    let mut cache = MemoryCache::new(time.clone());
    let key = CacheKey {
        instance: "a".into(),
        namespace: "jwks".into(),
        key: "key".into(),
    };
    run(cache.put(key.clone(), SecretBytes::new(vec![1]), 5)).unwrap();
    time.advance(5).unwrap();
    assert!(run(cache.get(&key)).unwrap().is_none());
    time.jump(0);
    assert!(run(cache.get(&key)).unwrap().is_none());
}
#[test]
fn http_unexpected_requests_and_connection_deadlines() {
    let time = SimClock::default();
    let mut client = ScriptedHttpClient::new(time.clone());
    assert_eq!(
        run(client.execute(http_request())).err(),
        Some(PortError::UnexpectedOperation)
    );
    client.script.push_back(step(5, 0, Ok(response(b"ok"))));
    assert_eq!(
        run(client.execute(http_request())).err(),
        Some(PortError::Timeout)
    );
    assert_eq!(time.monotonic(), 5);
    client
        .script
        .push_back(step(0, 0, Err(PortError::Unavailable)));
    assert_eq!(
        run(client.execute(http_request())).err(),
        Some(PortError::Unavailable)
    );
    client.script.push_back(step(0, 0, Ok(response(b"ok"))));
    let mut request = http_request();
    request.method = "POST".into();
    assert_eq!(
        run(client.execute(request)).err(),
        Some(PortError::UnexpectedOperation)
    );
}
#[test]
fn key_store_same_seed_and_unknown_handles() {
    let mut first = SimKeyStore::new([1; 32]);
    let mut second = SimKeyStore::new([1; 32]);
    let mut different = SimKeyStore::new([2; 32]);
    let key = run(first.generate("a", Algorithm::EdDsa)).unwrap();
    assert_eq!(key, run(second.generate("a", Algorithm::EdDsa)).unwrap());
    assert_ne!(key, run(different.generate("a", Algorithm::EdDsa)).unwrap());
    let a = run(first.sign(&key.handle, Algorithm::EdDsa, b"test")).unwrap();
    let b = run(second.sign(&key.handle, Algorithm::EdDsa, b"test")).unwrap();
    assert_eq!(a.expose(), b.expose());
    let data = run(first.new_data_key("a", "index")).unwrap();
    let mut forged = data.handle.clone();
    forged.instance = "b".into();
    assert_eq!(
        run(first.destroy_data_key(&forged)),
        Err(PortError::KeyNotFound)
    );
    assert_eq!(
        run(first.hmac(&key.handle, b"test")).err(),
        Some(PortError::KeyNotFound)
    );
    assert_eq!(
        run(first.unwrap_data_key(&SecretBytes::new(vec![0; 32]))).err(),
        Some(PortError::KeyNotFound)
    );
    assert_eq!(
        run(first.new_data_key("a", "")).err(),
        Some(PortError::InvalidInput)
    );
}
#[test]
fn authorizer_denies_by_default_and_propagates_failures() {
    let mut auth = RecordingAuthorizer::default();
    assert!(!run(auth.check(permission("org:1"))).unwrap());
    auth.allowed.insert(permission("org:1"));
    auth.outcomes.push_back(Err(PortError::Unavailable));
    assert_eq!(
        run(auth.check(permission("org:1"))),
        Err(PortError::Unavailable)
    );
    auth.outcomes.push_back(Err(PortError::Timeout));
    assert_eq!(
        run(auth.list_objects("a", &permission("org:1").subject, "read", "org")),
        Err(PortError::Timeout)
    );
}
#[test]
fn secret_debug_output_is_redacted_in_nested_types() {
    let secret = "never-log-this";
    let bytes = || SecretBytes::new(secret.as_bytes().to_vec());
    let mut request = http_request();
    request.url = bytes();
    request.headers.insert("authorization".into(), bytes());
    request.body = bytes();
    let mut message = mail();
    message.to = bytes();
    message.parameters.insert("otp".into(), bytes());
    let mut event = exported("1");
    event.payload = bytes();
    let mut subject = permission("org:1").subject;
    subject.session = Some(SessionId(secret.into()));
    for debug in [
        format!("{:?}", bytes()),
        format!("{request:?}"),
        format!("{message:?}"),
        format!("{event:?}"),
        format!("{subject:?}"),
    ] {
        assert!(!debug.contains(secret));
        assert!(debug.contains("REDACTED"));
    }
}
