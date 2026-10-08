//! Application-layer security ports for authority verification, quota/rate
//! governance, and secret retrieval. Infrastructure implementations live in
//! `security-adapter`.

pub mod ports;
pub use ports::*;
