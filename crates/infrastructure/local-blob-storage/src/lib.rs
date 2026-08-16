//! # local-blob-storage
//!
//! A local-disk implementation of `query_application::BlobStore`. Phase 1
//! (Desktop & Web Completion) addition — see that port's doc comment for
//! why it exists. Suitable for Desktop (which already has direct
//! filesystem access via Tauri) and for local development/testing; not
//! intended as the eventual production Web/Cloud Relay storage backend
//! (that would be an S3-compatible or similar adapter implementing the
//! same `BlobStore` trait — not built here, since Web file sharing is
//! out of scope entirely per the confirmed product decision, and no
//! other current consumer needs it).
//!
//! # Layout
//! Content-addressed, sharded by the first 2 hex characters of the
//! blob's [`BlobKey`] to avoid an unbounded flat directory (a real
//! concern once an org has thousands of files — most filesystems degrade
//! with very large single directories):
//! `{root}/{key[0..2]}/{key}`.
//!
//! # Durability
//! Writes go to a temp file in the same shard directory, then
//! `rename()` into place — rename is atomic on the same filesystem (POSIX
//! guarantee, and the reason the temp file must be created inside the
//! target shard directory rather than a shared tmp dir that might be a
//! different filesystem/mount). This means a concurrent reader never
//! observes a partially-written blob, and a crash mid-write leaves only
//! an orphaned temp file, never a corrupt blob at its real path.

use async_trait::async_trait;
use query_application::{BlobKey, BlobStore, BlobStoreError};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

/// A `BlobStore` backed by a local directory tree. See the module-level
/// doc comment for the on-disk layout and durability guarantees.
pub struct LocalBlobStore {
    root: PathBuf,
}

impl LocalBlobStore {
    /// Opens (creating if needed) a `LocalBlobStore` rooted at `root`.
    ///
    /// # Errors
    /// Returns [`BlobStoreError::Io`] if `root` cannot be created (e.g.
    /// permission denied, or a path component exists as a non-directory
    /// file).
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self, BlobStoreError> {
        let root = root.into();
        tokio::fs::create_dir_all(&root)
            .await
            .map_err(|e| BlobStoreError::Io(format!("creating blob store root: {e}")))?;
        Ok(Self { root })
    }

    /// Rejects a key that would escape `root` via path traversal (e.g.
    /// `../../etc/passwd`) or that is empty. `BlobKey` is caller-supplied
    /// (ultimately from a hash a client claims, before this store has
    /// verified anything about it — see `put`'s doc comment on
    /// verification), so it must never be trusted as a safe path
    /// component without this check.
    fn shard_and_file(&self, key: &BlobKey) -> Result<(PathBuf, PathBuf), BlobStoreError> {
        let raw = key.0.trim();
        if raw.is_empty() {
            return Err(BlobStoreError::Io("blob key must not be empty".to_string()));
        }
        // A `ContentHash` (see `file_domain::value::ContentHash`, whose
        // wire format this key is meant to mirror per `BlobKey`'s own
        // doc comment) is lowercase hex — reject anything else outright
        // rather than attempt to sanitize it, since a hash-shaped key is
        // exactly what every legitimate caller already has in hand.
        if !raw.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(BlobStoreError::Io(format!(
                "blob key must be a hex string, got: {raw}"
            )));
        }
        let shard = if raw.len() >= 2 { &raw[0..2] } else { raw };
        let shard_dir = self.root.join(shard);
        let file_path = shard_dir.join(raw);
        Ok((shard_dir, file_path))
    }
}

#[async_trait]
impl BlobStore for LocalBlobStore {
    /// Writes `content` to disk under `key`. Does **not** itself verify
    /// that `content`'s actual hash matches `key` — that verification is
    /// `file-domain`'s own responsibility (`ContentHash`/`AppendChunk`'s
    /// hash check happens at the domain-command layer, upstream of this
    /// call); this store is a dumb, content-addressed filesystem, not a
    /// re-implementation of that validation. A caller that writes
    /// mismatched content under a key has violated its own contract with
    /// this store, not triggered a `BlobStoreError::HashMismatch` here
    /// (that variant exists for a future integrity-check-on-read
    /// feature — not yet built, flagged rather than silently unused).
    async fn put(&self, key: &BlobKey, content: &[u8]) -> Result<(), BlobStoreError> {
        let (shard_dir, file_path) = self.shard_and_file(key)?;
        tokio::fs::create_dir_all(&shard_dir)
            .await
            .map_err(|e| BlobStoreError::Io(format!("creating shard dir: {e}")))?;

        // Idempotent per the trait's contract: if the exact path already
        // exists, treat a re-`put` as a success without rewriting —
        // avoids a redundant disk write for the common "retried command
        // after a crash" case this port's doc comment calls out.
        if tokio::fs::try_exists(&file_path).await.unwrap_or(false) {
            return Ok(());
        }

        let temp_path = shard_dir.join(format!(".{}.tmp-{}", key.0, std::process::id()));
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|e| BlobStoreError::Io(format!("creating temp file: {e}")))?;
        file.write_all(content)
            .await
            .map_err(|e| BlobStoreError::Io(format!("writing temp file: {e}")))?;
        file.sync_all()
            .await
            .map_err(|e| BlobStoreError::Io(format!("syncing temp file: {e}")))?;
        drop(file);

