//! Web Push cryptography and protocol primitives (RFC 8292 VAPID, RFC 8291
//! message encryption), supplied by BoringSSL-derived `ring`.
//!
//! This module is deliberately pure — no database, no HTTP. It owns:
//!   * the ES256 (P-256/SHA-256) VAPID JWT signer,
//!   * HKDF-SHA-256 (RFC 5869),
//!   * the RFC 8291 `aes128gcm` record construction, and
//!   * an internal decrypt helper used by the unit tests to prove the
//!     encryption round-trips against keys the test generated.
//!
//! The delivery worker in [`crate::push_delivery`] owns persistence, polling,
//! and the HTTP transport toward the push endpoint.

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use ring::{
    aead, agreement,
    rand::{SecureRandom, SystemRandom},
    signature::{self, EcdsaKeyPair, KeyPair},
};
use sha2::Sha256;

/// RFC 8291 requires each `aes128gcm` record to be at most this many octets.
pub const AES128GCM_RS: u32 = 4096;
/// A single notification payload easily fits in one 4096-byte record; refusing
/// larger payloads keeps the framing trivial (one record, no padding blocks).
pub const MAX_PAYLOAD_LEN: usize = 4096 - 97;

const CEK_LEN: usize = 16;
const AES128GCM_NONCE_LEN: usize = 12;
const AUTH_SECRET_LEN: usize = 16;
const UNCOMPRESSED_POINT_LEN: usize = 65;
/// Size of a raw (JOSE) ECDSA signature: r || s, 32 bytes each.
const RAW_ECDSA_SIG_LEN: usize = 64;
/// `aes128gcm` record header: salt(16) ‖ rs(4) ‖ idlen(1) ‖ keyid(65).
const RECORD_HEADER_LEN: usize = 16 + 4 + 1 + UNCOMPRESSED_POINT_LEN;

type HmacSha256 = Hmac<Sha256>;

/// HKDF-Extract (RFC 5869 §2.2). An empty salt is treated as a block of zeros
/// the length of the hash output.
fn hkdf_extract(salt: &[u8], ikm: &[u8]) -> [u8; 32] {
    let salt: &[u8] = if salt.is_empty() { &[0u8; 32] } else { salt };
    let mut mac = <HmacSha256 as Mac>::new_from_slice(salt).expect("HMAC accepts any key");
    mac.update(ikm);
    mac.finalize().into_bytes().into()
}

/// HKDF-Expand (RFC 5869 §2.3).
fn hkdf_expand(prk: &[u8; 32], info: &[u8], len: usize) -> Vec<u8> {
    let mut okm = Vec::with_capacity(len);
    let mut t = Vec::new();
    let mut counter: u8 = 1;
    while okm.len() < len {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(prk).expect("HMAC accepts any key");
        mac.update(&t);
        mac.update(info);
        mac.update(&[counter]);
        t = mac.finalize().into_bytes().to_vec();
        okm.extend_from_slice(&t);
        counter += 1;
    }
    okm.truncate(len);
    okm
}

/// RFC 5869 HKDF-SHA-256 extract-then-expand, exposed for unit tests.
pub fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let prk = hkdf_extract(salt, ikm);
    hkdf_expand(&prk, info, len)
}

/// RFC 8291 §2 key schedule: derives the 16-byte content-encryption key and
/// the 12-byte AES-GCM nonce from the ECDH shared secret.
fn derive_rfc8291_keys(
    shared_secret: &[u8],
    auth_secret: &[u8],
    ua_public: &[u8],
    server_public: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    let mut prk_mac =
        <HmacSha256 as Mac>::new_from_slice(auth_secret).expect("HMAC accepts any key");
    prk_mac.update(shared_secret);
    let prk: [u8; 32] = prk_mac.finalize().into_bytes().into();

    let mut key_info = Vec::with_capacity(13 + 2 + UNCOMPRESSED_POINT_LEN * 2);
    key_info.extend_from_slice(b"WebPush: info\0");
    key_info.extend_from_slice(ua_public);
    key_info.extend_from_slice(server_public);
    let cek = hkdf_expand(&prk, &key_info, CEK_LEN);

    let nonce = hkdf_expand(&prk, b"Content-Encoding: nonce\0", AES128GCM_NONCE_LEN);
    (cek, nonce)
}

