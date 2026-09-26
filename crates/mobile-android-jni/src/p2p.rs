//! P2P transport codec (MIGRATION_PLAN Phase 4.1) — framing / encryption /
//! handshake in pure Rust, exposed to Kotlin as JNI entry points on
//! `com.onyx.p2p.P2pCodec`.
//!
//! # Why this layer exists
//! DECISIONS P2P-1 rules that the P2P intelligence lives in Rust (transport
//! framing, encryption, handshake) while Kotlin owns the actual platform
//! drivers (`WifiP2pManager`, `BluetoothLeScanner`). The previous placeholder
//! C-ABI exports (`crates/transports/sync-transport-mobile`'s
//! `android_wifi_direct`/`android_ble`, re-exported through `mobile-core`)
//! were deliberately deleted in this phase — they allocated phantom handles
//! and carried no transport whatsoever, and P2P-1 forbids extending a fake
//! surface as if it did real work. This module is the real, replace-the-
//! stubs surface, following the same JNI discipline as the rest of this
//! crate: **no business logic**, just the codec primitives and the
//! handle-registry marshalling Kotlin needs to drive the platform sockets.
//!
//! # Cryptographic design (documented, not assumed secure)
//! * Handshake: ephemeral-ECDH over NIST P-256 (`ring::agreement`). Two
//!   messages: the initiator sends its 65-byte uncompressed public point,
//!   the responder answers with its own. **Unauthenticated by design** —
//!   each side brings no pre-shared identity, so active MITM is possible at
//!   this layer; the surrounding deployment authenticates the resulting
//!   channel at a higher layer (application/session auth) and this layer
//!   provides forward secrecy plus record confidentiality/integrity once the
//!   handshake is bound. That scope decision is explicit so nobody mistakes
//!   this for a CA-like trust anchor it is not.
//! * Key schedule: HKDF-SHA-256 (RFC 5869). The Extract step salts with
//!   `PROLOGUE ‖ client_pub ‖ server_pub`, binding the transcript into the
//!   keys (the ordering is fixed by role, so both sides derive identically).
//!   Expand then derives a 32-byte AEAD key and a 32-byte nonce key per
//!   direction (`i2r` for initiator→responder, `r2i` for the reverse);
//!   direction-keying prevents a sealed frame from one side being
//!   replayed/reflected into the other.
//! * Records: AES-256-GCM (`ring::aead`), header `4-byte big-endian
//!   plaintext length ‖ 1-byte version(0x01)`, then ciphertext+16-byte tag.
//!   The nonce is `HMAC-SHA256(nonce_key, counter.to_be_bytes())[..12]` with
//!   a per-direction counter starting at 0 — unique per (key, counter), and
//!   not trivially predictable from the ciphertext.
//! * **Stream-oriented**: the counters advance strictly and in order, so this
//!   codec requires a reliable, in-order channel (the platform drivers below
//!   this layer — a TCP socket on Wi-Fi Direct, a sequential GATT write/notify
//!   stream on BLE — provide exactly that). A failed `decode` desynchronizes
//!   the stream permanently; the caller must tear down and re-handshake.
//!
//! # JNI surface (class `com.onyx.p2p.P2pCodec`)
//! ```ignore
//! nativeSessionStart()                                  : Long      // initiator, returns handle (0 = failure)
//! nativeSessionClientMessage(handle: Long)              : ByteArray? // initiator's 65-byte public point
//! nativeSessionAccept(clientMessage: ByteArray)         : Long      // responder, returns handle (0 = failure)
//! nativeSessionServerMessage(handle: Long)              : ByteArray? // responder's 65-byte public point
//! nativeSessionComplete(handle: Long, serverMessage: ByteArray): Int // 0 ok, -1 failure
//! nativeSessionEncode(handle: Long, plaintext: ByteArray): ByteArray? // one frame
//! nativeSessionDecode(handle: Long, frame: ByteArray)   : ByteArray? // null on auth/malformed failure
//! nativeSessionClose(handle: Long)                      : Int      // 0 ok, -1 unknown handle
//! ```
//! The Rust codec itself (`start_handshake`, `accept_handshake`,
//! `complete_handshake`, `Session::encode`/`decode`) is fully unit-tested
//! on the host below; the JNI wrappers are deliberately thin (marshalling +
//! an opaque handle registry, the one bit of "state" the JNI contract
//! permits), matching `lib.rs`'s existing no-business-logic wrappers.
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use jni::errors::{Error as JniError, LogErrorAndDefault};
use jni::objects::{JByteArray, JClass};
use jni::sys::{jbyteArray, jint, jlong};
use jni::EnvUnowned;

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey, ECDH_P256};
use ring::error::Unspecified;
use ring::hkdf::{Prk, Salt, HKDF_SHA256};
use ring::hmac;
use ring::rand::SystemRandom;