        tokio::fs::rename(&temp_path, &file_path).await.map_err(|e| {
            // Best-effort cleanup of the orphaned temp file — a failure
            // here is secondary to the rename error itself, so it's
            // logged, not propagated (propagating it would mask the
            // actual rename failure the caller needs to see).
            let cleanup_path = temp_path.clone();
            tokio::spawn(async move {
                if let Err(cleanup_err) = tokio::fs::remove_file(&cleanup_path).await {
                    tracing::warn!(
                        path = %cleanup_path.display(),
                        error = %cleanup_err,
                        "failed to clean up orphaned blob temp file after a failed rename"
                    );
                }
            });
            BlobStoreError::Io(format!("renaming temp file into place: {e}"))
        })?;

        Ok(())
    }

    async fn get(&self, key: &BlobKey) -> Result<Option<Vec<u8>>, BlobStoreError> {
        let (_, file_path) = self.shard_and_file(key)?;
        match tokio::fs::read(&file_path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BlobStoreError::Io(format!("reading blob: {e}"))),
        }
    }

    async fn exists(&self, key: &BlobKey) -> Result<bool, BlobStoreError> {
        let (_, file_path) = self.shard_and_file(key)?;
        Ok(tokio::fs::try_exists(&file_path).await.unwrap_or(false))
    }

    async fn delete(&self, key: &BlobKey) -> Result<(), BlobStoreError> {
        let (_, file_path) = self.shard_and_file(key)?;
        match tokio::fs::remove_file(&file_path).await {
            Ok(()) => Ok(()),
            // Deleting an already-absent blob is not an error — matches
            // `put`'s idempotency stance, and the caller's intent
            // ("this blob should not exist") is already satisfied.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(BlobStoreError::Io(format!("deleting blob: {e}"))),
        }
    }
}

/// Not part of the public `BlobStore` trait (callers should not need
/// filesystem paths — that would leak this adapter's implementation
/// detail into port-level code), but useful for tests/diagnostics that
/// need to assert directly against disk state.
impl LocalBlobStore {
    /// The root directory this store was opened against.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(hex: &str) -> BlobKey {
        BlobKey::new(hex.to_string())
    }

    async fn temp_store() -> (LocalBlobStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir must succeed");
        let store = LocalBlobStore::open(dir.path())
            .await
            .expect("open must succeed");
        (store, dir)
    }

    #[tokio::test]
    async fn put_then_get_round_trips_content() {
        let (store, _dir) = temp_store().await;
        let k = key("abcdef0123456789");
        store.put(&k, b"hello world").await.unwrap();

        let read_back = store.get(&k).await.unwrap();
        assert_eq!(read_back, Some(b"hello world".to_vec()));
    }

    #[tokio::test]
    async fn get_missing_key_returns_none() {
        let (store, _dir) = temp_store().await;
        let result = store.get(&key("deadbeef")).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn exists_reflects_put_and_delete() {
        let (store, _dir) = temp_store().await;
        let k = key("ff00ff00");

        assert!(!store.exists(&k).await.unwrap());
        store.put(&k, b"content").await.unwrap();
        assert!(store.exists(&k).await.unwrap());
        store.delete(&k).await.unwrap();
        assert!(!store.exists(&k).await.unwrap());
    }

    #[tokio::test]
    async fn delete_missing_key_is_not_an_error() {
        let (store, _dir) = temp_store().await;
        store.delete(&key("0000000000")).await.unwrap();
    }

    #[tokio::test]
    async fn put_is_idempotent_for_identical_content() {
        let (store, _dir) = temp_store().await;
        let k = key("1234abcd");
        store.put(&k, b"same content").await.unwrap();
        store.put(&k, b"same content").await.unwrap();

        let read_back = store.get(&k).await.unwrap();
        assert_eq!(read_back, Some(b"same content".to_vec()));
    }

    #[tokio::test]
    async fn rejects_non_hex_key() {
        let (store, _dir) = temp_store().await;
        let result = store.put(&key("not-hex!"), b"x").await;
        assert!(matches!(result, Err(BlobStoreError::Io(_))));
    }

    #[tokio::test]
    async fn rejects_empty_key() {
        let (store, _dir) = temp_store().await;
        let result = store.put(&key(""), b"x").await;
        assert!(matches!(result, Err(BlobStoreError::Io(_))));
    }

    #[tokio::test]
    async fn blobs_are_sharded_by_key_prefix_on_disk() {
        let (store, _dir) = temp_store().await;
        let k = key("ab1234");
        store.put(&k, b"x").await.unwrap();

        let expected_path = store.root().join("ab").join("ab1234");
        assert!(tokio::fs::try_exists(&expected_path).await.unwrap());
    }

    #[tokio::test]
    async fn distinct_keys_do_not_collide() {
        let (store, _dir) = temp_store().await;
        store.put(&key("aaaa"), b"first").await.unwrap();
        store.put(&key("bbbb"), b"second").await.unwrap();

        assert_eq!(store.get(&key("aaaa")).await.unwrap(), Some(b"first".to_vec()));
        assert_eq!(store.get(&key("bbbb")).await.unwrap(), Some(b"second".to_vec()));
    }
}
