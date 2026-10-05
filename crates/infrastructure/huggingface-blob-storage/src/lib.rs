//! Hugging Face Storage Bucket implementation of the ONYX BlobStore port.
//!
//! Hugging Face exposes Storage Buckets through an AWS Signature V4 compatible
//! S3 gateway. This adapter contains all provider-specific addressing and
//! signing details so application/domain crates remain provider-neutral.
//!
//! The gateway currently requires an endpoint scoped to a Hugging Face
//! namespace, region us-east-1, and path-style bucket addressing.
//!
//! See: https://huggingface.co/docs/hub/storage-buckets-s3

use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use query_application::{BlobKey, BlobStore, BlobStoreError};
use reqwest::{Client, Method, StatusCode};
use sha2::{Digest, Sha256};
use std::{fmt, sync::Arc};
use url::Url;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct HuggingFaceBlobStore {
    client: Client,
    endpoint: Url,
    bucket: String,
    access_key_id: Arc<str>,
    secret_access_key: Arc<str>,
}

impl fmt::Debug for HuggingFaceBlobStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HuggingFaceBlobStore")
            .field("endpoint", &self.endpoint)
            .field("bucket", &self.bucket)
            .field("access_key_id", &self.access_key_id)
            .field("secret_access_key", &"[REDACTED]")
            .finish()
    }
}

impl HuggingFaceBlobStore {
    pub fn new(
        endpoint: &str,
        bucket: impl Into<String>,
        access_key_id: impl Into<Arc<str>>,
        secret_access_key: impl Into<Arc<str>>,
    ) -> Result<Self, BlobStoreError> {
        let endpoint = Url::parse(endpoint)
            .map_err(|error| BlobStoreError::Io(format!("invalid HF S3 endpoint: {error}")))?;
        if endpoint.scheme() != "https" {
            return Err(BlobStoreError::Io(
                "HF S3 endpoint must use https".to_string(),
            ));
        }

        let bucket = bucket.into();
        if bucket.is_empty() || bucket.contains('/') {
            return Err(BlobStoreError::Io(
                "HF S3 bucket must be a non-empty bare bucket name".to_string(),
            ));
        }
        if access_key_id.as_ref().is_empty() || secret_access_key.as_ref().is_empty() {
            return Err(BlobStoreError::Io(
                "HF S3 credentials must not be empty".to_string(),
            ));
        }

        Ok(Self {
            client: Client::new(),
            endpoint,
            bucket,
            access_key_id: access_key_id.into(),
            secret_access_key: secret_access_key.into(),
        })
    }

    fn object_url(&self, key: &BlobKey) -> Result<Url, BlobStoreError> {
        if key.0.is_empty() || key.0.starts_with('/') {
            return Err(BlobStoreError::Io(
                "blob key must be non-empty and relative".to_string(),
            ));
        }

        let mut url = self.endpoint.clone();
        {
            let mut segments = url.path_segments_mut().map_err(|_| {
                BlobStoreError::Io("HF S3 endpoint cannot be used as a path base".to_string())
            })?;
            segments.pop_if_empty();
            segments.push(&self.bucket);
            for segment in key.0.split('/') {
                if segment.is_empty() {
                    return Err(BlobStoreError::Io(
                        "blob key contains an empty path segment".to_string(),
                    ));
                }
                segments.push(segment);
            }
        }
        Ok(url)
    }

    async fn request(
        &self,
        method: Method,
        key: &BlobKey,
        body: Option<&[u8]>,
    ) -> Result<reqwest::Response, BlobStoreError> {
        let url = self.object_url(key)?;
        let body = body.unwrap_or_default();
        let payload_hash = hex_sha256(body);
        let timestamp = Utc::now();
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
        let date = timestamp.format("%Y%m%d").to_string();
        let host = host_header(&url)?;

        let canonical_headers = format!(
            "host:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";
        let canonical_request = format!(
            "{}\n{}\n\n{}\n{}\n{}",
            method.as_str(),
            url.path(),
            canonical_headers,
            signed_headers,
            payload_hash
        );
        let scope = format!("{date}/us-east-1/s3/aws4_request");
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
            hex_sha256(canonical_request.as_bytes())
        );
        let signing_key = signing_key(&self.secret_access_key, &date);
        let signature = hex::encode(hmac_bytes(&signing_key, &string_to_sign));

        let authorization = format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            self.access_key_id
        );