/// Wire version stamped into every frame header.
pub const P2P_VERSION: u8 = 0x01;
/// Uncompressed P-256 point: `0x04 ‖ X(32) ‖ Y(32)`.
pub const HANDSHAKE_PUBKEY_LEN: usize = 65;
/// Four-byte length prefix + one-byte version.
pub const HEADER_LEN: usize = 5;
/// AES-256-GCM tag length.
pub const TAG_LEN: usize = 16;
const KEY_LEN: usize = 32;
/// Domain separation / transcript binding constant. Ordering of the two
/// public points below this prefix is fixed by role (`client ‖ server`), so
/// initiator and responder derive identical keys without exchanging order.
const PROLOGUE: &[u8] = b"onyx-p2p-v1";

/// Direction label for initiator→responder records.
const INFO_I2R: &[u8] = b"i2r";
/// Direction label for responder→initiator records.
const INFO_R2I: &[u8] = b"r2i";
const INFO_ENC: &[u8] = b"onyx-p2p-enc";
const INFO_NONCE: &[u8] = b"onyx-p2p-nonce";

/// Errors surfaced by the codec. Kept intentionally small: every variant
/// maps cleanly to the `Int`/`null` JNI sentinels the Kotlin side consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// `EphemeralPrivateKey::generate` / `compute_public_key` failed.
    KeyGeneration,
    /// HKDF or AEAD key construction failed.
    KeyDerivation,
    /// ECDH agreement with the peer's public point failed.
    Handshake,
    /// A handshake API was called on a session in the wrong role/state.
    InvalidState,
    /// `encode`/`decode` called before the handshake completed.
    NotReady,
    /// Frame size/version/length mismatch.
    Malformed,
    /// AEAD authentication failed (tamper, wrong key, or desync).
    AuthFailed,
    /// Plaintext too large for the 32-bit length prefix.
    TooLarge,
    /// A JNI call referenced a handle that is not in the registry.
    UnknownHandle,
}

impl fmt::Display for CodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CodecError {}

impl From<Unspecified> for CodecError {
    fn from(_: Unspecified) -> Self {
        CodecError::KeyDerivation
    }
}

fn pubkey_of(private_key: &EphemeralPrivateKey) -> Result<[u8; HANDSHAKE_PUBKEY_LEN], CodecError> {
    let bytes = private_key
        .compute_public_key()
        .map_err(|_| CodecError::KeyGeneration)?;
    let point: [u8; HANDSHAKE_PUBKEY_LEN] = bytes
        .as_ref()
        .try_into()
        .map_err(|_| CodecError::KeyGeneration)?;
    Ok(point)
}

/// ECDH shared secret (32 bytes for P-256), consuming `private_key`.
fn agree(
    private_key: EphemeralPrivateKey,
    peer_point: &[u8; HANDSHAKE_PUBKEY_LEN],
) -> Result<[u8; KEY_LEN], CodecError> {
    let peer = UnparsedPublicKey::new(&ECDH_P256, peer_point.as_slice());
    let shared = agreement::agree_ephemeral(private_key, &peer, |secret| secret.to_vec())
        .map_err(|_| CodecError::Handshake)?;
    let shared: [u8; KEY_LEN] = shared.try_into().map_err(|_| CodecError::Handshake)?;
    Ok(shared)
}

/// Expand one 32-byte key from the handshake PRK. `info_parts` must live
/// only for the duration of the call (covered by the `&[&[u8]]` borrow).
fn hkdf_expand(prk: &Prk, info_parts: &[&[u8]]) -> Result<[u8; KEY_LEN], CodecError> {
    // `HKDF_SHA256` is a `Copy` static; `Prk::expand` takes the `KeyType`
    // (here: `Algorithm`) by value.
    let okm = prk.expand(info_parts, HKDF_SHA256)?;
    let mut out = [0u8; KEY_LEN];
    okm.fill(&mut out)?;
    Ok(out)
}

