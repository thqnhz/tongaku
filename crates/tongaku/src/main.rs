use std::sync::Arc;

use gpui_kit::assets::Assets;
use gpui_kit::*;

use tongaku_mpv::MpvPlayer;
use tongaku_player::PlayerView;

const DEFAULT_VIDEO_ID: &str = "dQw4w9WgXcQ";

fn main() {
    let video_id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_VIDEO_ID.to_owned());

    gpui_kit::application()
        .with_assets(Assets::new(""))
        .run(move |cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, move |_, cx| {
                cx.new(|cx| PlayerView::new(video_id.clone(), Arc::new(MpvPlayer::new()), cx))
            })
            .expect("Failed to open window");
        });
}
