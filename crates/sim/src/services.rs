use crate::{SimClock, basics::lock};
use loams_identity_core::*;
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Mutex,
};

/// One expected HTTP operation. Latency advances simulated time without sleeping.
pub struct HttpStep {
    pub method: String,
    pub url: SecretBytes,
    pub connect_ns: u64,
    pub latency_ns: u64,
    pub response: Result<HttpResponse, PortError>,
}
#[derive(Default)]
struct HttpState {
    script: VecDeque<HttpStep>,
    requests: Vec<HttpRequest>,
}
impl HttpState {
    fn execute(
        &mut self,
        clock: &SimClock,
        request: HttpRequest,
    ) -> Result<HttpResponse, PortError> {
        let Some(step) = self.script.pop_front() else {
            return Err(PortError::UnexpectedOperation);
        };
        let matches = step.method == request.method && step.url.expose() == request.url.expose();
        let connect_limit = request.connect_timeout_ns;
        let total_limit = request.timeout_ns;
        let max_body = request.max_response_bytes;
        self.requests.push(request);
        if !matches {
            return Err(PortError::UnexpectedOperation);
        }
        let elapsed = step
            .connect_ns
            .checked_add(step.latency_ns)
            .ok_or(PortError::Capacity)?;
        if connect_limit == 0 || total_limit == 0 {
            return Err(PortError::InvalidInput);
        }
        if step.connect_ns >= connect_limit {
            clock.advance(connect_limit.min(total_limit))?;
            return Err(PortError::Timeout);
        }
        if elapsed >= total_limit {
            clock.advance(total_limit)?;
            return Err(PortError::Timeout);
        }
        clock.advance(elapsed)?;
        let response = step.response?;
        if response.body.expose().len() > max_body {
            return Err(PortError::Capacity);
        }
        Ok(response)
    }
}
pub struct ScriptedHttpClient {
    clock: SimClock,
    state: Mutex<HttpState>,
}
impl ScriptedHttpClient {
    pub fn new(clock: SimClock) -> Self {
        Self {
            clock,
            state: Mutex::default(),
        }
    }
    /// Queue expected operations; they are consumed in order.
    pub fn extend_script(&self, steps: impl IntoIterator<Item = HttpStep>) {
        lock(&self.state).script.extend(steps);
    }
    pub fn push_step(&self, step: HttpStep) {
        lock(&self.state).script.push_back(step);
    }
    /// Scripted operations not yet consumed.
    pub fn remaining(&self) -> usize {
        lock(&self.state).script.len()
    }
    /// Every request seen, in order, including mismatched ones.
    pub fn requests(&self) -> Vec<HttpRequest> {
        lock(&self.state).requests.clone()
    }
}
impl HttpClient for ScriptedHttpClient {
    fn execute<'a>(&'a self, request: HttpRequest) -> Operation<'a, HttpResponse> {
        let result = lock(&self.state).execute(&self.clock, request);
        Box::pin(async move { result })
    }
}

