use async_trait::async_trait;
use platform_kernel::Timestamp;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretVersion {
    pub value: Vec<u8>,
    pub valid_until: Option<Timestamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RotatingSecret {
    pub current: SecretVersion,
    pub previous: Option<SecretVersion>,
}

impl RotatingSecret {
    pub fn verification_candidates(&self, now: Timestamp) -> Vec<&[u8]> {
        let mut values = vec![self.current.value.as_slice()];
        if let Some(previous) = &self.previous {
            if previous
                .valid_until
                .map(|expiry| expiry > now)
                .unwrap_or(false)
            {
                values.push(previous.value.as_slice());
            }
        }
        values
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretProviderError {
    #[error("secret {0} is not configured")]
    NotFound(String),
    #[error("secret {0} has invalid encoding")]
    InvalidEncoding(String),
    #[error("secret provider failed: {0}")]
    Infrastructure(String),
}

#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn get(&self, name: &str) -> Result<RotatingSecret, SecretProviderError>;
}