struct DirectionKeys {
    enc: [u8; KEY_LEN],
    nonce_key: [u8; KEY_LEN],
}

/// Derive the fixed-by-role send/recv key pair. `client_pub` is the
/// initiator's point, `server_pub` the responder's — callers supply them in
/// that order regardless of which role they are, so both sides agree.
fn derive_keys(
    client_pub: &[u8; HANDSHAKE_PUBKEY_LEN],
    server_pub: &[u8; HANDSHAKE_PUBKEY_LEN],
    shared: &[u8; KEY_LEN],
    role: Role,
) -> Result<(DirectionKeys, DirectionKeys), CodecError> {
    let mut salt = Vec::with_capacity(PROLOGUE.len() + 2 * HANDSHAKE_PUBKEY_LEN);
    salt.extend_from_slice(PROLOGUE);
    salt.extend_from_slice(client_pub);
    salt.extend_from_slice(server_pub);
    let prk = Salt::new(HKDF_SHA256, &salt).extract(shared);

    let (send_dir, recv_dir) = match role {
        Role::Initiator => (INFO_I2R, INFO_R2I),
        Role::Responder => (INFO_R2I, INFO_I2R),
    };
    let send = DirectionKeys {
        enc: hkdf_expand(&prk, &[INFO_ENC, send_dir])?,
        nonce_key: hkdf_expand(&prk, &[INFO_NONCE, send_dir])?,
    };
    let recv = DirectionKeys {
        enc: hkdf_expand(&prk, &[INFO_ENC, recv_dir])?,
        nonce_key: hkdf_expand(&prk, &[INFO_NONCE, recv_dir])?,
    };
    Ok((send, recv))
}

/// Derive `nonce[..12]` for a (key, counter) pair: unique because the key is
/// per-direction and the counter strictly increments.
fn nonce_for(nonce_key: &[u8; KEY_LEN], counter: u64) -> [u8; 12] {
    let key = hmac::Key::new(hmac::HMAC_SHA256, nonce_key);
    let tag = hmac::sign(&key, &counter.to_be_bytes());
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&tag.as_ref()[..12]);
    nonce
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Initiator,
    Responder,
}

/// A live (or handshake-pending) P2P transport session.
pub struct Session {
    role: Role,
    own_pub: [u8; HANDSHAKE_PUBKEY_LEN],
    peer_pub: [u8; HANDSHAKE_PUBKEY_LEN],
    /// `Some` only while the initiator is mid-handshake (moved out by
    /// `complete_handshake`). The responder's keypair is consumed by
    /// `accept_handshake` itself.
    ephemeral: Option<EphemeralPrivateKey>,
    keys: Option<(DirectionKeys, DirectionKeys)>,
    send_counter: u64,
    recv_counter: u64,
}

impl Session {
    /// First handshake message on the initiator side: creates a pending
    /// session and returns it together with the 65-byte public point Kotlin
    /// must deliver to the responder.
    pub fn start_handshake() -> Result<(Session, [u8; HANDSHAKE_PUBKEY_LEN]), CodecError> {
        let rng = SystemRandom::new();
        let private_key = EphemeralPrivateKey::generate(&ECDH_P256, &rng)?;
        let own_pub = pubkey_of(&private_key)?;
        let session = Session {
            role: Role::Initiator,
            own_pub,
            peer_pub: [0u8; HANDSHAKE_PUBKEY_LEN],
            ephemeral: Some(private_key),
            keys: None,
            send_counter: 0,
            recv_counter: 0,
        };
        Ok((session, own_pub))
    }

    /// Second handshake message on the responder side: consumes the
    /// initiator's public point, derives session keys immediately, and
    /// returns the responder session plus its own public point (which the
    /// initiator needs for `complete_handshake`).
    pub fn accept_handshake(
        client_point: &[u8; HANDSHAKE_PUBKEY_LEN],
    ) -> Result<(Session, [u8; HANDSHAKE_PUBKEY_LEN]), CodecError> {
        let rng = SystemRandom::new();
        let private_key = EphemeralPrivateKey::generate(&ECDH_P256, &rng)?;
        let own_pub = pubkey_of(&private_key)?;
        let shared = agree(private_key, client_point)?;
        let (send, recv) = derive_keys(client_point, &own_pub, &shared, Role::Responder)?;
        let session = Session {
            role: Role::Responder,
            own_pub,
            peer_pub: *client_point,
            ephemeral: None,
            keys: Some((send, recv)),
            send_counter: 0,
            recv_counter: 0,
        };
        Ok((session, own_pub))
    }

