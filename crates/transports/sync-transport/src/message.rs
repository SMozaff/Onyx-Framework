//! `SyncMessage`: the canonical wire format exchanged between replicas.
//!
//! Frozen contract per Team Prompt 4 §3.2, with the binary layout resolved
//! per the rulings in DECISIONS.md, and built against `platform_kernel`'s
//! real types (ReplicaId, OrganizationId, Timestamp, SchemaVersion) per the
//! real-workspace integration ruling. All newtype access uses `.0`, since
//! none of these real types expose `to_bytes()`/`from_bytes()`.
//!
//! - `version_len`: u8 length prefix + UTF-8 bytes.
//! - `target_replica`: 1-byte presence flag + 16 bytes (always present on
//!   the wire; bytes are zeroed and ignored when the flag is 0).
//! - `signature_len`: u16 big-endian length prefix + bytes.

use platform_kernel::{OrganizationId, ReplicaId, SchemaVersion, Timestamp};
use serde::{Deserialize, Serialize};

/// The canonical message exchanged between replicas.
/// This is the only format the sync engine (Team 3) understands.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncMessage {
    pub message_id: MessageId,  // Unique per message
    pub version: SchemaVersion, // "1.0"
    pub message_type: SyncMessageType,
    pub organization_id: OrganizationId, // MUST match sender and receiver
    pub sender_replica: ReplicaId,
    pub target_replica: Option<ReplicaId>, // None = broadcast to all in org
    pub payload: Vec<u8>,                  // Serialized operation batches, state deltas, etc.
    pub timestamp: Timestamp,
    pub signature: Option<Vec<u8>>, // Reserved for future end-to-end auth
}