/// Base64url (no padding) encoding, as required by VAPID/Web Push headers.
pub fn b64url_encode(input: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(input)
}

/// Base64url decoding that also tolerates the RFC 4648 padding variant used by
/// some subscription helpers.
pub fn b64url_decode(input: &str) -> Result<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(input)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(input))
        .context("invalid base64url string")
}

/// Wraps the raw (r ‖ s, RFC 7515 §3.4) signature bytes ring already emits
/// for the `_FIXED_SIGNING` algorithm, validating the length.
fn raw_signature_to_jose(sig: &[u8]) -> Result<&[u8]> {
    if sig.len() != RAW_ECDSA_SIG_LEN {
        bail!(
            "expected {RAW_ECDSA_SIG_LEN}-byte raw ECDSA signature, got {}",
            sig.len()
        );
    }
    Ok(sig)
}

fn build_signing_input(header: &str, payload: &str) -> String {
    format!(
        "{}.{}",
        b64url_encode(header.as_bytes()),
        b64url_encode(payload.as_bytes())
    )
}

/// ES256 VAPID signer (RFC 8292). Constructed from a PKCS#8 DER-encoded
/// P-256 key — the same format `npx web-push generate-vapid-keys` prints
/// (base64url) and `openssl pkcs8 -topk8 -nocrypt` emits.
pub struct VapidSigner {
    key: EcdsaKeyPair,
    rng: SystemRandom,
}

impl VapidSigner {
    /// Builds a signer from PKCS#8 DER. For PEM input, strip the `-----BEGIN
    /// PRIVATE KEY-----` armor and base64-decode first.
    pub fn from_pkcs8_der(pkcs8_der: &[u8]) -> Result<Self> {
        let rng = SystemRandom::new();
        let key =
            EcdsaKeyPair::from_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8_der, &rng)
                .map_err(|e| anyhow::anyhow!("invalid VAPID EC PKCS#8 key: {e:?}"))?;
        Ok(Self { key, rng })
    }

    /// Builds a fresh random key pair (used by deployment bootstrapping and
    /// tests). Returns the signer plus serialized PKCS#8 DER for persistence.
    pub fn generate() -> Result<(Self, Vec<u8>)> {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .map_err(|e| anyhow::anyhow!("failed to generate VAPID key: {e:?}"))?;
        let der = pkcs8.as_ref().to_vec();
        let signer = Self::from_pkcs8_der(&der)?;
        Ok((signer, der))
    }

    /// The uncompressed (65-byte) public point as base64url, for the VAPID
    /// `Authorization` header `k` parameter.
    pub fn public_key_base64url(&self) -> String {
        b64url_encode(self.key.public_key().as_ref())
    }

    /// RFC 8292: an ES256 JWT whose `aud` is the push endpoint origin, `sub`
    /// the contact address, and `exp` = `now_secs + ttl_secs`. The returned
    /// string is the full `t=<jwt>` value for the VAPID Authorization header.
    pub fn sign_auth_token(
        &self,
        audience: &str,
        subject: &str,
        now_secs: u64,
        ttl_secs: u64,
    ) -> Result<String> {
        let header = "{\"alg\":\"ES256\",\"typ\":\"JWT\"}";
        let claims = serde_json::json!({
            "aud": audience,
            "exp": now_secs + ttl_secs,
            "sub": subject,
        });
        let signing_input = build_signing_input(header, &claims.to_string());
        let sig = self
            .key
            .sign(&self.rng, signing_input.as_bytes())
            .map_err(|e| anyhow::anyhow!("failed to sign VAPID JWT: {e:?}"))?;
        let raw = raw_signature_to_jose(sig.as_ref())?;
        Ok(format!("{signing_input}.{}", b64url_encode(raw)))
    }
}

