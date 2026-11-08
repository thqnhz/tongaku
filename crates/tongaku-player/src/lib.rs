//! Playback feature: now-playing view and its workflow.
use gpui_kit::component::button::*;
use gpui_kit::component::*;
use gpui_kit::*;

use tongaku_playback::Player;
use tongaku_ytmusic::{Client, Track, YtError};

pub struct PlayerView {
    player: Box<dyn Player>,
    track: Option<Track>,
    error: Option<String>,
    _fetch_task: Task<()>,
}

impl PlayerView {
    pub fn new(video_id: String, player: Box<dyn Player>, cx: &mut Context<Self>) -> Self {
        let client = Client::new();

        let _fetch_task = cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = cx
                .background_spawn(async move {
                    let track = client.get_track(&video_id)?;
                    let url = Client::stream_url(&track.id)?;
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

        Self {
            player,
            track: None,
            error: None,
            _fetch_task,
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

impl Render for PlayerView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .child(self.status_text())
            .child(
                Button::new("toggle")
                    .primary()
                    .label("Play / Pause")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.player.toggle_pause();
                        cx.notify();
                    })),
            )
    }
}
