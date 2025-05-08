use std::sync::Arc;
use tokio::sync::RwLock;

pub mod broadcast;
pub mod connection;
pub mod signaling;

pub use yrs_tokio_macros::*;

pub type AwarenessRef = Arc<RwLock<yrs::sync::Awareness>>;