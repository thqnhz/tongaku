//! Playback feature: now-playing view and its workflow.
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::button::ButtonRounded;
use gpui_kit::component::button::*;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::*;
use gpui_kit::component::{Icon, IconName};
use gpui_kit::*;

use tongaku_playback::{PlaybackStatus, Player};
use tongaku_ytmusic::{Client, Track, YtError};

pub struct PlayerView {
    player: Arc<dyn Player>,
    track: Option<Track>,
    status: PlaybackStatus,
    error: Option<String>,
    _fetch_task: Task<()>,
    _poll_task: Task<()>,
}

impl PlayerView {
    pub fn new(video_id: String, player: Arc<dyn Player>, cx: &mut Context<Self>) -> Self {
        let client = Client::new();

        let _fetch_task = cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = cx
                .background_spawn(async move {
                    let (track, stream) = client.get_track_with_stream(&video_id)?;
                    let url = match stream {
                        Some(stream) => stream.url,
                        None => Client::stream_url(&track.id)?,
                    };
                    Ok::<_, YtError>((track, url))
                })
                .await;

            this.update(cx, |view, cx| {
                match result {
                    Ok((track, url)) => {
                        if let Err(err) = view.player.play(&url) {
                            view.error = Some(err.to_string());
                        }
                        view.track = Some(track);
                    }
                    Err(err) => view.error = Some(err.to_string()),
                }
                cx.notify();
            })
            .ok();
        });

        let poll_player = player.clone();
        let _poll_task = cx.spawn(async move |this, cx: &mut AsyncApp| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(500))
                    .await;

                let player = poll_player.clone();
                let status = cx.background_spawn(async move { player.status() }).await;

                this.update(cx, |view, cx| {
                    if view.status != status {
                        view.status = status;
                        cx.notify();
                    }
                })
                .ok();
            }
        });

        Self {
            player,
            track: None,
            status: PlaybackStatus::default(),
            error: None,
            _fetch_task,
            _poll_task,
        }
    }

    fn status_text(&self) -> SharedString {
        if let Some(error) = &self.error {
            format!("Error: {error}").into()
        } else if let Some(track) = &self.track {
            format!("{} - {}", track.artist, track.title).into()
        } else {
            "Loading...".into()
        }
    }
}

fn fmt_time(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

impl Render for PlayerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child(self.status_text())
            .child({
                let percent = match self.status.duration {
                    Some(duration) if !duration.is_zero() => {
                        (self.status.position.as_secs_f32() / duration.as_secs_f32() * 100.)
                            .clamp(0., 100.)
                    }
                    _ => 0.,
                };
                div()
                    .v_flex()
                    .w_96()
                    .gap_1()
                    .child(Progress::new("track-progress").value(percent))
                    .child(
                        div()
                            .h_flex()
                            .w_full()
                            .justify_between()
                            .child(fmt_time(self.status.position))
                            .child(match self.status.duration {
                                Some(duration) => fmt_time(duration),
                                None => "--:--".to_owned(),
                            }),
                    )
            })
            .child(
                Button::new("toggle")
                    .primary()
                    .rounded(ButtonRounded::Large)
                    .size_12()
                    .icon(if self.status.paused {
                        Icon::new(IconName::Play)
                    } else {
                        Icon::new(IconName::Pause)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.player.toggle_pause();
                        cx.notify();
                    })),
            )
    }
}
