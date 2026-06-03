//! Core ACP (Agent Communication Protocol) type definitions.
//!
//! Provides the foundational wire types used for agent-to-agent and
//! agent-to-host message exchange:
//!
//! - [`AgentId`] — unique agent identity (UUID wrapper).
//! - [`CorrelationId`] — request–response correlation (UUID v7).
//! - [`MessageType`] — classification of an ACP message.
//! - [`AcpEnvelope`] — transport-level metadata (version, TTL, extras).
//! - [`AcpMessage<T>`] — generic message combining an envelope with a typed payload.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// AgentId
// ---------------------------------------------------------------------------

/// Unique identifier for an agent.
///
/// Wraps a [`Uuid`] (any version). Display and FromStr follow the standard
/// hyphenated UUID format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub Uuid);

impl From<Uuid> for AgentId {
    #[inline]
    fn from(u: Uuid) -> Self {
        Self(u)
    }
}

impl From<AgentId> for Uuid {
    #[inline]
    fn from(id: AgentId) -> Self {
        id.0
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::str::FromStr for AgentId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(Self)
    }
}

// ---------------------------------------------------------------------------
// CorrelationId
// ---------------------------------------------------------------------------

/// Correlation identifier for pairing requests with responses.
///
/// **Must** be a UUID v7 (time-ordered) to preserve causal ordering and
/// enable database-friendly indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    /// Generate a new UUID v7 correlation ID.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for CorrelationId {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for CorrelationId {
    #[inline]
    fn from(u: Uuid) -> Self {
        Self(u)
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// ---------------------------------------------------------------------------
// MessageType
// ---------------------------------------------------------------------------

/// Classification of an ACP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// A request expecting a response.
    Request,
    /// A successful response to a prior request.
    Response,
    /// An error response to a prior request.
    Error,
    /// An unsolicited event / notification.
    Event,
}

impl MessageType {
    /// Returns `true` if this variant is a kind of response (success or error).
    #[inline]
    pub const fn is_response(self) -> bool {
        matches!(self, Self::Response | Self::Error)
    }

    /// Returns `true` if this is a request (expects a response).
    #[inline]
    pub const fn is_request(self) -> bool {
        matches!(self, Self::Request)
    }
}

// ---------------------------------------------------------------------------
// AcpEnvelope
// ---------------------------------------------------------------------------

/// Transport-level envelope carrying protocol metadata.
///
/// Every ACP message bears an envelope that is independent of the payload
/// type, making it suitable for routing, rate-limiting, and observability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcpEnvelope {
    /// Semantic version of the ACP protocol used for this message
    /// (e.g. `"0.1.0"`).
    pub acp_version: String,

    /// Time-to-live in seconds. A router or agent SHOULD discard the message
    /// if its age exceeds this value.
    pub ttl_secs: u64,

    /// Source agent identifier.
    pub source: AgentId,

    /// Target agent identifier (or a well-known broadcast / router id).
    pub target: AgentId,

    /// Correlation ID linking this message to a conversation.
    pub correlation_id: CorrelationId,

    /// Message type classification.
    pub message_type: MessageType,

    /// Extensibility key-value bag. Reserved keys prefixed with `x_` are
    /// implementation-defined; all others are reserved for future ACP spec
    /// additions.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extras: HashMap<String, String>,
}

impl AcpEnvelope {
    /// Create a new envelope with the minimum required fields.
    #[inline]
    pub fn new(
        acp_version: String,
        ttl_secs: u64,
        source: AgentId,
        target: AgentId,
        message_type: MessageType,
    ) -> Self {
        Self {
            acp_version,
            ttl_secs,
            source,
            target,
            correlation_id: CorrelationId::new(),
            message_type,
            extras: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// AcpMessage<T>
// ---------------------------------------------------------------------------

/// A generic ACP message combining a typed payload with its envelope.
///
/// Use `AcpMessage<SomePayload>` to send strongly-typed messages over the
/// wire while preserving routing metadata in the envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AcpMessage<T> {
    /// Protocol envelope (routing & metadata).
    pub envelope: AcpEnvelope,
    /// The typed payload body.
    pub payload: T,
}

impl<T> AcpMessage<T> {
    /// Map the payload to a different type, preserving the envelope.
    #[inline]
    pub fn map<U, F>(self, f: F) -> AcpMessage<U>
    where
        F: FnOnce(T) -> U,
    {
        AcpMessage {
            envelope: self.envelope,
            payload: f(self.payload),
        }
    }

    /// Borrow the payload.
    #[inline]
    pub fn payload(&self) -> &T {
        &self.payload
    }

    /// Consume the message and return the payload.
    #[inline]
    pub fn into_payload(self) -> T {
        self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_roundtrip() {
        let original = AgentId(Uuid::now_v7());
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: AgentId = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }

    #[test]
    fn correlation_id_new_generates_v7() {
        let cid = CorrelationId::new();
        let uuid: Uuid = cid.0;
        // UUID version nibble at bits 12-15 of octet 6
        assert_eq!(uuid.get_version(), Some(uuid::Version::SortRand));
    }

    #[test]
    fn correlation_id_default_uses_new() {
        // Two successive defaults should differ
        assert_ne!(CorrelationId::default(), CorrelationId::default());
    }

    #[test]
    fn message_type_snake_case_roundtrip() {
        let cases = [
            (MessageType::Request, "\"request\""),
            (MessageType::Response, "\"response\""),
            (MessageType::Error, "\"error\""),
            (MessageType::Event, "\"event\""),
        ];
        for (variant, expected_json) in &cases {
            let json = serde_json::to_string(variant).unwrap();
            assert_eq!(json, *expected_json);
        }
    }

    #[test]
    fn message_type_is_response() {
        assert!(MessageType::Response.is_response());
        assert!(MessageType::Error.is_response());
        assert!(!MessageType::Request.is_response());
        assert!(!MessageType::Event.is_response());
    }

    #[test]
    fn envelope_serde_roundtrip() {
        let envelope = AcpEnvelope {
            acp_version: "0.1.0".into(),
            ttl_secs: 60,
            source: AgentId(Uuid::nil()),
            target: AgentId(Uuid::nil()),
            correlation_id: CorrelationId(Uuid::nil()),
            message_type: MessageType::Request,
            extras: [("x_route".into(), "direct".into())].into(),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        let deserialized: AcpEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope, deserialized);
    }

    #[test]
    fn envelope_extras_defaults_empty() {
        let json = r#"{
            "acp_version": "0.1.0",
            "ttl_secs": 30,
            "source": "00000000-0000-0000-0000-000000000000",
            "target": "00000000-0000-0000-0000-000000000000",
            "correlation_id": "00000000-0000-0000-0000-000000000000",
            "message_type": "event"
        }"#;
        let envelope: AcpEnvelope = serde_json::from_str(json).unwrap();
        assert!(envelope.extras.is_empty());
    }

    #[test]
    fn acp_message_map_transforms_payload() {
        let msg = AcpMessage {
            envelope: AcpEnvelope::new(
                "0.1.0".into(),
                10,
                AgentId(Uuid::nil()),
                AgentId(Uuid::nil()),
                MessageType::Request,
            ),
            payload: 42u32,
        };
        let mapped = msg.map(|n| n.to_string());
        assert_eq!(mapped.payload, "42");
        assert_eq!(mapped.envelope.ttl_secs, 10);
    }
}
