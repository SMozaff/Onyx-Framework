//! Value types for the Communication bounded context.
//!
//! Source: Part I §4.7 (Communication). `Conversation` and `Message` are
//! separate aggregate roots per §4.7.1 — Message is independently
//! versioned, not an entity nested inside Conversation, so a `MessageId`
//! stands alone rather than being scoped to a `ConversationId`.

use platform_kernel::ObjectId;
use serde::{Deserialize, Serialize};

/// The identity of a Conversation aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConversationId(pub ObjectId);

impl ConversationId {
    /// Generates a new random conversation identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// The identity of a Message aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MessageId(pub ObjectId);

impl MessageId {
    /// Generates a new random message identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// What kind of conversation this is. Per §4.7.2: "channels, mission
/// discussions, direct conversations, and threaded messaging."
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConversationType {
    /// A one-to-one conversation between exactly two members.
    Direct,
    /// A multi-member channel, optionally scoped to a Mission via
    /// `ContextLink` (Part I §4.7.6) rather than embedded ownership.
    Channel,
}

/// Text content of a message. A newtype rather than a bare `String` per
/// Part II Chapter 1 §1.6.3's "newtypes are mandatory for ... validated
/// text" rule — enforces the non-empty invariant at construction so it
/// cannot be represented in an invalid state once built.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageBody(String);

impl MessageBody {
    /// Maximum body length in UTF-8 bytes. Generous for prose, small
    /// enough that a message can never approach the transport layer's
    /// message-size limits (Part II §8's `max_message_size`).
    pub const MAX_LEN: usize = 8_000;

    /// Constructs a `MessageBody`, rejecting empty/whitespace-only or
    /// oversized text.
    pub fn new(text: impl Into<String>) -> Result<Self, String> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err("message body must not be empty".to_string());
        }
        if text.len() > Self::MAX_LEN {
            return Err(format!(
                "message body exceeds {} bytes (was {})",
                Self::MAX_LEN,
                text.len()
            ));
        }
        Ok(Self(text))
    }

    /// The message text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Why a message was redacted. Free text rather than an enum: the
/// meaningful categories (policy violation, sender request, legal hold)
/// are a Policy-context concern (Part I §4.18) this crate does not own.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionReason(pub String);

/// A single reaction (e.g. an emoji) applied by one actor. CRDT-eligible
/// per Part I §3.5 (OR-Set, "Reactions") — this crate defines the shape;
/// merge semantics belong to the `crdt`/`synchronization-domain` crates,
/// not here (Part II Chapter 1 §1.6: domain crates are merge-mechanism-free).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reaction {
    /// The reacting user.
    pub user_id: platform_kernel::UserId,
    /// The emoji or short code, e.g. `"thumbsup"`.
    pub emoji_code: ReactionCode,
}

/// A short, validated reaction identifier. Newtype rather than a bare
/// `String` for the same reason as [`MessageBody`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReactionCode([u8; 16]);

impl ReactionCode {
    /// Maximum length in bytes for a reaction code.
    pub const MAX_LEN: usize = 16;

    /// Constructs a `ReactionCode`, rejecting empty or oversized input.
    pub fn new(code: &str) -> Result<Self, String> {
        if code.is_empty() {
            return Err("reaction code must not be empty".to_string());
        }
        let bytes = code.as_bytes();
        if bytes.len() > Self::MAX_LEN {
            return Err(format!("reaction code exceeds {} bytes", Self::MAX_LEN));
        }
        let mut buf = [0u8; Self::MAX_LEN];
        buf[..bytes.len()].copy_from_slice(bytes);
        Ok(Self(buf))
    }

    /// The reaction code as a string slice.
    pub fn as_str(&self) -> &str {
        let len = self.0.iter().position(|&b| b == 0).unwrap_or(Self::MAX_LEN);
        std::str::from_utf8(&self.0[..len]).unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_id_random_generation_is_unique() {
        assert_ne!(ConversationId::new_random(), ConversationId::new_random());
    }

    #[test]
    fn message_id_random_generation_is_unique() {
        assert_ne!(MessageId::new_random(), MessageId::new_random());
    }

    #[test]
    fn message_body_rejects_empty() {
        assert!(MessageBody::new("").is_err());
        assert!(MessageBody::new("   ").is_err());
    }

    #[test]
    fn message_body_rejects_oversized() {
        let too_long = "a".repeat(MessageBody::MAX_LEN + 1);
        assert!(MessageBody::new(too_long).is_err());
    }

    #[test]
    fn message_body_accepts_normal_text() {
        let body = MessageBody::new("Standup notes for today").unwrap();
        assert_eq!(body.as_str(), "Standup notes for today");
    }

    #[test]
    fn reaction_code_round_trips() {
        let code = ReactionCode::new("thumbsup").unwrap();
        assert_eq!(code.as_str(), "thumbsup");
    }

    #[test]
    fn reaction_code_rejects_oversized() {
        assert!(ReactionCode::new(&"x".repeat(17)).is_err());
    }
}
