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

/// One resolved stream: where to play + where the URL expires.
#[derive(Debug, Clone)]
pub struct AudioStream {
    /// Direct audio URL
    pub url: String,
    pub mime: String,
    pub bitrate: u32,
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
    pub fn get_track_with_stream(
        &self,
        video_id: &str,
    ) -> Result<(Track, Option<AudioStream>), YtError> {
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
        let stream = pick_audio_stream(&response["streamingData"]);
        Ok((
            Track {
                id: video_id.to_owned(),
                title: details["title"].as_str().unwrap_or_default().to_owned(),
                artist: details["author"].as_str().unwrap_or_default().to_owned(),
                duration_ms: details["lengthSeconds"]
                    .as_str()
                    .and_then(|seconds| seconds.parse::<u64>().ok())
                    .unwrap_or_default()
                    * 1000,
            },
            stream,
        ))
    }

    /// Resolve a direct audio stream URL via yt-dlp.
    pub fn stream_url(video_id: &str) -> Result<String, YtError> {
        let watch = format!("https://music.youtube.com/watch?v={video_id}");
        Ok(tongaku_ytdlp::stream_url(&watch)?)
    }
}

fn pick_audio_stream(data: &Value) -> Option<AudioStream> {
    let mut best: Option<AudioStream> = None;
    let mut consider = |formats: &Value| {
        for format in formats.as_array()? {
            let mime = format["mimeType"].as_str().unwrap_or_default();
            if !mime.starts_with("audio/") {
                continue;
            }
            let url = format["url"].as_str()?;
            let bitrate = format["bitrate"].as_u64().unwrap_or(0) as u32;
            let candicate = AudioStream {
                url: url.to_owned(),
                mime: mime.to_owned(),
                bitrate,
            };
            if best
                .as_ref()
                .map(|b| candicate.bitrate > b.bitrate)
                .unwrap_or(true)
            {
                best = Some(candicate);
            }
        }
        None::<()>
    };
    consider(&data["adaptiveFormats"]);
    consider(&data["formats"]);
    best
}