        self.client
            .request(method, url)
            .header("Host", host)
            .header("x-amz-content-sha256", payload_hash)
            .header("x-amz-date", amz_date)
            .header("Authorization", authorization)
            .body(body.to_vec())
            .send()
            .await
            .map_err(|error| BlobStoreError::Io(format!("HF S3 request failed: {error}")))
    }

    fn status_error(operation: &str, status: StatusCode) -> BlobStoreError {
        BlobStoreError::Io(format!("HF S3 {operation} failed with HTTP {status}"))
    }
}

#[async_trait]
impl BlobStore for HuggingFaceBlobStore {
    async fn put(&self, key: &BlobKey, content: &[u8]) -> Result<(), BlobStoreError> {
        let response = self.request(Method::PUT, key, Some(content)).await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Self::status_error("PUT", response.status()))
        }
    }

    async fn get(&self, key: &BlobKey) -> Result<Option<Vec<u8>>, BlobStoreError> {
        let response = self.request(Method::GET, key, None).await?;
        match response.status() {
            StatusCode::OK => response
                .bytes()
                .await
                .map(|bytes| Some(bytes.to_vec()))
                .map_err(|error| BlobStoreError::Io(format!("HF S3 GET body failed: {error}"))),
            StatusCode::NOT_FOUND => Ok(None),
            status => Err(Self::status_error("GET", status)),
        }
    }

    async fn exists(&self, key: &BlobKey) -> Result<bool, BlobStoreError> {
        let response = self.request(Method::HEAD, key, None).await?;
        match response.status() {
            status if status.is_success() => Ok(true),
            StatusCode::NOT_FOUND => Ok(false),
            status => Err(Self::status_error("HEAD", status)),
        }
    }

    async fn delete(&self, key: &BlobKey) -> Result<(), BlobStoreError> {
        let response = self.request(Method::DELETE, key, None).await?;
        match response.status() {
            StatusCode::NO_CONTENT | StatusCode::OK | StatusCode::NOT_FOUND => Ok(()),
            status => Err(Self::status_error("DELETE", status)),
        }
    }
}

fn host_header(url: &Url) -> Result<String, BlobStoreError> {
    let host = url
        .host_str()
        .ok_or_else(|| BlobStoreError::Io("HF S3 endpoint has no host".to_string()))?;
    Ok(match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_string(),
    })
}

fn hex_sha256(value: &[u8]) -> String {
    hex::encode(Sha256::digest(value))
}

fn hmac_bytes(key: &[u8], value: &str) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts arbitrary key length");
    mac.update(value.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

fn signing_key(secret: &str, date: &str) -> Vec<u8> {
    let k_date = hmac_bytes(format!("AWS4{secret}").as_bytes(), date);
    let k_region = hmac_bytes(&k_date, "us-east-1");
    let k_service = hmac_bytes(&k_region, "s3");
    hmac_bytes(&k_service, "aws4_request")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signing_key_is_deterministic_and_32_bytes() {
        let first = signing_key("secret", "20150830");
        let second = signing_key("secret", "20150830");
        assert_eq!(first, second);
        assert_eq!(first.len(), 32);
    }

    #[test]
    fn rejects_non_https_or_invalid_bucket() {
        assert!(HuggingFaceBlobStore::new(
            "http://s3.hf.co/ns",
            "bucket",
            "HFAK",
            "secret"
        )
        .is_err());
        assert!(HuggingFaceBlobStore::new(
            "https://s3.hf.co/ns",
            "namespace/bucket",
            "HFAK",
            "secret"
        )
        .is_err());
    }

    #[test]
    fn object_paths_are_namespace_scoped_and_path_style() {
        let store = HuggingFaceBlobStore::new(
            "https://s3.hf.co/onyx",
            "production",
            "HFAK",
            "secret",
        )
        .unwrap();
        let url = store.object_url(&BlobKey::new("aa/bb")).unwrap();
        assert_eq!(url.as_str(), "https://s3.hf.co/onyx/production/aa/bb");
    }
}
