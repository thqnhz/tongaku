//! Playback feature: now-playing view and its workflow.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::button::ButtonRounded;
use gpui_kit::component::button::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;

use tongaku_playback::{PlaybackStatus, Player};
use tongaku_ytmusic::{Client, Track, YtError};

use crate::queue::Queue;

mod queue;

pub struct PlayerView {
    player: Arc<dyn Player>,
    track: Option<Track>,
    status: PlaybackStatus,
    error: Option<String>,
    input: Entity<InputState>,
    results: Vec<Track>,
    resolved: HashMap<String, String>,
    queue: queue::Queue,
    searching: bool,
    _search_task: Option<Task<()>>,
    _poll_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl PlayerView {
    pub fn new(player: Arc<dyn Player>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let _subscriptions =
            vec![
                cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        this.run_search(cx)
                    }
                }),
            ];

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
                    view.check_end(cx);
                })
                .ok();
            }
        });

        Self {
            player,
            track: None,
            status: PlaybackStatus::default(),
            error: None,
            input,
            results: Vec::new(),
            resolved: HashMap::new(),
            queue: Queue::new(),
            searching: false,
            _search_task: None,
            _poll_task,
            _subscriptions,
        }
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().to_string();
        if query.trim().is_empty() {
            return;
        }
        self.searching = true;
        self.error = None;
        cx.notify();

        let client = Client::new();
        self._search_task = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
            let result = cx
                .background_spawn(async move { client.search(&query) })
                .await;

            this.update(cx, |view, cx| {
                view.searching = false;
                match result {
                    Ok(results) => view.results = results,
                    Err(err) => view.error = Some(err.to_string()),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn play(&mut self, video_id: String, cx: &mut Context<Self>) {
        if !self.queue.select(&video_id) {
            if let Some(track) = self.results.iter().find(|t| t.id == video_id).cloned() {
                self.queue.push(track);
            }
        }

        if let Some(url) = self.resolved.get(&video_id).cloned() {
            self.error = None;
            self.track = self.results.iter().find(|t| t.id == video_id).cloned();
            self.player.play(&url).ok();
            cx.notify();
            return;
        }

        self.error = None;
        self.track = None;
        cx.notify();

        cx.spawn(async move |this, cx: &mut AsyncApp| {
            let video_id_clone = video_id.clone();
            let client_clone = Client::new();
            let result = cx
                .background_spawn(async move {
                    let track = client_clone.get_track(&video_id_clone)?;
                    Ok::<_, YtError>(track)
                })
                .await;

            let url_result = cx
                .background_spawn(async move { Client::stream_url(&video_id) })
                .await;

            let result = match (result, url_result) {
                (Ok(track), Ok(url)) => Ok((track, url)),
                (Err(err), _) => Err(err),
                (_, Err(err)) => Err(err),
            };

            this.update(cx, |view, cx| {
                match result {
                    Ok((track, url)) => {
                        if let Err(err) = view.player.play(&url) {
                            view.error = Some(err.to_string());
                        } else {
                            view.resolved.insert(track.id.clone(), url);
                            let next = view.queue.upcoming_ids(3);
                            view.prefetch(next, cx);
                        }
                        view.track = Some(track);
                    }
                    Err(err) => view.error = Some(err.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Advance to next queued track.
    fn next_track(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.queue.next_id() {
            self.play(id, cx);
        }
    }

    /// Go back to previous queued track.
    fn prev_track(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.queue.prev_id() {
            self.play(id, cx);
        }
    }

    /// If current track ended, advance the queue.
    fn check_end(&mut self, cx: &mut Context<Self>) {
        let Some(duration) = self.status.duration else {
            return;
        };
        if duration.is_zero() || self.status.paused || self.status.position.is_zero() {
            return;
        }
        let reached_end = self.status.position + Duration::from_millis(300) >= duration;
        if reached_end {
            self.next_track(cx);
        }
    }

    fn prefetch(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        let pending: Vec<String> = ids
            .into_iter()
            .filter(|id| !self.resolved.contains_key(id))
            .take(3)
            .collect();
        for id in pending {
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                let client_id = id.clone();
                let url = cx
                    .background_spawn(async move { Client::stream_url(&client_id) })
                    .await;
                if let Ok(url) = url {
                    this.update(cx, |view, _| {
                        view.resolved.insert(id.clone(), url);
                    })
                    .ok();
                }
            })
            .detach();
        }
    }

    fn status_text(&self) -> SharedString {
        if let Some(error) = &self.error {
            format!("Error: {error}").into()
        } else if let Some(track) = &self.track {
            format!("{} - {}", track.artist, track.title).into()
        } else {
            "Nothing playing - search a song".into()
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
            .child(Input::new(&self.input).w_64())
            .when(self.searching, |this| this.child("Searching..."))
            .child(
                div()
                    .v_flex()
                    .w_96()
                    .children(self.results.iter().take(10).map(|track| {
                        let id = track.id.clone();
                        div()
                            .id(format!("search-result-{id}"))
                            .cursor_pointer()
                            .child(format!("{} - {}", track.artist, track.title))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.play(id.clone(), cx);
                            }))
                    })),
            )
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
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("prev")
                            .rounded(ButtonRounded::Large)
                            .icon(Icon::new(IconName::SkipBack))
                            .on_click(cx.listener(|this, _, _, cx| this.prev_track(cx))),
                    )
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
                    .child(
                        Button::new("next")
                            .rounded(ButtonRounded::Large)
                            .icon(Icon::new(IconName::SkipForward))
                            .on_click(cx.listener(|this, _, _, cx| this.next_track(cx))),
                    ),
            )
    }
}
