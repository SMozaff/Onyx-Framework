//! Blob-store composition policy for the API process.
//!
//! The application sees only query_application::BlobStore. Provider selection
//! is an infrastructure concern resolved once at process startup.
//! Production requires an external object store; local disk remains available
//! for development and test compositions.

use std::{env, sync::Arc};

use anyhow::Context;
use huggingface_blob_storage::HuggingFaceBlobStore;
use local_blob_storage::LocalBlobStore;
use query_application::BlobStore;

use crate::config::AppConfig;

pub async fn build(config: &AppConfig) -> anyhow::Result<Arc<dyn BlobStore>> {
    let backend = env::var("ONYX_BLOB_STORE_BACKEND").unwrap_or_else(|_| {
        if config.is_production() {
            "huggingface".to_string()
        } else {
            "local".to_string()
        }
    });

    match backend.trim().to_ascii_lowercase().as_str() {
        "local" => {
            if config.is_production() {
                anyhow::bail!(
                    "production blob storage must use an external provider; \
                     set ONYX_BLOB_STORE_BACKEND=huggingface"
                );
            }

            let root = env::var("ONYX_BLOB_STORE_ROOT").unwrap_or_else(|_| {
                env::temp_dir()
                    .join("onyx-api-server-blobs")
                    .to_string_lossy()
                    .into_owned()
            });
            let store = LocalBlobStore::open(&root)
                .await
                .with_context(|| format!("opening local blob store at {root}"))?;
            Ok(Arc::new(store))
        }
        "huggingface" | "huggingface_s3" => {
            let required = [
                "ONYX_BLOB_STORE_S3_ENDPOINT",
                "ONYX_BLOB_STORE_S3_BUCKET",
                "ONYX_BLOB_STORE_S3_ACCESS_KEY_ID",
                "ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY",
            ];
            let missing = missing_required_env(&required, |name| {
                env::var(name)
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false)
            });
            if !missing.is_empty() {
                anyhow::bail!(
                    "{}",
                    missing
                        .iter()
                        .map(|name| format!(
                            "{name} is required for Hugging Face blob storage — set this in your deployment secrets (Render dashboard: Environment → Secret Files, or Helm secretRef: onyx-api-secrets)"
                        ))
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            }

            let endpoint = required_env("ONYX_BLOB_STORE_S3_ENDPOINT")?;
            let bucket = required_env("ONYX_BLOB_STORE_S3_BUCKET")?;
            let access_key = required_env("ONYX_BLOB_STORE_S3_ACCESS_KEY_ID")?;
            let secret_key = required_env("ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY")?;

            let store = HuggingFaceBlobStore::new(&endpoint, bucket, access_key, secret_key)
                .map_err(|error| {
                    anyhow::anyhow!("invalid Hugging Face blob-store configuration: {error}")
                })?;

            Ok(Arc::new(store))
        }
        other => anyhow::bail!(
            "invalid ONYX_BLOB_STORE_BACKEND={other:?}; expected local or huggingface"
        ),
    }
}

fn required_env(name: &str) -> anyhow::Result<String> {
    env::var(name).with_context(|| format!(
        "{name} is required for Hugging Face blob storage — set this in your deployment secrets (Render dashboard: Environment → Secret Files, or Helm secretRef: onyx-api-secrets)"
    ))
}


fn missing_required_env<'a>(
    required: &'a [&'a str],
    is_set: impl Fn(&str) -> bool,
) -> Vec<&'a str> {
    required
        .iter()
        .copied()
        .filter(|name| !is_set(name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::missing_required_env;

    #[test]
    fn hugging_face_preflight_reports_every_missing_setting() {
        let required = [
            "ONYX_BLOB_STORE_S3_ENDPOINT",
            "ONYX_BLOB_STORE_S3_BUCKET",
            "ONYX_BLOB_STORE_S3_ACCESS_KEY_ID",
            "ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY",
        ];
        assert_eq!(missing_required_env(&required, |_| false), required.to_vec());
    }

    #[test]
    fn hugging_face_preflight_omits_configured_settings() {
        let required = ["ENDPOINT", "BUCKET", "ACCESS_KEY", "SECRET_KEY"];
        let missing = missing_required_env(&required, |name| name == "ENDPOINT" || name == "BUCKET");
        assert_eq!(missing, vec!["ACCESS_KEY", "SECRET_KEY"]);
    }
}
