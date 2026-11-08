//! Playback abstraction: what the app needs from any audio backend.
use thiserror::Error;

#[derive(Debug, Error)]
#[error("player: {0}")]
pub struct PlayerError(String);

impl PlayerError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl From<String> for PlayerError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

/// Audio playback backend. Implementation own their transport
/// (processes, devices); the app sees only this contract.
pub trait Player: Send + Sync {
    /// Play a media URL, replacing whatever is playinng.
    fn play(&self, url: &str) -> Result<(), PlayerError>;

    fn toggle_pause(&self);

    fn stop(&self);
}
