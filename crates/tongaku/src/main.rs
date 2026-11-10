use std::sync::Arc;

use gpui_kit::assets::AllAssets;
use gpui_kit::*;

use tongaku_mpv::MpvPlayer;
use tongaku_player::PlayerView;

fn main() {
    gpui_kit::application()
        .with_assets(AllAssets::new(""))
        .run(|cx| {
            gpui_kit::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| PlayerView::new(Arc::new(MpvPlayer::new()), window, cx))
            })
            .expect("Failed to open window");
        });
}