/// Binary serialization format (frozen byte-level layout, big-endian
/// throughout). See DECISIONS.md for the resolution of ambiguities in
/// the literal prompt text.
///
/// ```text
/// [1 byte]   message_type      (enum as u8)
/// [16 bytes] message_id        (raw bytes)
/// [1 byte]   version_len       (0-255)
/// [N bytes]  version           (UTF-8, N = version_len)
/// [16 bytes] organization_id   (ObjectId.0)
/// [16 bytes] sender_replica    (ReplicaId.0)
/// [1 byte]   target_present    (1 = Some, 0 = None)
/// [16 bytes] target_replica    (always present on the wire; zeroed if target_present == 0)
/// [4 bytes]  payload_len       (u32, big-endian)
/// [N bytes]  payload           (N = payload_len)
/// [8 bytes]  timestamp         (u64 nanoseconds, big-endian; Timestamp.0)
/// [1 byte]   signature_present (1 = Some, 0 = None)
/// [2 bytes]  signature_len     (u16, big-endian; 0 if signature_present == 0)
/// [N bytes]  signature         (N = signature_len)
/// ```
impl SyncMessage {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64 + self.payload.len());

        // message_type
        buf.push(self.message_type.as_u8());

        // message_id
        buf.extend_from_slice(&self.message_id.0);

        // version
        let version_bytes = self.version.0.as_bytes();
        debug_assert!(
            version_bytes.len() <= u8::MAX as usize,
            "version string exceeds 255 bytes"
        );
        buf.push(version_bytes.len() as u8);
        buf.extend_from_slice(version_bytes);

        // organization_id (ObjectId newtype, real platform_kernel type)
        buf.extend_from_slice(&self.organization_id.0);

        // sender_replica
        buf.extend_from_slice(&self.sender_replica.0);

        // target_replica
        match self.target_replica {
            Some(replica) => {
                buf.push(1);
                buf.extend_from_slice(&replica.0);
            }
            None => {
                buf.push(0);
                buf.extend_from_slice(&[0u8; 16]);
            }
        }

        // payload
        let payload_len = self.payload.len() as u32;
        buf.extend_from_slice(&payload_len.to_be_bytes());
        buf.extend_from_slice(&self.payload);

        // timestamp
        buf.extend_from_slice(&self.timestamp.0.to_be_bytes());

        // signature
        match &self.signature {
            Some(sig) => {
                debug_assert!(
                    sig.len() <= u16::MAX as usize,
                    "signature exceeds 65535 bytes"
                );
                buf.push(1);
                buf.extend_from_slice(&(sig.len() as u16).to_be_bytes());
                buf.extend_from_slice(sig);
            }
            None => {
                buf.push(0);
                buf.extend_from_slice(&0u16.to_be_bytes());
            }
        }

        buf
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self, crate::placeholder_types::SerializationError> {
        use crate::placeholder_types::SerializationError as E;

        let mut cursor = 0usize;

        macro_rules! need {
            ($n:expr) => {
                if bytes.len() < cursor + $n {
                    return Err(E::BufferTooShort {
                        expected: cursor + $n,
                        actual: bytes.len(),
                    });
                }
            };
        }

        // message_type
        need!(1);
        let message_type =
            SyncMessageType::from_u8(bytes[cursor]).ok_or(E::InvalidMessageType(bytes[cursor]))?;
        cursor += 1;

        // message_id
        need!(16);
        let mut id_bytes = [0u8; 16];
        id_bytes.copy_from_slice(&bytes[cursor..cursor + 16]);
        let message_id = MessageId(id_bytes);
        cursor += 16;

        // version
        need!(1);
        let version_len = bytes[cursor] as usize;
        cursor += 1;
        need!(version_len);
        let version_str = std::str::from_utf8(&bytes[cursor..cursor + version_len])
            .map_err(|_| E::InvalidVersionUtf8)?
            .to_string();
        cursor += version_len;

        // organization_id
        need!(16);
        let mut org_bytes = [0u8; 16];
        org_bytes.copy_from_slice(&bytes[cursor..cursor + 16]);
        // `OrganizationId` is a type ALIAS for `ObjectId` (`pub type
        // OrganizationId = ObjectId;`), not a distinct newtype — so it
        // must be constructed via `ObjectId(...)`, not `OrganizationId(...)`.
        // Caught by `cargo check` (E0423), not assumed.
        let organization_id: OrganizationId = platform_kernel::ObjectId(org_bytes);
        cursor += 16;

        // sender_replica
        need!(16);
        let mut sender_bytes = [0u8; 16];
        sender_bytes.copy_from_slice(&bytes[cursor..cursor + 16]);
        let sender_replica = ReplicaId(sender_bytes);
        cursor += 16;

        // target_replica
        need!(1);
        let target_present = bytes[cursor] != 0;
        cursor += 1;
        need!(16);
        let mut target_bytes = [0u8; 16];
        target_bytes.copy_from_slice(&bytes[cursor..cursor + 16]);
        cursor += 16;
        let target_replica = if target_present {
            Some(ReplicaId(target_bytes))
        } else {
            None
        };

        // payload
        need!(4);
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(&bytes[cursor..cursor + 4]);
        let payload_len = u32::from_be_bytes(len_bytes);
        cursor += 4;
        let remaining_before_payload = bytes.len() - cursor;
        if (payload_len as usize) > remaining_before_payload {
            return Err(E::PayloadLengthMismatch {
                declared: payload_len,
                remaining: remaining_before_payload,
            });
        }
        let payload = bytes[cursor..cursor + payload_len as usize].to_vec();
        cursor += payload_len as usize;

        // timestamp
        need!(8);
        let mut ts_bytes = [0u8; 8];
        ts_bytes.copy_from_slice(&bytes[cursor..cursor + 8]);
        let timestamp = Timestamp::from_nanos(u64::from_be_bytes(ts_bytes));
        cursor += 8;

        // signature
        need!(1);
        let signature_present = bytes[cursor] != 0;
        cursor += 1;
        need!(2);
        let mut sig_len_bytes = [0u8; 2];
        sig_len_bytes.copy_from_slice(&bytes[cursor..cursor + 2]);
        let signature_len = u16::from_be_bytes(sig_len_bytes);
        cursor += 2;
        let remaining_before_sig = bytes.len() - cursor;
        if (signature_len as usize) > remaining_before_sig {
            return Err(E::SignatureLengthMismatch {
                declared: signature_len,
                remaining: remaining_before_sig,
            });
        }
        let sig_bytes = bytes[cursor..cursor + signature_len as usize].to_vec();
        cursor += signature_len as usize;
        let signature = if signature_present {
            Some(sig_bytes)
        } else {
            None
        };

        if cursor != bytes.len() {
            return Err(E::TrailingBytes);
        }

        Ok(SyncMessage {
            message_id,
            version: SchemaVersion(version_str),
            message_type,
            organization_id,
            sender_replica,
            target_replica,
            payload,
            timestamp,
            signature,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMessageType {
    DiscoveryRequest,
    DiscoveryResponse,
    OfferOperations,      // "I have these ops for you"
    OperationBatch,       // The actual operations
    ConflictNotification, // "A conflict was detected"
    Ack,                  // Acknowledgment
}

impl SyncMessageType {
    fn as_u8(&self) -> u8 {
        match self {
            SyncMessageType::DiscoveryRequest => 0,
            SyncMessageType::DiscoveryResponse => 1,
            SyncMessageType::OfferOperations => 2,
            SyncMessageType::OperationBatch => 3,
            SyncMessageType::ConflictNotification => 4,
            SyncMessageType::Ack => 5,
        }
    }

    fn from_u8(b: u8) -> Option<Self> {
        match b {
            0 => Some(SyncMessageType::DiscoveryRequest),
            1 => Some(SyncMessageType::DiscoveryResponse),
            2 => Some(SyncMessageType::OfferOperations),
            3 => Some(SyncMessageType::OperationBatch),
            4 => Some(SyncMessageType::ConflictNotification),
            5 => Some(SyncMessageType::Ack),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageId(pub [u8; 16]);

impl MessageId {
    pub fn new() -> Self {
        Self(*uuid::Uuid::new_v4().as_bytes())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_message(target: Option<ReplicaId>, signature: Option<Vec<u8>>) -> SyncMessage {
        SyncMessage {
            message_id: MessageId::new(),
            version: SchemaVersion::new("1.0"),
            message_type: SyncMessageType::OperationBatch,
            organization_id: OrganizationId::new_random(),
            sender_replica: ReplicaId::new_random(),
            target_replica: target,
            payload: vec![1, 2, 3, 4, 5],
            timestamp: Timestamp::now(),
            signature,
        }
    }

    #[test]
    fn roundtrip_broadcast_no_signature() {
        let msg = sample_message(None, None);
        let bytes = msg.serialize();
        let decoded = SyncMessage::deserialize(&bytes).unwrap();
        assert_eq!(decoded.message_id, msg.message_id);
        assert_eq!(decoded.organization_id, msg.organization_id);
        assert_eq!(decoded.sender_replica, msg.sender_replica);
        assert_eq!(decoded.target_replica, None);
        assert_eq!(decoded.payload, msg.payload);
        assert_eq!(decoded.signature, None);
    }

    #[test]
    fn roundtrip_targeted_with_signature() {
        let target = ReplicaId::new_random();
        let msg = sample_message(Some(target), Some(vec![9, 9, 9]));
        let bytes = msg.serialize();
        let decoded = SyncMessage::deserialize(&bytes).unwrap();
        assert_eq!(decoded.target_replica, Some(target));
        assert_eq!(decoded.signature, Some(vec![9, 9, 9]));
    }

    #[test]
    fn roundtrip_empty_payload() {
        let mut msg = sample_message(None, None);
        msg.payload = vec![];
        let bytes = msg.serialize();
        let decoded = SyncMessage::deserialize(&bytes).unwrap();
        assert_eq!(decoded.payload, Vec::<u8>::new());
    }

    #[test]
    fn deserialize_rejects_truncated_buffer() {
        let msg = sample_message(None, None);
        let mut bytes = msg.serialize();
        bytes.truncate(bytes.len() - 5);
        let err = SyncMessage::deserialize(&bytes).unwrap_err();
        matches!(
            err,
            crate::placeholder_types::SerializationError::BufferTooShort { .. }
        );
    }

    #[test]
    fn deserialize_rejects_bad_message_type() {
        let msg = sample_message(None, None);
        let mut bytes = msg.serialize();
        bytes[0] = 255;
        let err = SyncMessage::deserialize(&bytes).unwrap_err();
        assert_eq!(
            err,
            crate::placeholder_types::SerializationError::InvalidMessageType(255)
        );
    }

    #[test]
    fn deserialize_rejects_trailing_bytes() {
        let msg = sample_message(None, None);
        let mut bytes = msg.serialize();
        bytes.push(0xFF);
        let err = SyncMessage::deserialize(&bytes).unwrap_err();
        assert_eq!(
            err,
            crate::placeholder_types::SerializationError::TrailingBytes
        );
    }

    #[test]
    fn deserialize_rejects_bogus_payload_length() {
        let msg = sample_message(None, None);
        let mut bytes = msg.serialize();
        // Offset math: message_type[1] + message_id[16] + version_len[1] +
        // version[3, "1.0"] + organization_id[16] + sender_replica[16] +
        // target_present[1] + target_replica[16] = 70.
        let payload_len_offset = 1 + 16 + 1 + 3 + 16 + 16 + 1 + 16;
        bytes[payload_len_offset..payload_len_offset + 4]
            .copy_from_slice(&(u32::MAX).to_be_bytes());
        let err = SyncMessage::deserialize(&bytes).unwrap_err();
        matches!(
            err,
            crate::placeholder_types::SerializationError::PayloadLengthMismatch { .. }
        );
    }

    #[test]
    fn all_message_type_variants_roundtrip() {
        let variants = [
            SyncMessageType::DiscoveryRequest,
            SyncMessageType::DiscoveryResponse,
            SyncMessageType::OfferOperations,
            SyncMessageType::OperationBatch,
            SyncMessageType::ConflictNotification,
            SyncMessageType::Ack,
        ];
        for v in variants {
            let mut msg = sample_message(None, None);
            msg.message_type = v.clone();
            let bytes = msg.serialize();
            let decoded = SyncMessage::deserialize(&bytes).unwrap();
            assert_eq!(decoded.message_type, v);
        }
    }
}
