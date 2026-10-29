//! Encrypted local database (SQLCipher).
//!
//! Every open sets the key before any other statement. The key is the 32-byte
//! value from `local_key::derive_local_key`, passed to SQLCipher as hex text.
//! A wrong or missing key makes every read fail, so data stays unreadable
//! without the person's password.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use crate::local_key::KEY_LEN;

/// Opens (creating if needed) the encrypted database at `path` with `key`.
pub async fn open_encrypted_pool(path: &Path, key: &[u8; KEY_LEN]) -> Result<SqlitePool, String> {
    let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .pragma("key", hex);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| format!("open encrypted database: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Row;

    fn scratch_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("onyx-enc-{}.sqlite", uuid::Uuid::new_v4()))
    }

    #[tokio::test]
    async fn data_survives_reopen_with_the_same_key() {
        let path = scratch_path();
        let key = [1u8; KEY_LEN];
        {
            let pool = open_encrypted_pool(&path, &key).await.unwrap();
            sqlx::query("CREATE TABLE t (v TEXT)")
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO t (v) VALUES ('secret-row')")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }
        let pool = open_encrypted_pool(&path, &key).await.unwrap();
        let v: String = sqlx::query("SELECT v FROM t")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get("v");
        assert_eq!(v, "secret-row");
        pool.close().await;
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn wrong_key_cannot_read_the_data() {
        let path = scratch_path();
        {
            let pool = open_encrypted_pool(&path, &[1u8; KEY_LEN]).await.unwrap();
            sqlx::query("CREATE TABLE t (v TEXT)")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }
        let pool = open_encrypted_pool(&path, &[2u8; KEY_LEN]).await.unwrap();
        assert!(sqlx::query("SELECT name FROM sqlite_master")
            .fetch_all(&pool)
            .await
            .is_err());
        pool.close().await;
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn unkeyed_open_cannot_read_the_file() {
        let path = scratch_path();
        {
            let pool = open_encrypted_pool(&path, &[3u8; KEY_LEN]).await.unwrap();
            sqlx::query("CREATE TABLE t (v TEXT)")
                .execute(&pool)
                .await
                .unwrap();
            pool.close().await;
        }
        let options = SqliteConnectOptions::new().filename(&path);
        let plain = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        assert!(sqlx::query("SELECT name FROM sqlite_master")
            .fetch_all(&plain)
            .await
            .is_err());
        plain.close().await;
        let _ = std::fs::remove_file(&path);
    }
}