    /// Initiator finishes the handshake by binding the responder's public
    /// point into the key schedule. Takes `self` and returns it in the ready
    /// state (consumes the pending ephemeral keypair).
    pub fn complete_handshake(
        mut session: Session,
        server_point: &[u8; HANDSHAKE_PUBKEY_LEN],
    ) -> Result<Session, CodecError> {
        if session.role != Role::Initiator || session.ephemeral.is_none() {
            return Err(CodecError::InvalidState);
        }
        let private_key = session.ephemeral.take().expect("checked above");
        let shared = agree(private_key, server_point)?;
        session.peer_pub = *server_point;
        let (send, recv) = derive_keys(&session.own_pub, server_point, &shared, Role::Initiator)?;
        session.keys = Some((send, recv));
        Ok(session)
    }

    fn ready_keys(&mut self) -> Result<(&mut DirectionKeys, &mut DirectionKeys), CodecError> {
        self.keys
            .as_mut()
            .map(|(send, recv)| (send, recv))
            .ok_or(CodecError::NotReady)
    }

    /// Seal one record: `[len(4) ‖ version(1) ‖ ciphertext ‖ tag(16)]`.
    pub fn encode(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, CodecError> {
        let plaintext_len = u32::try_from(plaintext.len()).map_err(|_| CodecError::TooLarge)?;
        let counter = self.send_counter;
        let (send, _) = self.ready_keys()?;
        let nonce = nonce_for(&send.nonce_key, counter);
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &send.enc)?);

        let mut frame = Vec::with_capacity(HEADER_LEN + plaintext.len() + TAG_LEN);
        frame.extend_from_slice(&plaintext_len.to_be_bytes());
        frame.push(P2P_VERSION);
        frame.extend_from_slice(plaintext);

        let tag = key
            .seal_in_place_separate_tag(
                Nonce::assume_unique_for_key(nonce),
                Aad::empty(),
                &mut frame[HEADER_LEN..],
            )
            .map_err(|_| CodecError::KeyDerivation)?;
        frame.extend_from_slice(tag.as_ref());
        self.send_counter += 1;
        Ok(frame)
    }

    /// Open one record. Returns the plaintext, or `AuthFailed`/`Malformed`
    /// on any verification failure. Note: a failed open does **not** advance
    /// the receive counter, but the stream is desynchronized regardless —
    /// the caller must tear down and re-handshake (documented at module top).
    pub fn decode(&mut self, frame: &[u8]) -> Result<Vec<u8>, CodecError> {
        if frame.len() < HEADER_LEN + TAG_LEN {
            return Err(CodecError::Malformed);
        }
        if frame[4] != P2P_VERSION {
            return Err(CodecError::Malformed);
        }
        let declared =
            u32::from_be_bytes(frame[..4].try_into().expect("len>=HEADER_LEN checked")) as usize;
        if frame.len() != HEADER_LEN + declared + TAG_LEN {
            return Err(CodecError::Malformed);
        }

        let counter = self.recv_counter;
        let (_, recv) = self.ready_keys()?;
        let nonce = nonce_for(&recv.nonce_key, counter);
        let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &recv.enc)?);

        let mut body = frame[HEADER_LEN..].to_vec();
        let plaintext = key
            .open_in_place(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut body)
            .map_err(|_| CodecError::AuthFailed)?;
        self.recv_counter += 1;
        Ok(plaintext.to_vec())
    }

    pub fn role(&self) -> Role {
        self.role
    }

    pub fn is_ready(&self) -> bool {
        self.keys.is_some()
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Key material and the pending ephemeral are deliberately excluded.
        f.debug_struct("Session")
            .field("role", &self.role)
            .field("ready", &self.keys.is_some())
            .field("send_counter", &self.send_counter)
            .field("recv_counter", &self.recv_counter)
            .finish()
    }
}

// ============================================================================
// JNI surface — `com.onyx.p2p.P2pCodec`
// ============================================================================

type Handle = u64;
type Registry = Mutex<HashMap<Handle, Session>>;

