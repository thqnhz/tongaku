//! Queue: ordered up-next tracks.
use std::collections::VecDeque;

use tongaku_ytmusic::Track;

/// Playback queue: VecDeque of tracks + cursor.
#[derive(Debug, Default)]
pub struct Queue {
    inner: VecDeque<Track>,
    now: Option<usize>,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            inner: VecDeque::new(),
            now: None,
        }
    }

    /// Push track; set cursor to end.
    pub fn push(&mut self, track: Track) {
        self.inner.push_back(track);
        self.now = Some(self.inner.len() - 1);
    }

    /// Find id in queue; set cursor.
    pub fn select(&mut self, id: &str) -> bool {
        if let Some(ix) = self.inner.iter().position(|t| t.id == id) {
            self.now = Some(ix);
            true
        } else {
            false
        }
    }

    /// Next track id after cursor.
    pub fn next_id(&self, looping: bool, shuffle: bool) -> Option<String> {
        let now = self.now?;
        let len = self.inner.len();
        if len == 0 {
            return None;
        }
        if shuffle && len > 1 {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos() as usize;
            let mut idx = seed % len;
            if idx == now {
                idx = (idx + 1) % len;
            }
            return self.inner.get(idx).map(|t| t.id.clone());
        }
        if now + 1 < len {
            self.inner.get(now + 1).map(|t| t.id.clone())
        } else if looping {
            self.inner.front().map(|t| t.id.clone())
        } else {
            None
        }
    }

    /// Previous track id before cursor.
    pub fn prev_id(&self) -> Option<String> {
        self.now
            .filter(|&now| now > 0)
            .and_then(|now| self.inner.get(now - 1).map(|t| t.id.clone()))
    }

    /// Up to n ids after cursor.
    pub fn upcoming_ids(&self, n: usize) -> Vec<String> {
        let start = self.now.map(|now| now + 1).unwrap_or(0);
        self.inner
            .iter()
            .skip(start)
            .take(n)
            .map(|t| t.id.clone())
            .collect()
    }
}
