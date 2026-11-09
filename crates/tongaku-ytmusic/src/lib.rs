//! YouTube Music (InnerTube) client
use serde_json::{Value, json};
use thiserror::Error;

pub use tongaku_core::Track;

const YTM_BASE: &str = "https://music.youtube.com/youtubei/v1";

// InnerTube client identity.
// YouTube gates streamingData per client flavor;
// ANDROID usually returns the direct `url` fields instead of signatureCipher.
const CLIENT_NAME: &str = "ANDROID";
const CLIENT_VERSION: &str = "20.10.38";
const USER_AGENT: &str = "com.google.android.youtube/20.10.38 (Linux; U; Android 11) gzip";

#[derive(Debug, Error)]
pub enum YtError {
    #[error("http: {0}")]
    Http(#[from] ureq::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("track unavailable: {0}")]
    Unavailable(String),
    #[error("no playable audio stream (all formats ciphered or absent)")]
    NoAudioStream,
    #[error(transparent)]
    External(#[from] tongaku_ytdlp::Error),
}

pub struct Client {
    agent: ureq::Agent,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_connect(Some(std::time::Duration::from_secs(10)))
            .timeout_per_read(Some(std::time::Duration::from_secs(30)))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }

    /// Metadata + direct audio URL for one video/track id.
    pub fn get_track_with_stream(&self, video_id: &str) -> Result<Track, YtError> {
        let body = json!({
            "context": {
                "client": {
                    "clientName": CLIENT_NAME,
                    "clientVersion": CLIENT_VERSION,
                    "androidSdkVersion": 30,
                    "hl": "en",
                    "gl": "US",
                }
            },
            "videoId": video_id,
            "contentCheckOk": true,
            "racyCheckOk": true,
        });

        let mut response = self
            .agent
            .post(&format!("{YTM_BASE}/player?alt=json"))
            .header("User-Agent", USER_AGENT)
            .header("Content-Type", "application/json")
            .header("X-Goog-Api-Format-Version", "2")
            .send_json(&body)?;

        let response: Value = response.body_mut().read_json()?;

        if response
            .pointer("/playabilityStatus/status")
            .and_then(Value::as_str)
            != Some("OK")
        {
            let reason = response
                .pointer("/playabilityStatus/reason")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            return Err(YtError::Unavailable(reason.to_owned()));
        }

        let details = &response["videoDetails"];
        Ok(Track {
            id: video_id.to_owned(),
            title: details["title"].as_str().unwrap_or_default().to_owned(),
            artist: details["author"].as_str().unwrap_or_default().to_owned(),
            duration_ms: details["lengthSeconds"]
                .as_str()
                .and_then(|seconds| seconds.parse::<u64>().ok())
                .unwrap_or_default()
                * 1000,
        })
    }

    /// Resolve a direct audio stream URL via yt-dlp.
    pub fn stream_url(video_id: &str) -> Result<String, YtError> {
        let watch = format!("https://music.youtube.com/watch?v={video_id}");
        Ok(tongaku_ytdlp::stream_url(&watch)?)
    }

    /// Search results for a query string.
    pub fn search(&self, query: &str) -> Result<Vec<Track>, YtError> {
        let body = json!({
            "context": {
                "client": {
                    "clientName": CLIENT_NAME,
                    "clientVersion": CLIENT_VERSION,
                    "androidSdkVersion": 30,
                    "hl": "en",
                    "gl": "US",
                }
            },
            "query": query,
        });

        let mut response = self
            .agent
            .post(&format!("{YTM_BASE}/search?alt=json"))
            .header("User-Agent", USER_AGENT)
            .header("Content-Type", "application/json")
            .header("X-Goog-Api-Format-Version", "2")
            .send_json(&body)?;

        let response: Value = response.body_mut().read_json()?;

        let mut tracks = Vec::new();
        for section in response["contents"]["sectionListRenderer"]["contents"]
            .as_array()
            .unwrap_or(&Vec::new())
        {
            for item in section["itemSectionRenderer"]["contents"]
                .as_array()
                .unwrap_or(&Vec::new())
            {
                let Some(video) = item.get("compactVideoRenderer") else {
                    continue;
                };
                let id = video["videoId"].as_str().unwrap_or_default();
                if id.is_empty() || tracks.iter().any(|t: &Track| t.id == id) {
                    continue;
                }
                let runs = |key: &str| {
                    video[key]["runs"]
                        .as_array()
                        .map(|v| {
                            v.iter()
                                .filter_map(|r| r["text"].as_str())
                                .collect::<Vec<_>>()
                                .join("")
                        })
                        .unwrap_or_default()
                };
                tracks.push(Track {
                    id: id.to_owned(),
                    title: runs("title"),
                    artist: runs("longByLineText"),
                    duration_ms: 0,
                });
                if tracks.len() >= 20 {
                    return Ok(tracks);
                }
            }
        }
        Ok(tracks)
    }
}
