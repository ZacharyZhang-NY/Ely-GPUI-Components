use gpui::{
    Context, IntoElement, Modifiers, ParentElement, Render, ScrollDelta, ScrollEvent, ScrollSource,
    SharedString, Styled, TestAppContext, TouchPhase, VisualTestContext, Window, div, point, px,
};

use super::setup;
use crate::{
    forms::Choice,
    navigation::{EditorTab, EditorTabs, Tabs},
};

struct Strip {
    chosen: SharedString,
}

impl Render for Strip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let tabs = [
            Choice::new("a", "A"),
            Choice::new("b", "B").disabled(),
            Choice::new("c", "C"),
        ];
        Tabs::new("tabs", tabs, self.chosen.clone()).on_change(move |value, _, cx| {
            view.update(cx, |view, cx| {
                view.chosen = value.clone();
                cx.notify();
            })
        })
    }
}

#[gpui::test]
fn tab_arrows_skip_disabled_tabs_and_wrap(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Strip { chosen: "a".into() });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("right");
    assert_eq!(view.read_with(cx, |view, _| view.chosen.clone()), "c");
    cx.simulate_keystrokes("right");
    assert_eq!(view.read_with(cx, |view, _| view.chosen.clone()), "a");
}

struct Docs {
    chosen: SharedString,
    clicked: Option<SharedString>,
    closed: Option<SharedString>,
}

impl Render for Docs {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (picker, closer) = (cx.entity(), cx.entity());
        let tabs = (0..10).map(|ix| EditorTab::new(format!("t{ix}"), format!("document-{ix}.rs")));
        div().w(px(200.0)).child(
            EditorTabs::new("docs", tabs)
                .selected(self.chosen.clone())
                .on_select(move |id, _, cx| {
                    let id = id.clone();
                    picker.update(cx, |view, _| view.clicked = Some(id));
                })
                .on_close(move |id, _, cx| {
                    let id = id.clone();
                    closer.update(cx, |view, _| view.closed = Some(id));
                }),
        )
    }
}

/// The test platform never runs next-frame callbacks; a refresh stands in for the display link.
fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn click_at(x: f32, cx: &mut VisualTestContext) {
    let at = point(px(x), px(16.0));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_click(at, Modifiers::none());
}

#[gpui::test]
fn the_chosen_tab_scrolls_in_once_and_a_manual_scroll_stays(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Docs {
        chosen: "t9".into(),
        clicked: None,
        closed: None,
    });
    settle(cx);
    let strip_end = cx
        .update(|window, _| window.viewport_size().width)
        .min(px(200.0))
        - cx.update(|_, cx| {
            crate::theme::ActiveTheme::theme(cx)
                .control_height(crate::theme::ControlSize::Sm)
                .to_pixels(px(16.0))
        })
        - px(8.0);
    click_at(f32::from(strip_end) - 16.0, cx);
    let closed = view.read_with(cx, |view, _| view.closed.clone());
    assert_eq!(
        closed,
        Some(SharedString::from("t9")),
        "the chosen tab's close button sits at the strip's end"
    );
    view.update(cx, |view, cx| {
        view.chosen = "t8".into();
        cx.notify();
    });
    settle(cx);
    cx.simulate_event(ScrollEvent {
        position: point(px(60.0), px(16.0)),
        delta: ScrollDelta::Pixels(point(px(5000.0), px(0.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    view.update(cx, |_, cx| cx.notify());
    settle(cx);
    click_at(6.0, cx);
    let clicked = view.read_with(cx, |view, _| view.clicked.clone());
    assert_eq!(
        clicked,
        Some(SharedString::from("t0")),
        "a manual scroll stays put"
    );
}

struct Context_ {
    chosen: SharedString,
    asked: Option<SharedString>,
}

impl Render for Context_ {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let tabs = [Choice::new("a", "A"), Choice::new("c", "C")];
        Tabs::new("tabs", tabs, self.chosen.clone()).on_context(move |value, _, _, cx| {
            view.update(cx, |view, cx| {
                view.asked = Some(value.clone());
                cx.notify();
            })
        })
    }
}

#[gpui::test]
fn a_right_click_names_its_tab_and_keeps_the_choice(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Context_ {
        chosen: "c".into(),
        asked: None,
    });
    settle(cx);
    let at = point(px(12.0), px(16.0));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_mouse_down(at, gpui::MouseButton::Right, Modifiers::none());
    let (chosen, asked) = view.read_with(cx, |view, _| (view.chosen.clone(), view.asked.clone()));
    assert_eq!(
        asked.as_deref(),
        Some("a"),
        "the clicked tab, not the chosen one"
    );
    assert_eq!(chosen, "c");
}

#[gpui::test]
fn tabs_whose_choice_closed_mark_none_and_arrows_start_at_the_ends(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Strip {
        chosen: "gone".into(),
    });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("right");
    assert_eq!(view.read_with(cx, |view, _| view.chosen.clone()), "a");
    view.update(cx, |view, cx| {
        view.chosen = "gone".into();
        cx.notify();
    });
    cx.simulate_keystrokes("left");
    assert_eq!(view.read_with(cx, |view, _| view.chosen.clone()), "c");
}