static REGISTRY: OnceLock<Registry> = OnceLock::new();
static NEXT_HANDLE: AtomicU64 = AtomicU64::new(1);

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn mint_handle() -> Handle {
    NEXT_HANDLE.fetch_add(1, Ordering::Relaxed)
}

/// Reads a `jbyteArray` into an owned `Vec<u8>` (jbyte is signed; the bit
/// pattern is preserved verbatim, which is the correct wire behavior).
fn jbyte_array_to_vec(env: &jni::Env<'_>, array: &JByteArray) -> Result<Vec<u8>, JniError> {
    let len = array.len(env)?;
    let mut raw = vec![0i8; len];
    array.get_region(env, 0, &mut raw)?;
    Ok(raw.iter().map(|&b| b as u8).collect())
}

/// Pushes an owned byte buffer out as a fresh `jbyteArray`, returning its
/// raw handle for the JNI return convention used across this crate.
fn vec_to_jbyte_array(env: &mut jni::Env<'_>, bytes: &[u8]) -> Result<jbyteArray, JniError> {
    Ok(env.byte_array_from_slice(bytes)?.as_raw())
}

/// `nativeSessionStart` — create an initiator session; the caller then pulls
/// `nativeSessionClientMessage` to get the first handshake message.
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionStart<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> jlong {
    env.with_env(|_env| -> Result<jlong, JniError> {
        match Session::start_handshake() {
            Ok((session, _)) => {
                let handle = mint_handle();
                registry().lock().unwrap().insert(handle, session);
                Ok(handle as jlong)
            }
            Err(_) => Ok(0),
        }
    })
    .resolve::<LogErrorAndDefault>()
}

/// `nativeSessionClientMessage(handle)` — initiator's 65-byte public point.
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionClientMessage<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jbyteArray {
    env.with_env(|env| -> Result<jbyteArray, JniError> {
        let point = match registry().lock().unwrap().get(&(handle as Handle)) {
            Some(s) => s.own_pub,
            None => return Ok(std::ptr::null_mut()),
        };
        vec_to_jbyte_array(env, &point)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `nativeSessionAccept(clientMessage)` — responder side; stores the ready
/// responder session and returns its handle (`0` on a bad client point).
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionAccept<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    client_message: JByteArray<'local>,
) -> jlong {
    env.with_env(|env| -> Result<jlong, JniError> {
        let bytes = jbyte_array_to_vec(env, &client_message)?;
        let client_point: [u8; HANDSHAKE_PUBKEY_LEN] = match bytes.as_slice().try_into() {
            Ok(p) => p,
            Err(_) => return Ok(0),
        };
        match Session::accept_handshake(&client_point) {
            Ok((session, _)) => {
                let handle = mint_handle();
                registry().lock().unwrap().insert(handle, session);
                Ok(handle as jlong)
            }
            Err(_) => Ok(0),
        }
    })
    .resolve::<LogErrorAndDefault>()
}

/// `nativeSessionServerMessage(handle)` — responder's 65-byte public point.
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionServerMessage<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jbyteArray {
    env.with_env(|env| -> Result<jbyteArray, JniError> {
        let point = match registry().lock().unwrap().get(&(handle as Handle)) {
            Some(s) => s.own_pub,
            None => return Ok(std::ptr::null_mut()),
        };
        vec_to_jbyte_array(env, &point)
    })
    .resolve::<LogErrorAndDefault>()
}

/// `nativeSessionComplete(handle, serverMessage)` — initiator finishes the
/// handshake. Returns `0` on success, `-1` on any failure (invalid state or
/// an unusable server point).
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionComplete<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    server_message: JByteArray<'local>,
) -> jint {
    env.with_env(|env| -> Result<jint, JniError> {
        let bytes = jbyte_array_to_vec(env, &server_message)?;
        let server_point: [u8; HANDSHAKE_PUBKEY_LEN] = match bytes.as_slice().try_into() {
            Ok(p) => p,
            Err(_) => return Ok(-1),
        };
        let registry = registry();
        let mut guard = registry.lock().unwrap();
        let Some(session) = guard.remove(&(handle as Handle)) else {
            return Ok(-1);
        };
        match Session::complete_handshake(session, &server_point) {
            Ok(completed) => {
                guard.insert(handle as Handle, completed);
                Ok(0)
            }
            Err(_) => Ok(-1),
        }
    })
    .resolve::<LogErrorAndDefault>()
}

/// `nativeSessionEncode(handle, plaintext)` — one sealed frame.
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionEncode<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    plaintext: JByteArray<'local>,
) -> jbyteArray {
    env.with_env(|env| -> Result<jbyteArray, JniError> {
        let plaintext = jbyte_array_to_vec(env, &plaintext)?;
        let registry = registry();
        let mut guard = registry.lock().unwrap();
        let encoded = guard
            .get_mut(&(handle as Handle))
            .map(|s| s.encode(&plaintext))
            .transpose()
            .ok()
            .flatten();
        drop(guard);
        match encoded {
            Some(frame) => vec_to_jbyte_array(env, &frame),
            None => Ok(std::ptr::null_mut()),
        }
    })
    .resolve::<LogErrorAndDefault>()
}

