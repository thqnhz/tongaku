//! Playback abstraction: what the app needs from any audio backend.
use std::time::Duration;

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

/// Playback snapshot for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[non_exhaustive]
pub struct PlaybackStatus {
    pub position: Duration,
    pub duration: Option<Duration>,
    pub paused: bool,
}

impl PlaybackStatus {
    pub fn new(position: Duration, duration: Option<Duration>, paused: bool) -> Self {
        Self {
            position,
            duration,
            paused,
        }
    }
}

/// Audio playback backend. Implementation own their transport
/// (processes, devices); the app sees only this contract.
pub trait Player: Send + Sync {
    /// Play a media URL, replacing whatever is playing.
    fn play(&self, url: &str) -> Result<(), PlayerError>;

    fn toggle_pause(&self);

    fn stop(&self);

    /// Current playback snapshot.
    fn status(&self) -> PlaybackStatus;

    fn seek(&self, position: Duration);
}
