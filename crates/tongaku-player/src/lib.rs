//! Playback feature: now-playing view and its workflow.
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::slider::*;
use gpui_kit::prelude::*;
use gpui_kit::*;

use tongaku_playback::{PlaybackStatus, Player};
use tongaku_ytmusic::{Client, Track, YtError};

use crate::queue::Queue;

mod fetch;
mod queue;
mod ui;

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
    looping: bool,
    shuffle: bool,
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
            looping: false,
            shuffle: false,
            dragging: false,
            searching: false,
            _search_task: None,
            _poll_task,
            _subscriptions,
        }
    }

    pub(crate) fn toggle_loop(&mut self, cx: &mut Context<Self>) {
        self.looping = !self.looping;
        cx.notify();
    }

    pub(crate) fn toggle_shuffle(&mut self, cx: &mut Context<Self>) {
        self.shuffle = !self.shuffle;
        cx.notify();
    }
}