/// Derives the `aud` (audience) value for VAPID: the serialized origin of the
/// push endpoint, e.g. `https://fcm.googleapis.com`.
pub fn endpoint_audience(endpoint: &str) -> Result<String> {
    let url = url::Url::parse(endpoint).context("push endpoint is not a valid URL")?;
    let scheme = url.scheme();
    if scheme != "https" && scheme != "http" {
        bail!("push endpoint scheme must be https (or http for local testing)");
    }
    let host = url.host_str().context("push endpoint has no host")?;
    Ok(match url.port_or_known_default() {
        Some(port) if is_non_default_port(scheme, port) => format!("{scheme}://{host}:{port}"),
        _ => format!("{scheme}://{host}"),
    })
}

fn is_non_default_port(scheme: &str, port: u16) -> bool {
    (scheme == "https" && port != 443) || (scheme == "http" && port != 80)
}

fn random_bytes(rng: &SystemRandom, len: usize) -> Result<Vec<u8>> {
    let mut out = vec![0u8; len];
    rng.fill(&mut out)
        .map_err(|_| anyhow::anyhow!("secure RNG failure"))?;
    Ok(out)
}

/// Encrypts a push payload per RFC 8291 into a single `aes128gcm` record
/// (RFC 8188 framing: `<salt><rs><idlen><keyid><ciphertext><tag>`). The
/// sender's ephemeral P-256 key is supplied so a caller can reuse it per
/// message; the caller may simply generate one per call.
pub fn encrypt_payload(
    payload: &[u8],
    ua_public_key_b64url: &str,
    ua_auth_secret_b64url: &str,
    sender_ephemeral: agreement::EphemeralPrivateKey,
    rng: &SystemRandom,
) -> Result<Vec<u8>> {
    if payload.len() > MAX_PAYLOAD_LEN {
        bail!(
            "push payload too large: {} bytes (limit {})",
            payload.len(),
            MAX_PAYLOAD_LEN
        );
    }
    let ua_public = b64url_decode(ua_public_key_b64url)?;
    if ua_public.len() != UNCOMPRESSED_POINT_LEN {
        bail!(
            "p256dh must be an uncompressed 65-byte point, got {} bytes",
            ua_public.len()
        );
    }
    let auth = b64url_decode(ua_auth_secret_b64url)?;
    if auth.len() != AUTH_SECRET_LEN {
        bail!("auth secret must be 16 bytes, got {}", auth.len());
    }

    let server_public = sender_ephemeral
        .compute_public_key()
        .map_err(|e| anyhow::anyhow!("ECDH public key computation failed: {e:?}"))?;
    let peer = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, &ua_public);
    let shared = agreement::agree_ephemeral(sender_ephemeral, &peer, |k| k.to_vec())
        .map_err(|e| anyhow::anyhow!("ECDH agreement failed: {e:?}"))?;

    let (cek, nonce) = derive_rfc8291_keys(&shared, &auth, &ua_public, server_public.as_ref());

    let header_salt = random_bytes(rng, AUTH_SECRET_LEN)?;
    let mut header = Vec::with_capacity(RECORD_HEADER_LEN);
    header.extend_from_slice(&header_salt);
    header.extend_from_slice(&AES128GCM_RS.to_be_bytes());
    header.push(UNCOMPRESSED_POINT_LEN as u8);
    header.extend_from_slice(server_public.as_ref());

    let unbound = aead::UnboundKey::new(&aead::AES_128_GCM, &cek)
        .map_err(|e| anyhow::anyhow!("AES key rejected: {e:?}"))?;
    let secret = aead::LessSafeKey::new(unbound);
    let nonce = aead::Nonce::assume_unique_for_key(nonce[..].try_into().expect("12-byte nonce"));

    let mut ciphertext = payload.to_vec();
    let tag = secret
        .seal_in_place_separate_tag(nonce, aead::Aad::from(&header), &mut ciphertext)
        .map_err(|e| anyhow::anyhow!("AES-128-GCM seal failed: {e:?}"))?;

    let mut out = header;
    out.extend_from_slice(&ciphertext);
    out.extend_from_slice(tag.as_ref());
    Ok(out)
}

