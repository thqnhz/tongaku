//! UI rendering for the player view.
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::button::*;
use gpui_kit::component::input::*;
use gpui_kit::component::slider::*;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::PlayerView;

impl PlayerView {
    fn status_text(&self) -> SharedString {
        if let Some(error) = &self.error {
            format!("Error: {error}").into()
        } else if let Some(track) = &self.track {
            format!("{} - {}", track.artist, track.title).into()
        } else {
            "Nothing playing - search a song".into()
        }
    }

    fn render_search(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .flex_1()
            .gap_2()
            .items_center()
            .justify_start()
            .pt_6()
            .child(Input::new(&self.input).w_64())
            .when(self.searching, |this| this.child("Searching..."))
            .child(
                div()
                    .v_flex()
                    .w_96()
                    .max_h(px(320.))
                    .overflow_hidden()
                    .children(self.results.iter().take(10).map(|track| {
                        let id = track.id.clone();
                        div()
                            .id(format!("search-result-{id}"))
                            .cursor_pointer()
                            .child(format!("{} - {}", track.artist, track.title))
                            .on_click(cx.listener(move |this, _, _, cx| this.play(id.clone(), cx)))
                    })),
            )
    }

    fn render_seekbar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dur_secs = self.status.duration.map(|d| d.as_secs_f32()).unwrap_or(1.0);
        let pos_frac = (self.status.position.as_secs_f32() / dur_secs.max(1.0)).clamp(0.0, 1.0);
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
    }

    fn render_controls(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .h_flex()
            .gap_2()
            .items_center()
            .child(
                Button::new("loop")
                    .rounded(ButtonRounded::Large)
                    .icon(Icon::new(IconName::Repeat))
                    .selected(self.looping)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_loop(cx))),
            )
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
            )
            .child(
                Button::new("shuffle")
                    .rounded(ButtonRounded::Large)
                    .icon(Icon::new(IconName::Shuffle))
                    .selected(self.shuffle)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_shuffle(cx))),
            )
    }
}

impl Render for PlayerView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .size_full()
            .child(self.render_search(window, cx))
            .child(
                div()
                    .v_flex()
                    .gap_2()
                    .items_center()
                    .border_t_1()
                    .child(self.status_text())
                    .child(self.render_seekbar(window, cx))
                    .child(self.render_controls(cx))
                    .py_3(),
            )
    }
}

fn fmt_time(time: Duration) -> String {
    let secs = time.as_secs();
    format!("{}:{:02}", secs / 60, secs % 60)
}
