use async_trait::async_trait;
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use platform_kernel::Timestamp;
use security_application::{RotatingSecret, SecretProvider, SecretProviderError, SecretVersion};

#[derive(Clone, Debug, Default)]
pub struct EnvironmentSecretProvider;

impl EnvironmentSecretProvider {
    fn decode(name: &str, value: String) -> Result<Vec<u8>, SecretProviderError> {
        if let Some(encoded) = value.strip_prefix("base64url:") {
            return URL_SAFE_NO_PAD
                .decode(encoded)
                .map_err(|_| SecretProviderError::InvalidEncoding(name.to_string()));
        }
        if let Some(encoded) = value.strip_prefix("base64:") {
            return STANDARD
                .decode(encoded)
                .map_err(|_| SecretProviderError::InvalidEncoding(name.to_string()));
        }
        if let Some(encoded) = value.strip_prefix("hex:") {
            return hex::decode(encoded)
                .map_err(|_| SecretProviderError::InvalidEncoding(name.to_string()));
        }
        Ok(value.into_bytes())
    }

    fn previous_expiry(name: &str) -> Result<Option<Timestamp>, SecretProviderError> {
        let expiry_name = format!("{name}_PREVIOUS_VALID_UNTIL_UNIX");
        match std::env::var(&expiry_name) {
            Ok(raw) => {
                let seconds = raw
                    .parse::<u64>()
                    .map_err(|_| SecretProviderError::InvalidEncoding(expiry_name))?;
                Ok(Some(Timestamp::from_nanos(
                    seconds.saturating_mul(1_000_000_000),
                )))
            }
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(error) => Err(SecretProviderError::Infrastructure(error.to_string())),
        }
    }
}

#[async_trait]
impl SecretProvider for EnvironmentSecretProvider {
    async fn get(&self, name: &str) -> Result<RotatingSecret, SecretProviderError> {
        let current_raw = std::env::var(name).map_err(|error| match error {
            std::env::VarError::NotPresent => SecretProviderError::NotFound(name.to_string()),
            other => SecretProviderError::Infrastructure(other.to_string()),
        })?;
        let current = SecretVersion {
            value: Self::decode(name, current_raw)?,
            valid_until: None,
        };

        let previous_name = format!("{name}_PREVIOUS");
        let previous = match std::env::var(&previous_name) {
            Ok(raw) => {
                let expiry_name = format!("{name}_PREVIOUS_VALID_UNTIL_UNIX");
                let valid_until = Self::previous_expiry(name)?
                    .ok_or(SecretProviderError::InvalidEncoding(expiry_name))?;
                Some(SecretVersion {
                    value: Self::decode(&previous_name, raw)?,
                    valid_until: Some(valid_until),
                })
            }
            Err(std::env::VarError::NotPresent) => None,
            Err(error) => return Err(SecretProviderError::Infrastructure(error.to_string())),
        };

        Ok(RotatingSecret { current, previous })
    }
}