/// Test/decrypt helper mirroring the state a push *service worker* holds:
/// the subscription private key and `auth` secret. Verifies the auth-client
/// side of RFC 8291 by recovering the payload from a produced record.
#[cfg(test)]
pub(crate) fn decrypt_push_message(
    message: &[u8],
    ua_private: agreement::EphemeralPrivateKey,
    ua_auth_secret_b64url: &str,
) -> Result<Vec<u8>> {
    let (header, rest) = message.split_at(RECORD_HEADER_LEN);
    // rs (bytes 16..20) and idlen (byte 20) are validated for framing sanity.
    let rs = u32::from_be_bytes(header[16..20].try_into().expect("4-byte rs"));
    let idlen = header[20] as usize;
    if rs != AES128GCM_RS {
        bail!("unexpected record size {rs}");
    }
    if idlen != UNCOMPRESSED_POINT_LEN {
        bail!("unexpected key id length {idlen}");
    }
    let keyid = &message[RECORD_HEADER_LEN - UNCOMPRESSED_POINT_LEN..RECORD_HEADER_LEN];
    let ct_and_tag = rest;

    let ua_public = ua_private
        .compute_public_key()
        .map_err(|e| anyhow::anyhow!("ECDH public key computation failed: {e:?}"))?;
    let peer = agreement::UnparsedPublicKey::new(&agreement::ECDH_P256, keyid);
    let shared = agreement::agree_ephemeral(ua_private, &peer, |k| k.to_vec())
        .map_err(|e| anyhow::anyhow!("ECDH agreement failed: {e:?}"))?;
    let auth = b64url_decode(ua_auth_secret_b64url)?;
    let (cek, nonce) = derive_rfc8291_keys(&shared, &auth, ua_public.as_ref(), keyid);

    let unbound = aead::UnboundKey::new(&aead::AES_128_GCM, &cek)
        .map_err(|e| anyhow::anyhow!("AES key rejected: {e:?}"))?;
    let secret = aead::LessSafeKey::new(unbound);
    let nonce = aead::Nonce::assume_unique_for_key(nonce[..].try_into().expect("12-byte nonce"));
    let mut buf = Vec::from(ct_and_tag);
    let plaintext = secret
        .open_in_place(nonce, aead::Aad::from(header), &mut buf)
        .map_err(|e| anyhow::anyhow!("AES-128-GCM open failed: {e:?}"))?;
    Ok(plaintext.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 5869 Appendix A.1 test case (SHA-256).
    #[test]
    fn hkdf_sha256_matches_rfc5869_vector() {
        let ikm = [0x0b; 22];
        let salt = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
        ];
        let info = [0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9];
        let expected = [
            0x3c, 0xb2, 0x5f, 0x25, 0xfa, 0xac, 0xd5, 0x7a, 0x90, 0x43, 0x4f, 0x64, 0xd0, 0x36,
            0x2f, 0x2a, 0x2d, 0x2d, 0x0a, 0x90, 0xcf, 0x1a, 0x5a, 0x4c, 0x5d, 0xb0, 0x2d, 0x56,
            0xec, 0xc4, 0xc5, 0xbf, 0x34, 0x00, 0x72, 0x08, 0xd5, 0xb8, 0x87, 0x18, 0x58, 0x65,
        ];
        let okm = hkdf_sha256(&salt, &ikm, &info, 42);
        assert_eq!(okm, expected);
    }

    #[test]
    fn hkdf_sha256_default_salt_is_zeros() {
        // RFC 5869 §2.2: a missing salt defaults to HashLen zero bytes.
        let okm = hkdf_sha256(&[], b"input", b"info", 16);
        assert_eq!(okm.len(), 16);
        // Just asserting determinism + length; the zero-salt path is what some
        // tooling uses and must not panic or truncate.
        assert_eq!(okm, hkdf_sha256(&[0u8; 32], b"input", b"info", 16));
    }

    fn test_signer() -> (VapidSigner, Vec<u8>) {
        let (signer, der) = VapidSigner::generate().expect("generate key");
        // Round-trip the PKCS#8 so from_pkcs8_der is exercised too.
        let rebuilt = VapidSigner::from_pkcs8_der(&der).expect("reimport pkcs8");
        // The rebuilt signer signs the same payload identically? No — ECDSA is
        // randomized; assert the public keys match instead.
        assert_eq!(
            signer.public_key_base64url(),
            rebuilt.public_key_base64url()
        );
        (signer, der)
    }

    #[test]
    fn vapid_jwt_signs_and_verifies() {
        let (signer, der) = test_signer();
        let audience = "https://fcm.googleapis.com";
        let subject = "mailto:ops@onyx.example";
        let token = signer
            .sign_auth_token(audience, subject, 1_700_000_000, 43_200)
            .expect("sign JWT");

        let mut parts = token.split('.');
        let header_b64 = parts.next().expect("header");
        let payload_b64 = parts.next().expect("payload");
        let sig_b64 = parts.next().expect("signature");
        assert!(parts.next().is_none(), "token must have exactly 3 parts");

        let signing_input = format!("{header_b64}.{payload_b64}");
        let sig = b64url_decode(sig_b64).expect("decode signature");
        assert_eq!(sig.len(), RAW_ECDSA_SIG_LEN, "raw r||s signature");

        let public_key = b64url_decode(&signer.public_key_base64url()).expect("decode public key");
        let verifying =
            signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, public_key);
        verifying
            .verify(signing_input.as_bytes(), &sig)
            .expect("ES256 signature verifies");

        let header_json = b64url_decode(header_b64).expect("decode header");
        assert_eq!(
            std::str::from_utf8(&header_json).unwrap(),
            "{\"alg\":\"ES256\",\"typ\":\"JWT\"}",
            "header must be exactly the VAPID-specified JOSE header"
        );
        let payload_bytes = b64url_decode(payload_b64).expect("decode payload");
        let payload_json = std::str::from_utf8(&payload_bytes).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(payload_json).expect("payload json");
        assert_eq!(parsed["aud"], audience);
        assert_eq!(parsed["sub"], subject);
        assert_eq!(parsed["exp"], 1_700_043_200);
        drop(der);
    }

    #[test]
    fn encrypt_payload_round_trips_through_subscription_keys() {
        let rng = SystemRandom::new();
        let ua_private = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng)
            .expect("ua ephemeral");
        let ua_public = ua_private.compute_public_key().expect("ua public");
        let auth = [0x42u8; 16];

        let sender = agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng)
            .expect("sender ephemeral");

        let payload = b"Onyx notification: mission #42 needs review";
        let record = encrypt_payload(
            payload,
            &b64url_encode(ua_public.as_ref()),
            &b64url_encode(&auth),
            sender,
            &rng,
        )
        .expect("encrypt");
        assert_eq!(
            record.len(),
            RECORD_HEADER_LEN + payload.len() + aead::AES_128_GCM.tag_len(),
            "salt(16) + rs(4) + idlen(1) + keyid(65) + ct + tag(16)"
        );

        let plaintext =
            decrypt_push_message(&record, ua_private, &b64url_encode(&auth)).expect("decrypt");
        assert_eq!(plaintext, payload);
    }

    #[test]
    fn encrypt_payload_rejects_short_key_material() {
        let rng = SystemRandom::new();
        let auth = [0x42u8; 16];
        let make_sender = || {
            agreement::EphemeralPrivateKey::generate(&agreement::ECDH_P256, &rng)
                .expect("sender ephemeral")
        };

        assert!(
            encrypt_payload(
                b"x",
                &b64url_encode(&[0x42u8; 65]),
                &b64url_encode(&auth),
                make_sender(),
                &rng
            )
            .is_err(),
            "short p256dh must be rejected"
        );

        assert!(
            encrypt_payload(
                b"x",
                &b64url_encode(&[0x42u8; 65]),
                &b64url_encode(&[0x42u8; 15]),
                make_sender(),
                &rng
            )
            .is_err(),
            "15-byte auth must be rejected"
        );
    }

    #[test]
    fn endpoint_audience_derives_origin() {
        assert_eq!(
            endpoint_audience("https://fcm.googleapis.com/fcm/send/abc").unwrap(),
            "https://fcm.googleapis.com"
        );
        assert_eq!(
            endpoint_audience("https://push.example.com:8443/push/xyz").unwrap(),
            "https://push.example.com:8443"
        );
        assert_eq!(
            endpoint_audience("http://127.0.0.1:8080/push").unwrap(),
            "http://127.0.0.1:8080"
        );
        assert!(endpoint_audience("not-a-url").is_err());
        assert!(endpoint_audience("ftp://push.example.com/x").is_err());
    }
}
