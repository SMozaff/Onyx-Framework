//! # persistence-common
//!
//! Shared conversion helpers for persistence adapters (`persistence-postgres`,
//! `persistence-sqlite`): `Timestamp`<->milliseconds, identifier-bytes<->UUID,
//! JSON value<->text, and shared test fixtures.
//!
//! Added per Architectural Ruling "Add persistence-common Crate"
//! (DECISIONS.md item C1) to avoid duplicating this logic between the two
//! adapters. Not part of Team Prompt 2's original §2 File Manifest;
//! approved as an infrastructure-layer addition.

pub mod json;
pub mod test_helpers;
pub mod timestamp;
pub mod uuid;

pub use json::{text_to_value, value_to_text, JsonConversionError};
pub use timestamp::{millis_to_timestamp, timestamp_to_millis};
pub use uuid::{bytes_to_uuid, uuid_to_bytes};
