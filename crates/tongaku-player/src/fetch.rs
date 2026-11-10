//! Fetch workflow: search, play resolution, prefetch, queue advance.
use std::time::Duration;

use gpui_kit::*;

use super::{Client, PlayerView, YtError};

impl PlayerView {
    pub(crate) fn run_search(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn play(&mut self, video_id: String, cx: &mut Context<Self>) {
        if !self.queue.select(&video_id)
            && let Some(track) = self.results.iter().find(|t| t.id == video_id).cloned()
        {
            self.queue.push(track);
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
    pub(crate) fn next_track(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.queue.next_id(self.looping, self.shuffle) {
            self.play(id, cx);
        }
    }

    /// Go back to previous queued track.
    pub(crate) fn prev_track(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.queue.prev_id() {
            self.play(id, cx);
        }
    }

    /// If current track ended, advance the queue.
    pub(crate) fn check_end(&mut self, cx: &mut Context<Self>) {
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

    pub(crate) fn status_text(&self) -> SharedString {
        if let Some(error) = &self.error {
            format!("Error: {error}").into()
        } else if let Some(track) = &self.track {
            format!("{} - {}", track.artist, track.title).into()
        } else {
            "Nothing playing - search a song".into()
        }
    }
}
