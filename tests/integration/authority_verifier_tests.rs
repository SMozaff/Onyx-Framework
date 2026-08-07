use ed25519_dalek::SigningKey;
use platform_kernel::{
    ActorContext, AuthorityProof, AuthorityScope, DomainObjectRef, ObjectId, ProofType, Timestamp,
};
use security_adapter::Ed25519AuthorityVerifier;
use security_application::{AuthorityVerificationRequest, AuthorityVerifier};

fn request(now: u64) -> (SigningKey, AuthorityVerificationRequest) {
    let signing = SigningKey::from_bytes(&[7_u8; 32]);
    let organization = ObjectId([1; 16]);
    let actor = ActorContext {
        user_id: ObjectId([2; 16]),
        device_id: ObjectId([3; 16]),
        organization_id: organization,
    };
    let target = DomainObjectRef {
        id: ObjectId([4; 16]),
        r#type: "mission".to_string(),
        organization_id: organization,
    };
    let request = AuthorityVerificationRequest {
        proof: AuthorityProof {
            proof_type: ProofType::Jwt,
            scope: AuthorityScope {
                organization_id: organization,
                object_type: "mission".to_string(),
                object_id: Some(target.id),
                command_types: vec!["mission.Activate".to_string()],
                delegation_depth: 0,
            },
            issued_at: Timestamp::from_nanos(now - 10),
            expires_at: Timestamp::from_nanos(now + 10),
            signature: None,
        },
        actor,
        target,
        command_type: "mission.Activate".to_string(),
        trusted_now: Timestamp::from_nanos(now),
    };
    (signing, request)
}

#[tokio::test]
async fn valid_invalid_and_expired_authority_proofs() {
    let (signing, mut req) = request(100);
    let verifier = Ed25519AuthorityVerifier::new(vec![signing.verifying_key()]).unwrap();
    let signed = Ed25519AuthorityVerifier::sign_proof(&signing, &req);
    req.proof = signed;
    assert!(verifier.verify(&req).await.is_ok());

    req.proof.signature.as_mut().unwrap()[0] ^= 0xff;
    assert!(verifier.verify(&req).await.is_err());

    let (signing, mut expired) = request(100);
    expired.proof.expires_at = Timestamp::from_nanos(99);
    let signed_expired = Ed25519AuthorityVerifier::sign_proof(&signing, &expired);
    expired.proof = signed_expired;
    assert!(verifier.verify(&expired).await.is_err());
}