/// `nativeSessionDecode(handle, frame)` — opens one frame; `null` on any
/// auth/malformed failure (the stream is then desynchronized, see module doc).
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionDecode<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    frame: JByteArray<'local>,
) -> jbyteArray {
    env.with_env(|env| -> Result<jbyteArray, JniError> {
        let frame = jbyte_array_to_vec(env, &frame)?;
        let registry = registry();
        let mut guard = registry.lock().unwrap();
        let decoded = guard
            .get_mut(&(handle as Handle))
            .map(|s| s.decode(&frame))
            .transpose()
            .ok()
            .flatten();
        drop(guard);
        match decoded {
            Some(plaintext) => vec_to_jbyte_array(env, &plaintext),
            None => Ok(std::ptr::null_mut()),
        }
    })
    .resolve::<LogErrorAndDefault>()
}

/// `nativeSessionClose(handle)` — drops the session; `0` ok, `-1` unknown.
#[no_mangle]
pub extern "system" fn Java_com_onyx_p2p_P2pCodec_nativeSessionClose<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jint {
    env.with_env(|_env| -> Result<jint, JniError> {
        let removed = registry().lock().unwrap().remove(&(handle as Handle));
        Ok(if removed.is_some() { 0 } else { -1 })
    })
    .resolve::<LogErrorAndDefault>()
}

