//! `serde_json::Value` <-> `String` helpers, for adapters storing JSON as
//! `TEXT` (SQLite; per the SQLite Schema Mapping ruling, DECISIONS.md S1).
//! PostgreSQL uses native `JSONB` via sqlx's `Json<T>`/`json` feature and
//! does not need these helpers.

use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum JsonConversionError {
    #[error("failed to serialize JSON value: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Serialize a `serde_json::Value` to a `String` for storage in a `TEXT`
/// column (SQLite).
pub fn value_to_text(value: &Value) -> Result<String, JsonConversionError> {
    Ok(serde_json::to_string(value)?)
}

/// Deserialize a `TEXT` column's contents back into a `serde_json::Value`.
pub fn text_to_value(text: &str) -> Result<Value, JsonConversionError> {
    Ok(serde_json::from_str(text)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn roundtrips_object() {
        let value = json!({"a": 1, "b": [1, 2, 3], "c": "text"});
        let text = value_to_text(&value).unwrap();
        let restored = text_to_value(&text).unwrap();
        assert_eq!(value, restored);
    }

    #[test]
    fn rejects_malformed_json() {
        let result = text_to_value("{not valid json");
        assert!(result.is_err());
    }
}
