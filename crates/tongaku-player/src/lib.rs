//! Playback feature: now-playing view and its workflow.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::button::ButtonRounded;
use gpui_kit::component::button::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::slider::*;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;

use tongaku_playback::{PlaybackStatus, Player};
use tongaku_ytmusic::{Client, Track, YtError};

use crate::queue::Queue;

mod fetch;
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
    seek_state: Entity<SliderState>,
    dragging: bool,
    searching: bool,
    _search_task: Option<Task<()>>,
    _poll_task: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl PlayerView {
    pub fn new(player: Arc<dyn Player>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx));
        let seek_state = cx.new(|_| SliderState::new().min(0.0).max(1.0).step(0.001));
        let _subscriptions = vec![
            cx.subscribe(&input, |this: &mut Self, _, event: &InputEvent, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.run_search(cx)
                }
            }),
            cx.subscribe(
                &seek_state,
                |this: &mut Self, _, event: &SliderEvent, cx| match event {
                    SliderEvent::Change(_) => {
                        this.dragging = true;
                        cx.notify();
                    }
                    SliderEvent::Release(SliderValue::Single(frac)) => {
                        this.dragging = false;
                        if let Some(dur) = this.status.duration {
                            this.player
                                .seek(Duration::from_secs_f32(frac * dur.as_secs_f32()));
                        }
                        cx.notify();
                    }
                    _ => {}
                },
            ),
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
            seek_state,
            dragging: false,
            searching: false,
            _search_task: None,
            _poll_task,
            _subscriptions,
        }
    }
}

fn fmt_time(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                let dur_secs = self.status.duration.map(|d| d.as_secs_f32()).unwrap_or(1.0);
                let pos_frac =
                    (self.status.position.as_secs_f32() / dur_secs.max(1.0)).clamp(0.0, 1.0);
                self.seek_state.update(cx, |state, cx| {
                    if !self.dragging {
                        state.set_value(SliderValue::Single(pos_frac), window, cx);
                    }
                });
                div()
                    .v_flex()
                    .w_96()
                    .gap_1()
                    .child(Slider::new(&self.seek_state).horizontal())
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
