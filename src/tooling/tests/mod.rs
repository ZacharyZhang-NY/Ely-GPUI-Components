use gpui::{
    AnyElement, Context, FocusHandle, InteractiveElement, IntoElement, KeyUpEvent, Keystroke,
    ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, div, px,
};

use std::cell::Cell;

use crate::{
    forms,
    primitives::{FocusNext, FocusPrev, FocusScope},
    theme::Theme,
    tooling::{Knob, Playground},
};

mod catalogs;
mod inspection;

struct Stage {
    root: FocusHandle,
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.root)
            .root()
            .size_full()
            .child(div().w(px(600.0)).child(playground()))
    }
}

thread_local! {
    static LARGE: Cell<bool> = const { Cell::new(true) };
}

fn playground() -> AnyElement {
    Playground::new(
        "playground",
        [
            Knob::Toggle("disabled".into(), false),
            Knob::Choice(
                "size".into(),
                ["Sm", "Md", "Lg"][..2 + usize::from(LARGE.get())]
                    .iter()
                    .map(|size| (*size).into())
                    .collect(),
                1,
            ),
            Knob::Number("share".into(), 0.5, (0.0, 1.0), 0.25),
        ],
        |settings, _, _| {
            let shown = format!(
                "preview-{}-{}-{:.2}",
                settings.on("disabled"),
                settings.picked("size"),
                settings.number("share")
            );
            div().debug_selector(|| shown).into_any_element()
        },
    )
    .into_any_element()
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

/// Presses a key with no refresh of its own, so only what notifies redraws.
fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.run_until_parked();
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    cx.run_until_parked();
}

/// Each knob's edit redraws the preview with the new settings.
#[gpui::test]
fn knobs_redraw_the_preview(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([
            gpui::KeyBinding::new("tab", FocusNext, None),
            gpui::KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Stage {
        root: cx.focus_handle(),
    });
    let root = view.read_with(cx, |stage, _| stage.root.clone());
    cx.update(|window, cx| window.focus(&root, cx));
    settle(cx);
    let shown =
        |selector: &'static str, cx: &mut VisualTestContext| cx.debug_bounds(selector).is_some();
    assert!(shown("preview-false-Md-0.50", cx));
    press("tab", cx);
    press("space", cx);
    assert!(shown("preview-true-Md-0.50", cx), "the switch turned");
    press("tab", cx);
    for key in ["down", "down", "enter"] {
        press(key, cx);
    }
    assert!(shown("preview-true-Lg-0.50", cx), "the choice moved");
    press("tab", cx);
    press("right", cx);
    press("right", cx);
    assert!(
        shown("preview-true-Lg-1.00", cx),
        "the number stepped twice"
    );
    assert!(
        shown("knob-readout-share-1.00", cx),
        "its readout in the step's places"
    );
    press("shift-tab", cx);
    press("shift-tab", cx);
    press("space", cx);
    assert!(
        shown("preview-false-Lg-1.00", cx),
        "the switch showed its own setting, so a second press turned it back"
    );
    LARGE.set(false);
    settle(cx);
    press("tab", cx);
    press("tab", cx);
    press("right", cx);
    assert!(
        shown("preview-false-Md-0.75", cx),
        "the owner took away the picked size, so every setting started over"
    );
}