// ============================================================================
// Host-runnable unit tests — exercise the codec, not JNI marshalling.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Full two-message handshake + strict in-order carriage of several
    /// messages in both directions (each direction has its own counter).
    #[test]
    fn initiator_responder_round_trip() {
        let (initiator, client_msg) = Session::start_handshake().unwrap();
        let (mut responder, server_msg) = Session::accept_handshake(&client_msg).unwrap();
        let mut initiator = Session::complete_handshake(initiator, &server_msg).unwrap();

        assert!(initiator.is_ready());
        assert!(responder.is_ready());
        assert_eq!(initiator.role(), Role::Initiator);
        assert_eq!(responder.role(), Role::Responder);

        let messages: [&[u8]; 4] = [
            b"mission/3c2f1a4e/state",
            b"env-raid",
            b"second-message",
            b"third",
        ];
        // Forward (initiator -> responder), in strict order.
        let frames: Vec<Vec<u8>> = messages
            .iter()
            .map(|m| initiator.encode(m).unwrap())
            .collect();
        for (frame, expected) in frames.iter().zip(messages.iter()) {
            assert_eq!(responder.decode(frame).unwrap().as_slice(), *expected);
        }
        // Reverse (responder -> initiator).
        let reply = responder.encode(b"ack").unwrap();
        assert_eq!(initiator.decode(&reply).unwrap().as_slice(), b"ack");
    }

    #[test]
    fn tampered_frame_is_rejected() {
        let (initiator, client_msg) = Session::start_handshake().unwrap();
        let (mut responder, server_msg) = Session::accept_handshake(&client_msg).unwrap();
        let mut initiator = Session::complete_handshake(initiator, &server_msg).unwrap();

        let mut frame = initiator.encode(b"classified").unwrap();
        let last = frame.len() - 1;
        frame[last] ^= 0x01;
        assert_eq!(responder.decode(&frame), Err(CodecError::AuthFailed));

        // Length header tamper must fail the structural check (or auth).
        let mut frame = initiator.encode(b"classified").unwrap();
        frame[2] ^= 0xff;
        assert!(matches!(
            responder.decode(&frame),
            Err(CodecError::Malformed) | Err(CodecError::AuthFailed)
        ));
    }

    #[test]
    fn truncated_or_oversized_frames_are_malformed() {
        let (initiator, client_msg) = Session::start_handshake().unwrap();
        let (mut responder, server_msg) = Session::accept_handshake(&client_msg).unwrap();
        let mut initiator = Session::complete_handshake(initiator, &server_msg).unwrap();

        let frame = initiator.encode(b"short").unwrap();
        // Too short to even hold header+tag.
        assert_eq!(
            responder.decode(&frame[..HEADER_LEN + TAG_LEN - 1]),
            Err(CodecError::Malformed)
        );
        // Extraneous trailing bytes (length prefix disagrees with body).
        let mut junk = frame.clone();
        junk.push(0x00);
        assert_eq!(responder.decode(&junk), Err(CodecError::Malformed));
        // Wrong version byte.
        let mut wrong_version = frame.clone();
        wrong_version[4] = 0x02;
        assert_eq!(responder.decode(&wrong_version), Err(CodecError::Malformed));
    }

    #[test]
    fn wrong_peer_cannot_decrypt() {
        // Two independent handshakes; a frame from one session must not open
        // under the other.
        let (a, ca) = Session::start_handshake().unwrap();
        let (b, cb) = Session::start_handshake().unwrap();
        let (mut rb, sb) = Session::accept_handshake(&ca).unwrap();
        let (mut ra, sa) = Session::accept_handshake(&cb).unwrap();
        let mut a = Session::complete_handshake(a, &sb).unwrap();
        let mut b = Session::complete_handshake(b, &sa).unwrap();

        let frame_a = a.encode(b"from-session-a").unwrap();
        assert_eq!(ra.decode(&frame_a), Err(CodecError::AuthFailed));
        let frame_b = b.encode(b"from-session-b").unwrap();
        assert_eq!(rb.decode(&frame_b), Err(CodecError::AuthFailed));

        // And valid cross-pairs still open (sanity — not a bogus failure mode).
        assert_eq!(rb.decode(&frame_a).unwrap().as_slice(), b"from-session-a");
        assert_eq!(ra.decode(&frame_b).unwrap().as_slice(), b"from-session-b");
    }

    #[test]
    fn handshake_state_is_enforced() {
        let (initiator, client_msg) = Session::start_handshake().unwrap();

        // Not ready: encode/decode must fail before completion.
        let mut pending = initiator;
        assert_eq!(pending.encode(b"x").unwrap_err(), CodecError::NotReady);

        // Responder cannot be completed as an initiator.
        let (responder, server_msg) = Session::accept_handshake(&client_msg).unwrap();
        assert_eq!(
            Session::complete_handshake(responder, &server_msg).unwrap_err(),
            CodecError::InvalidState
        );

        // Completing twice must fail too (ephemeral already consumed).
        let completed =
            Session::complete_handshake(pending_after(pending, &server_msg), &server_msg)
                .unwrap_err();
        assert_eq!(completed, CodecError::InvalidState);

        // Garbage client point on accept is a handshake failure -> 0 at JNI,
        // Handshake here.
        assert_eq!(
            Session::accept_handshake(&[0x42; HANDSHAKE_PUBKEY_LEN]).unwrap_err(),
            CodecError::Handshake
        );
    }

    fn pending_after(pending: Session, server_msg: &[u8; HANDSHAKE_PUBKEY_LEN]) -> Session {
        // complete once, then drop the result; the function name documents
        // that the returned session has no pending ephemeral left.
        Session::complete_handshake(pending, server_msg).unwrap()
    }

    #[test]
    fn direction_keys_differ_per_role() {
        let (initiator, client_msg) = Session::start_handshake().unwrap();
        let (mut responder, server_msg) = Session::accept_handshake(&client_msg).unwrap();
        let mut initiator = Session::complete_handshake(initiator, &server_msg).unwrap();

        // A frame from the initiator must not open on the initiator itself
        // (it uses a different direction key for recv vs send).
        let frame = initiator.encode(b"self-test").unwrap();
        assert_eq!(initiator.decode(&frame), Err(CodecError::AuthFailed));
        // And it does open on the responder.
        assert_eq!(responder.decode(&frame).unwrap().as_slice(), b"self-test");
    }
}
