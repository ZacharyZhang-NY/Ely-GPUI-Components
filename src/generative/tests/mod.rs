use gpui::{KeyBinding, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext};

use crate::{forms, primitives::FocusNext, theme::Theme};

mod canvas;
mod data;
mod models;
mod results;
mod settings;
mod sound;
mod stale;

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Presses and releases `key`, a frame apart.
fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

fn tab(times: usize, cx: &mut VisualTestContext) {
    for _ in 0..times {
        cx.update(|window, cx| window.focus_next(cx));
    }
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
}
