use std::path::PathBuf;

use gpui::{
    KeyBinding, KeyUpEvent, Keystroke, Pixels, Point, TestAppContext, VisualTestContext, point,
};

use crate::{primitives::FocusNext, theme::Theme};

mod audio;
mod devices;
mod edit;
mod images;
mod player;
mod stale;
mod video;

pub(super) fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    for _ in 0..4 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

pub(super) fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

/// A picture `w` by `h` written to a file named for its test, the same file each run.
pub(super) fn picture(name: &str, w: u32, h: u32) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ely-media-{name}.png"));
    image::RgbaImage::from_pixel(w, h, image::Rgba([90, 120, 150, 255]))
        .save(&path)
        .expect("the picture writes");
    path
}

/// Where a share of a box sits in the window, halfway down.
pub(super) fn across(
    selector: &'static str,
    share: f32,
    cx: &mut VisualTestContext,
) -> Point<Pixels> {
    let bounds = cx.debug_bounds(selector).expect("the box draws");
    point(bounds.left() + bounds.size.width * share, bounds.center().y)
}

/// Moves focus to the `nth` Tab stop from none.
pub(super) fn tab_to(nth: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        for _ in 0..nth {
            window.focus_next(cx);
        }
    });
    settle(cx);
}
