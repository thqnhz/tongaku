//! yt-dlp wrapper: page URL to direct stream URL.
use thiserror::Error;

#[derive(Debug, Error)]
#[error("yt-dlp: {0}")]
pub struct Error(String);

/// Resolve a direct audio stream URL for a page URL (e.g. a watch URL).
pub fn stream_url(page_url: &str) -> Result<String, Error> {
    let output = std::process::Command::new("yt-dlp")
        .args([
            "--no-playlist",
            "--socket-timeout",
            "15",
            "-f",
            "bestaudio[ext=m4a]/bestaudio",
            "-g",
            page_url,
        ])
        .output()
        .map_err(|err| Error(format!("failed to spawn yt-dlp: {err}")))?;

    if !output.status.success() {
        return Err(Error(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }

    let url = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if url.is_empty() {
        Err(Error("no stream URL in output".to_owned()))
    } else {
        Ok(url)
    }
}