#[derive(Default)]
struct MailState {
    outcomes: VecDeque<Result<(), PortError>>,
    attempts: Vec<Mail>,
    delivered: Vec<Mail>,
}
#[derive(Default)]
pub struct RecordingMailer(Mutex<MailState>);
impl RecordingMailer {
    /// Each outcome controls exactly one attempt. Empty means success.
    pub fn extend_outcomes(&self, outcomes: impl IntoIterator<Item = Result<(), PortError>>) {
        lock(&self.0).outcomes.extend(outcomes);
    }
    pub fn attempts(&self) -> Vec<Mail> {
        lock(&self.0).attempts.clone()
    }
    pub fn delivered(&self) -> Vec<Mail> {
        lock(&self.0).delivered.clone()
    }
}
impl Mailer for RecordingMailer {
    fn send<'a>(&'a self, mail: Mail) -> Operation<'a, ()> {
        let result = {
            let mut state = lock(&self.0);
            state.attempts.push(mail.clone());
            match state.outcomes.pop_front() {
                Some(Err(error)) => Err(error),
                _ => {
                    state.delivered.push(mail);
                    Ok(())
                }
            }
        };
        Box::pin(async move { result })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Delivery {
    Deliver,
    /// Delivery fails before acceptance, so a caller must retry.
    Drop,
    /// Accepted by the receiver, but acknowledgement is lost; retry may duplicate it.
    LoseAck,
    Duplicate,
    /// Accept into the simulated transport; the controller later releases it.
    Hold,
}
#[derive(Default)]
struct SinkState {
    script: VecDeque<Delivery>,
    attempts: Vec<ExportedEvent>,
    delivered: Vec<ExportedEvent>,
    held: Vec<ExportedEvent>,
}
#[derive(Default)]
pub struct RecordingEventSink(Mutex<SinkState>);
impl RecordingEventSink {
    /// Each entry controls exactly one delivery. Empty means `Delivery::Deliver`.
    pub fn extend_script(&self, script: impl IntoIterator<Item = Delivery>) {
        lock(&self.0).script.extend(script);
    }
    pub fn push_script(&self, delivery: Delivery) {
        lock(&self.0).script.push_back(delivery);
    }
    pub fn attempts(&self) -> Vec<ExportedEvent> {
        lock(&self.0).attempts.clone()
    }
    pub fn delivered(&self) -> Vec<ExportedEvent> {
        lock(&self.0).delivered.clone()
    }
    pub fn release_reversed(&self) {
        let mut state = lock(&self.0);
        let held: Vec<_> = state.held.drain(..).rev().collect();
        state.delivered.extend(held);
    }
    pub fn pending(&self) -> usize {
        lock(&self.0).held.len()
    }
}
impl EventSink for RecordingEventSink {
    fn deliver<'a>(&'a self, event: ExportedEvent) -> Operation<'a, ()> {
        let result = {
            let mut state = lock(&self.0);
            state.attempts.push(event.clone());
            match state.script.pop_front().unwrap_or(Delivery::Deliver) {
                Delivery::Drop => Err(PortError::Unavailable),
                Delivery::LoseAck => {
                    state.delivered.push(event);
                    Err(PortError::Timeout)
                }
                Delivery::Duplicate => {
                    state.delivered.push(event.clone());
                    state.delivered.push(event);
                    Ok(())
                }
                Delivery::Hold => {
                    state.held.push(event);
                    Ok(())
                }
                Delivery::Deliver => {
                    state.delivered.push(event);
                    Ok(())
                }
            }
        };
        Box::pin(async move { result })
    }
}

#[derive(Default)]
struct AuthState {
    allowed: BTreeSet<Permission>,
    checks: Vec<Permission>,
    outcomes: VecDeque<Result<(), PortError>>,
}
impl AuthState {
    fn check(&mut self, permission: Permission) -> Result<bool, PortError> {
        let allowed = self.allowed.contains(&permission);
        self.checks.push(permission);
        if let Some(outcome) = self.outcomes.pop_front() {
            outcome?;
        }
        Ok(allowed)
    }
}
#[derive(Default)]
pub struct RecordingAuthorizer(Mutex<AuthState>);
impl RecordingAuthorizer {
    pub fn allow(&self, permissions: impl IntoIterator<Item = Permission>) {
        lock(&self.0).allowed.extend(permissions);
    }
    /// Each outcome controls exactly one check or list call. Empty means success.
    pub fn extend_outcomes(&self, outcomes: impl IntoIterator<Item = Result<(), PortError>>) {
        lock(&self.0).outcomes.extend(outcomes);
    }
    pub fn checks(&self) -> Vec<Permission> {
        lock(&self.0).checks.clone()
    }
}
impl Authorizer for RecordingAuthorizer {
    fn check<'a>(&'a self, permission: Permission) -> Operation<'a, bool> {
        let result = lock(&self.0).check(permission);
        Box::pin(async move { result })
    }
    fn batch_check<'a>(&'a self, permissions: Vec<Permission>) -> Operation<'a, Vec<bool>> {
        let result = {
            let mut state = lock(&self.0);
            permissions
                .into_iter()
                .map(|permission| state.check(permission))
                .collect()
        };
        Box::pin(async move { result })
    }
    fn list_objects<'a>(
        &'a self,
        instance: &'a str,
        subject: &'a Subject,
        relation: &'a str,
        object_type: &'a str,
    ) -> Operation<'a, Vec<String>> {
        let result = (|| {
            let mut state = lock(&self.0);
            if let Some(outcome) = state.outcomes.pop_front() {
                outcome?;
            }
            let prefix = format!("{object_type}:");
            Ok(state
                .allowed
                .iter()
                .filter(|p| {
                    p.instance == instance
                        && &p.subject == subject
                        && p.relation == relation
                        && p.object.starts_with(&prefix)
                })
                .map(|p| p.object.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect())
        })();
        Box::pin(async move { result })
    }
}
