//! # messaging-adapter
//!
//! In-process `EventPublisher` implementation using a Tokio broadcast
//! channel. This adapter is the "embedded channel" variant referenced in
//! Team Prompt 2 §2 and §3.5 ("NATS or embedded channel implementation").
//! NATS integration is out of scope for Increment 2 — a NATS adapter
//! would implement the same `EventPublisher` port and be wired in at the
//! binary level; see DECISIONS.md item M1.

pub mod channel_publisher;

pub use channel_publisher::{ChannelEventPublisher, ChannelEventReceiver};
