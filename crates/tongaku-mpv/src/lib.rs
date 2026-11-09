//! Playback engine
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::json;

use tongaku_playback::{PlaybackStatus, Player, PlayerError};

struct Ipc {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next_id: i64,
}

impl Ipc {
    /// Send one command, skip async events, return the matching response.
    fn transact(
        &mut self,
        command: serde_json::Value,
    ) -> Result<serde_json::Value, tongaku_playback::PlayerError> {
        self.next_id += 1;
        let id = self.next_id;
        let line = json!({ "command": command, "request_id": id });
        writeln!(self.writer, "{line}")
            .map_err(|err| tongaku_playback::PlayerError::new(format!("ipc write: {err}")))?;

        loop {
            let mut response = String::new();
            let n = self
                .reader
                .read_line(&mut response)
                .map_err(|err| tongaku_playback::PlayerError::new(format!("ipc read: {err}")))?;
            if n == 0 {
                return Err(tongaku_playback::PlayerError::new(
                    "mpv socket closed".to_owned(),
                ));
            }
            let value: serde_json::Value = serde_json::from_str(&response)
                .map_err(|err| tongaku_playback::PlayerError::new(format!("ipc parse: {err}")))?;
            if value.get("request_id").and_then(|r| r.as_i64()) != Some(id) {
                continue;
            }
            if value.get("error").and_then(|e| e.as_str()) != Some("success") {
                return Err(tongaku_playback::PlayerError::new(format!(
                    "mpv: {response}"
                )));
            }
            return Ok(value);
        }
    }
}

struct Mpv {
    ipc: Ipc,
    _child: Child,
    socket_path: PathBuf,
}
pub struct MpvPlayer {
    mpv: Mutex<Option<Mpv>>,
}

impl Default for MpvPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl MpvPlayer {
    pub fn new() -> Self {
        Self {
            mpv: Mutex::new(None),
        }
    }

    fn spawn_mpv() -> Result<Mpv, tongaku_playback::PlayerError> {
        let socket_path =
            std::env::temp_dir().join(format!("tongaku-mpv-{}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket_path);

        let mut child = Command::new("mpv")
            .arg("--no-video")
            .arg("--really-quiet")
            .arg("--idle=yes")
            .arg(format!("--input-ipc-server={}", socket_path.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| {
                tongaku_playback::PlayerError::new(format!("failed to spawn mpv: {err}"))
            })?;

        // mpv creates the socket asynchronously after startup.
        let mut stream = None;
        for _ in 0..50 {
            if let Ok(s) = UnixStream::connect(&socket_path) {
                stream = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let stream = match stream {
            Some(stream) => stream,
            None => {
                let _ = child.kill();
                return Err(tongaku_playback::PlayerError::new(
                    "mpv socket never appeared - is mpv installed?".to_owned(),
                ));
            }
        };
        Ok(Mpv {
            ipc: Ipc {
                writer: stream.try_clone().unwrap(),
                reader: BufReader::new(stream),
                next_id: 0,
            },
            _child: child,
            socket_path,
        })
    }
}

impl Player for MpvPlayer {
    /// Play any media URL. Streams and decodes inside mpv.
    fn play(&self, url: &str) -> Result<(), PlayerError> {
        let mut slot = self.mpv.lock().unwrap();
        if slot.is_none() {
            *slot = Some(Self::spawn_mpv()?);
        }
        let mpv = slot.as_mut().unwrap();
        mpv.ipc
            .transact(json!(["loadfile", url, "replace"]))
            .map(|_| ())
    }

    fn toggle_pause(&self) {
        if let Some(mpv) = self.mpv.lock().unwrap().as_mut() {
            let _ = mpv.ipc.transact(json!(["cycle", "pause"]));
        }
    }

    fn stop(&self) {
        if let Some(mpv) = self.mpv.lock().unwrap().as_mut() {
            let _ = mpv.ipc.transact(json!(["stop"]));
        }
    }

    fn status(&self) -> PlaybackStatus {
        let mut slot = self.mpv.lock().unwrap();
        let Some(mpv) = slot.as_mut() else {
            return PlaybackStatus::default();
        };

        let position = mpv
            .ipc
            .transact(json!(["get_property", "time-pos"]))
            .ok()
            .and_then(|value| value.get("data").and_then(|d| d.as_f64()))
            .filter(|secs| secs.is_finite() && *secs >= 0.0)
            .map(Duration::from_secs_f64)
            .unwrap_or_default();

        let duration = mpv
            .ipc
            .transact(json!(["get_property", "duration"]))
            .ok()
            .and_then(|value| value.get("data").and_then(|d| d.as_f64()))
            .filter(|secs| secs.is_finite() && *secs >= 0.0)
            .map(Duration::from_secs_f64);

        PlaybackStatus::new(position, duration)
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        let _ = self._child.kill();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
