use crate::SimClock;
use loams_identity_core::*;
use std::collections::{BTreeSet, VecDeque};

/// One expected HTTP operation. Latency advances simulated time without sleeping.
pub struct HttpStep {
    pub method: String,
    pub url: SecretBytes,
    pub connect_ns: u64,
    pub latency_ns: u64,
    pub response: Result<HttpResponse, PortError>,
}
pub struct ScriptedHttpClient {
    clock: SimClock,
    pub script: VecDeque<HttpStep>,
    pub requests: Vec<HttpRequest>,
}
impl ScriptedHttpClient {
    pub fn new(clock: SimClock) -> Self {
        Self {
            clock,
            script: VecDeque::new(),
            requests: Vec::new(),
        }
    }
}
impl HttpClient for ScriptedHttpClient {
    fn execute<'a>(&'a mut self, request: HttpRequest) -> Operation<'a, HttpResponse> {
        Box::pin(async move {
            let Some(step) = self.script.pop_front() else {
                return Err(PortError::UnexpectedOperation);
            };
            let matches =
                step.method == request.method && step.url.expose() == request.url.expose();
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
                self.clock.advance(connect_limit.min(total_limit))?;
                return Err(PortError::Timeout);
            }
            if elapsed >= total_limit {
                self.clock.advance(total_limit)?;
                return Err(PortError::Timeout);
            }
            self.clock.advance(elapsed)?;
            let response = step.response?;
            if response.body.expose().len() > max_body {
                return Err(PortError::Capacity);
            }
            Ok(response)
        })
    }
}

#[derive(Default)]
pub struct RecordingMailer {
    /// Each result controls exactly one attempt. Empty means success.
    pub outcomes: VecDeque<Result<(), PortError>>,
    pub attempts: Vec<Mail>,
    pub delivered: Vec<Mail>,
}
impl Mailer for RecordingMailer {
    fn send<'a>(&'a mut self, mail: Mail) -> Operation<'a, ()> {
        Box::pin(async move {
            self.attempts.push(mail.clone());
            if let Some(outcome) = self.outcomes.pop_front() {
                outcome?;
            }
            self.delivered.push(mail);
            Ok(())
        })
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
pub struct RecordingEventSink {
    pub script: VecDeque<Delivery>,
    pub attempts: Vec<ExportedEvent>,
    pub delivered: Vec<ExportedEvent>,
    held: Vec<ExportedEvent>,
}
impl RecordingEventSink {
    pub fn release_reversed(&mut self) {
        self.delivered.extend(self.held.drain(..).rev());
    }
    pub fn pending(&self) -> usize {
        self.held.len()
    }
}
impl EventSink for RecordingEventSink {
    fn deliver<'a>(&'a mut self, event: ExportedEvent) -> Operation<'a, ()> {
        Box::pin(async move {
            self.attempts.push(event.clone());
            match self.script.pop_front().unwrap_or(Delivery::Deliver) {
                Delivery::Drop => return Err(PortError::Unavailable),
                Delivery::LoseAck => {
                    self.delivered.push(event);
                    return Err(PortError::Timeout);
                }
                Delivery::Duplicate => {
                    self.delivered.push(event.clone());
                    self.delivered.push(event);
                }
                Delivery::Hold => self.held.push(event),
                Delivery::Deliver => self.delivered.push(event),
            }
            Ok(())
        })
    }
}

#[derive(Default)]
pub struct RecordingAuthorizer {
    pub allowed: BTreeSet<Permission>,
    pub checks: Vec<Permission>,
    pub outcomes: VecDeque<Result<(), PortError>>,
}
impl Authorizer for RecordingAuthorizer {
    fn check<'a>(&'a mut self, permission: Permission) -> Operation<'a, bool> {
        Box::pin(async move {
            let allowed = self.allowed.contains(&permission);
            self.checks.push(permission);
            if let Some(outcome) = self.outcomes.pop_front() {
                outcome?;
            }
            Ok(allowed)
        })
    }
    fn batch_check<'a>(&'a mut self, permissions: Vec<Permission>) -> Operation<'a, Vec<bool>> {
        Box::pin(async move {
            let mut results = Vec::with_capacity(permissions.len());
            for permission in permissions {
                results.push(self.check(permission).await?);
            }
            Ok(results)
        })
    }
    fn list_objects<'a>(
        &'a mut self,
        instance: &'a str,
        subject: &'a Subject,
        relation: &'a str,
        object_type: &'a str,
    ) -> Operation<'a, Vec<String>> {
        Box::pin(async move {
            if let Some(outcome) = self.outcomes.pop_front() {
                outcome?;
            }
            let prefix = format!("{object_type}:");
            Ok(self
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
        })
    }
}
