use gpui::{
    Context, FocusHandle, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Modifiers, ParentElement,
    Render, Styled, TestAppContext, VisualTestContext, Window, div, point, px,
};

use super::{Button, SegmentedControl, ToggleButton, ToggleItem};
use crate::{
    primitives::{FocusNext, FocusScope},
    theme::Theme,
};

/// One button that counts its presses.
struct Bench {
    root: FocusHandle,
    loading: bool,
    presses: usize,
}

impl Render for Bench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        FocusScope::new(&self.root).root().size_full().child(
            Button::new("save", "Save")
                .loading(self.loading)
                .on_click(move |_, _, cx| view.update(cx, |bench, _| bench.presses += 1)),
        )
    }
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    cx.run_until_parked();
}

#[gpui::test]
fn a_loading_button_ignores_presses_and_keeps_its_tab_stop(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Bench {
        root: cx.focus_handle(),
        loading: true,
        presses: 0,
    });
    let root = view.read_with(cx, |bench, _| bench.root.clone());
    cx.update(|window, cx| window.focus(&root, cx));
    press("tab", cx);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |bench, _| bench.presses), 0);
    view.update(cx, |bench, cx| {
        bench.loading = false;
        cx.notify();
    });
    cx.run_until_parked();
    press("enter", cx);
    assert_eq!(
        view.read_with(cx, |bench, _| bench.presses),
        1,
        "focus stayed on it"
    );
}

/// One toggle that counts its presses.
struct Toggles {
    disabled: bool,
    presses: usize,
}

impl Render for Toggles {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        div().size_full().child(
            ToggleButton::new("bold", ToggleItem::new("bold").label("B"), false)
                .disabled(self.disabled)
                .on_toggle(move |_, _, cx| view.update(cx, |toggles, _| toggles.presses += 1)),
        )
    }
}

#[gpui::test]
fn a_disabled_toggle_ignores_clicks(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (view, cx) = cx.add_window_view(|_, _| Toggles {
        disabled: true,
        presses: 0,
    });
    cx.simulate_click(point(px(4.0), px(4.0)), Modifiers::none());
    assert_eq!(view.read_with(cx, |toggles, _| toggles.presses), 0);
    view.update(cx, |toggles, cx| {
        toggles.disabled = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_click(point(px(4.0), px(4.0)), Modifiers::none());
    assert_eq!(view.read_with(cx, |toggles, _| toggles.presses), 1);
}

/// Segments whose chosen value left with a change of list.
struct Stale;

impl Render for Stale {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(300.0)).child(
            SegmentedControl::new("views", "gone")
                .segment("list", "List", None)
                .segment("grid", "Grid", None),
        )
    }
}

#[gpui::test]
fn a_choice_missing_from_its_segments_marks_none(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Stale);
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    assert!(cx.debug_bounds("segment list").is_some());
    assert!(
        cx.debug_bounds("segment-thumb").is_none(),
        "no thumb marks a choice"
    );
}
