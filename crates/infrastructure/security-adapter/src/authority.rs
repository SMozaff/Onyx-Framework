use std::{collections::HashSet, sync::Arc};

use async_trait::async_trait;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use platform_kernel::{AuthorityProof, ObjectId, ProofType, Timestamp};
use security_application::{
    AuthorityVerificationError, AuthorityVerificationReceipt, AuthorityVerificationRequest,
    AuthorityVerifier, RotatingSecret,
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Ed25519AuthorityVerifier {
    verifying_keys: Arc<Vec<VerifyingKey>>,
    revoked_fingerprints: Arc<RwLock<HashSet<String>>>,
}

impl Ed25519AuthorityVerifier {
    pub fn new(verifying_keys: Vec<VerifyingKey>) -> Result<Self, AuthorityVerificationError> {
        if verifying_keys.is_empty() {
            return Err(AuthorityVerificationError::Infrastructure(
                "at least one Ed25519 verifying key is required".to_string(),
            ));
        }
        Ok(Self {
            verifying_keys: Arc::new(verifying_keys),
            revoked_fingerprints: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    pub fn from_signing_seeds(seeds: &[Vec<u8>]) -> Result<Self, AuthorityVerificationError> {
        let mut keys = Vec::with_capacity(seeds.len());
        for seed in seeds {
            let bytes: [u8; 32] = seed.as_slice().try_into().map_err(|_| {
                AuthorityVerificationError::Infrastructure(
                    "Ed25519 signing seed must contain exactly 32 bytes".to_string(),
                )
            })?;
            keys.push(SigningKey::from_bytes(&bytes).verifying_key());
        }
        Self::new(keys)
    }

    pub async fn revoke(&self, proof: &AuthorityProof) {
        self.revoked_fingerprints
            .write()
            .await
            .insert(proof_fingerprint(proof));
    }

    pub fn sign_proof(
        signing_key: &SigningKey,
        request: &AuthorityVerificationRequest,
    ) -> AuthorityProof {
        let mut proof = request.proof.clone();
        proof.signature = None;
        let unsigned_request = AuthorityVerificationRequest {
            proof: proof.clone(),
            actor: request.actor.clone(),
            target: request.target.clone(),
            command_type: request.command_type.clone(),
            trusted_now: request.trusted_now,
        };
        let signature = signing_key.sign(&canonical_authority_bytes(&unsigned_request));
        proof.signature = Some(signature.to_bytes().to_vec());
        proof
    }
}

#[async_trait]
impl AuthorityVerifier for Ed25519AuthorityVerifier {
    async fn verify(
        &self,
        request: &AuthorityVerificationRequest,
    ) -> Result<
        (
            platform_kernel::VerifiedAuthority,
            AuthorityVerificationReceipt,
        ),
        AuthorityVerificationError,
    > {
        if self
            .revoked_fingerprints
            .read()
            .await
            .contains(&proof_fingerprint(&request.proof))
        {
            return Err(AuthorityVerificationError::Revoked);
        }

        if request.proof.issued_at > request.trusted_now {
            return Err(AuthorityVerificationError::NotYetValid);
        }
        if request.proof.expires_at <= request.trusted_now {
            return Err(AuthorityVerificationError::Expired);
        }
        if request.proof.scope.organization_id != request.actor.organization_id
            || request.proof.scope.organization_id != request.target.organization_id
        {
            return Err(AuthorityVerificationError::OrganizationMismatch);
        }
        if request.proof.scope.object_type != "*"
            && request.proof.scope.object_type != request.target.r#type
        {
            return Err(AuthorityVerificationError::ObjectScopeMismatch);
        }
        if let Some(object_id) = request.proof.scope.object_id {
            if object_id != request.target.id {
                return Err(AuthorityVerificationError::ObjectScopeMismatch);
            }
        }
        if !request
            .proof
            .scope
            .command_types
            .iter()
            .any(|allowed| allowed == "*" || allowed == &request.command_type)
        {
            return Err(AuthorityVerificationError::CommandNotAuthorized(
                request.command_type.clone(),
            ));
        }

        let signature_bytes = request
            .proof
            .signature
            .as_deref()
            .ok_or(AuthorityVerificationError::MissingSignature)?;
        let signature = Signature::from_slice(signature_bytes)
            .map_err(|_| AuthorityVerificationError::InvalidSignature)?;
        let mut unsigned = request.clone();
        unsigned.proof.signature = None;
        let message = canonical_authority_bytes(&unsigned);
        let valid = self
            .verifying_keys
            .iter()
            .any(|key| key.verify(&message, &signature).is_ok());
        if !valid {
            return Err(AuthorityVerificationError::InvalidSignature);
        }

        Ok((
            platform_kernel::VerifiedAuthority,
            AuthorityVerificationReceipt {
                verified_at: request.trusted_now,
                organization_id: request.proof.scope.organization_id.to_string(),
                object_type: request.target.r#type.clone(),
                command_type: request.command_type.clone(),
                delegation_depth: request.proof.scope.delegation_depth,
            },
        ))
    }
}

fn write_len_prefixed(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    output.extend_from_slice(bytes);
}

fn write_object_id(output: &mut Vec<u8>, id: ObjectId) {
    output.extend_from_slice(&id.0);
}

fn canonical_authority_bytes(request: &AuthorityVerificationRequest) -> Vec<u8> {
    let mut output = b"ONYX-AUTHORITY-PROOF-V1\0".to_vec();
    output.push(match &request.proof.proof_type {
        ProofType::Jwt => 1,
        ProofType::Paseto => 2,
        ProofType::DelegationChain => 3,
    });
    write_object_id(&mut output, request.proof.scope.organization_id);
    write_len_prefixed(&mut output, request.proof.scope.object_type.as_bytes());
    match request.proof.scope.object_id {
        Some(id) => {
            output.push(1);
            write_object_id(&mut output, id);
        }
        None => output.push(0),
    }
    let mut commands = request.proof.scope.command_types.clone();
    commands.sort();
    output.extend_from_slice(&(commands.len() as u32).to_be_bytes());
    for command in commands {
        write_len_prefixed(&mut output, command.as_bytes());
    }
    output.extend_from_slice(&request.proof.scope.delegation_depth.to_be_bytes());
    output.extend_from_slice(&request.proof.issued_at.0.to_be_bytes());
    output.extend_from_slice(&request.proof.expires_at.0.to_be_bytes());
    write_object_id(&mut output, request.actor.user_id);
    write_object_id(&mut output, request.actor.device_id);
    write_object_id(&mut output, request.actor.organization_id);
    write_object_id(&mut output, request.target.id);
    write_len_prefixed(&mut output, request.target.r#type.as_bytes());
    write_object_id(&mut output, request.target.organization_id);
    write_len_prefixed(&mut output, request.command_type.as_bytes());
    output
}

fn proof_fingerprint(proof: &AuthorityProof) -> String {
    let mut hasher = Sha256::new();
    hasher.update(proof.scope.organization_id.0);
    hasher.update(proof.scope.object_type.as_bytes());
    if let Some(object_id) = proof.scope.object_id {
        hasher.update(object_id.0);
    }
    for command in &proof.scope.command_types {
        hasher.update(command.as_bytes());
    }
    hasher.update(proof.issued_at.0.to_be_bytes());
    hasher.update(proof.expires_at.0.to_be_bytes());
    if let Some(signature) = &proof.signature {
        hasher.update(signature);
    }
    hex::encode(hasher.finalize())
}

/// Minimal EdDSA JWT codec used by the Team 6 web authentication surface.
/// It intentionally supports only `alg=EdDSA` and no algorithm negotiation.
#[derive(Clone)]
pub struct Ed25519JwtCodec {
    signing_key: Arc<SigningKey>,
    verification_keys: Arc<Vec<(VerifyingKey, Option<Timestamp>)>>,
}

impl Ed25519JwtCodec {
    pub fn from_signing_seeds(
        current_seed: &[u8],
        previous_seeds: &[Vec<u8>],
    ) -> Result<Self, AuthorityVerificationError> {
        let current: [u8; 32] = current_seed.try_into().map_err(|_| {
            AuthorityVerificationError::Infrastructure(
                "Ed25519 signing seed must contain exactly 32 bytes".to_string(),
            )
        })?;
        let signing_key = SigningKey::from_bytes(&current);
        let mut verification_keys = vec![(signing_key.verifying_key(), None)];
        for seed in previous_seeds {
            let previous: [u8; 32] = seed.as_slice().try_into().map_err(|_| {
                AuthorityVerificationError::Infrastructure(
                    "previous Ed25519 seed must contain exactly 32 bytes".to_string(),
                )
            })?;
            verification_keys.push((SigningKey::from_bytes(&previous).verifying_key(), None));
        }
        Ok(Self {
            signing_key: Arc::new(signing_key),
            verification_keys: Arc::new(verification_keys),
        })
    }

    pub fn from_rotating_secret(
        secret: &RotatingSecret,
    ) -> Result<Self, AuthorityVerificationError> {
        let current: [u8; 32] = secret.current.value.as_slice().try_into().map_err(|_| {
            AuthorityVerificationError::Infrastructure(
                "Ed25519 signing seed must contain exactly 32 bytes".to_string(),
            )
        })?;
        let signing_key = SigningKey::from_bytes(&current);
        let mut verification_keys = vec![(signing_key.verifying_key(), None)];
        if let Some(previous) = &secret.previous {
            let previous_seed: [u8; 32] = previous.value.as_slice().try_into().map_err(|_| {
                AuthorityVerificationError::Infrastructure(
                    "previous Ed25519 seed must contain exactly 32 bytes".to_string(),
                )
            })?;
            verification_keys.push((
                SigningKey::from_bytes(&previous_seed).verifying_key(),
                previous.valid_until,
            ));
        }
        Ok(Self {
            signing_key: Arc::new(signing_key),
            verification_keys: Arc::new(verification_keys),
        })
    }

    pub fn encode<T: Serialize>(&self, claims: &T) -> Result<String, AuthorityVerificationError> {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"EdDSA","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(claims)
                .map_err(|error| AuthorityVerificationError::Infrastructure(error.to_string()))?,
        );
        let signing_input = format!("{header}.{payload}");
        let signature = self.signing_key.sign(signing_input.as_bytes());
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        ))
    }

    pub fn decode<T: DeserializeOwned>(
        &self,
        token: &str,
    ) -> Result<T, AuthorityVerificationError> {
        self.decode_at(token, Timestamp::now())
    }

    pub fn decode_at<T: DeserializeOwned>(
        &self,
        token: &str,
        now: Timestamp,
    ) -> Result<T, AuthorityVerificationError> {
        let mut parts = token.split('.');
        let (Some(header), Some(payload), Some(signature), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(AuthorityVerificationError::InvalidSignature);
        };
        let header_bytes = URL_SAFE_NO_PAD
            .decode(header)
            .map_err(|_| AuthorityVerificationError::InvalidSignature)?;
        let header_value: serde_json::Value = serde_json::from_slice(&header_bytes)
            .map_err(|_| AuthorityVerificationError::InvalidSignature)?;
        if header_value.get("alg").and_then(serde_json::Value::as_str) != Some("EdDSA") {
            return Err(AuthorityVerificationError::InvalidSignature);
        }
        let signature_bytes = URL_SAFE_NO_PAD
            .decode(signature)
            .map_err(|_| AuthorityVerificationError::InvalidSignature)?;
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|_| AuthorityVerificationError::InvalidSignature)?;
        let signing_input = format!("{header}.{payload}");
        if !self
            .verification_keys
            .iter()
            .filter(|(_, valid_until)| valid_until.map(|expiry| expiry > now).unwrap_or(true))
            .any(|(key, _)| key.verify(signing_input.as_bytes(), &signature).is_ok())
        {
            return Err(AuthorityVerificationError::InvalidSignature);
        }
        let payload_bytes = URL_SAFE_NO_PAD
            .decode(payload)
            .map_err(|_| AuthorityVerificationError::InvalidSignature)?;
        serde_json::from_slice(&payload_bytes)
            .map_err(|error| AuthorityVerificationError::Infrastructure(error.to_string()))
    }
}
