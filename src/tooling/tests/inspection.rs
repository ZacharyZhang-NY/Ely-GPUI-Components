use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyBinding, KeyUpEvent, Keystroke, Modifiers, ParentElement, Render, ScrollDelta, ScrollEvent,
    ScrollSource, Styled, TestAppContext, TouchPhase, VisualTestContext, Window, div, point, px,
};

use crate::{
    forms::{self, TextInput},
    primitives::{FocusNext, FocusPrev, FocusScope},
    theme::Theme,
    tooling::{EventLogger, FpsMeter, RenderCounter, install_inspector},
};

fn shown(selector: &'static str, cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds(selector).is_some()
}

fn redraw(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

struct Sample;

impl Render for Sample {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            div()
                .id("sample")
                .mt(px(4.0))
                .ml(px(4.0))
                .p(px(8.0))
                .border_2()
                .w(px(120.0))
                .h(px(40.0))
                .child("Sample"),
        )
    }
}

/// Hovering picks a box and the panel gives its size and layers; a press holds it, and Pick picks again.
#[gpui::test]
fn the_inspector_shows_the_picked_box(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        install_inspector(cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| Sample);
    cx.update(|window, cx| window.toggle_inspector(cx));
    cx.run_until_parked();
    assert!(shown("inspector-picking-true", cx));
    cx.simulate_mouse_move(point(px(60.0), px(24.0)), None, Modifiers::none());
    assert!(shown("inspector-size-120x40", cx), "the hovered box");
    assert!(shown("inspector-margin-4-0-0-4", cx));
    assert!(shown("inspector-border-2-2-2-2", cx));
    assert!(shown("inspector-padding-8-8-8-8", cx));
    assert!(
        shown("inspector-content-100x20", cx),
        "less border and padding"
    );
    cx.simulate_click(point(px(60.0), px(24.0)), Modifiers::none());
    // gpui holds a pick without a redraw; the panel's own frame requests do not run in tests.
    redraw(cx);
    assert!(shown("inspector-picking-false", cx), "a press holds it");
    let pick = cx
        .debug_bounds("inspector-pick")
        .expect("Pick shows once held");
    cx.simulate_click(pick.center(), Modifiers::none());
    assert!(
        cx.update(|window, cx| window.is_inspector_picking(cx)),
        "Pick picks again"
    );
}

struct Stops {
    root: FocusHandle,
    first: FocusHandle,
    second: FocusHandle,
}

impl Render for Stops {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.root)
            .root()
            .size_full()
            .child(div().track_focus(&self.first).size(px(40.0)))
            .child(div().track_focus(&self.second).size(px(40.0)))
    }
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.run_until_parked();
}

/// Tab reaches the panel's Pick and goes on from it, both ways.
#[gpui::test]
fn tab_passes_through_the_panel(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        install_inspector(cx);
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Stops {
        root: cx.focus_handle(),
        first: cx.focus_handle().tab_stop(true),
        second: cx.focus_handle().tab_stop(true),
    });
    let (root, first, second) = view.read_with(cx, |stops, _| {
        (
            stops.root.clone(),
            stops.first.clone(),
            stops.second.clone(),
        )
    });
    cx.update(|window, cx| {
        window.focus(&root, cx);
        window.toggle_inspector(cx);
    });
    cx.run_until_parked();
    cx.simulate_click(point(px(20.0), px(20.0)), Modifiers::none());
    redraw(cx);
    assert!(shown("inspector-pick", cx), "a press holds the box");
    let focused = |cx: &mut VisualTestContext| cx.update(|window, cx| window.focused(cx));
    for _ in 0..3 {
        press("tab", cx);
    }
    let pick = focused(cx).expect("Pick holds focus");
    assert!(
        ![&root, &first, &second].contains(&&pick),
        "Tab reached Pick"
    );
    press("tab", cx);
    assert_eq!(focused(cx), Some(first.clone()), "Tab went on from Pick");
    press("shift-tab", cx);
    assert_eq!(focused(cx), Some(pick), "Shift-Tab came back");
    press("shift-tab", cx);
    assert_eq!(focused(cx), Some(second));
}

struct Meter;

impl Render for Meter {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(FpsMeter::new("meter"))
    }
}

/// The meter counts a frame per draw on the executor's clock, and names the worst.
#[gpui::test]
fn the_meter_counts_frames(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Meter);
    for _ in 0..60 {
        cx.executor().advance_clock(Duration::from_millis(20));
        redraw(cx);
    }
    assert!(shown("fps-50", cx), "a frame each 20 ms");
    assert!(shown("fps-worst-20.0", cx));
    cx.executor().advance_clock(Duration::from_millis(100));
    redraw(cx);
    assert!(shown("fps-worst-100.0", cx), "one slow frame");
}

struct Counted;

impl Render for Counted {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(RenderCounter::new("counter", "Page"))
    }
}

/// Each render of the view that draws it counts once.
#[gpui::test]
fn the_counter_counts_renders(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Counted);
    cx.run_until_parked();
    assert!(shown("renders-Page-1", cx), "the first frame");
    redraw(cx);
    redraw(cx);
    assert!(shown("renders-Page-3", cx));
}

struct Logged {
    root: FocusHandle,
    target: FocusHandle,
    field: Entity<TextInput>,
}

impl Render for Logged {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.root)
            .root()
            .size_full()
            .p(px(10.0))
            .child(
                EventLogger::new("logger")
                    .child(
                        div()
                            .id("target")
                            .track_focus(&self.target)
                            .h(px(100.0))
                            .capture_any_mouse_down(|_, _, cx| cx.stop_propagation()),
                    )
                    .child(self.field.clone()),
            )
    }
}

/// Pointer events inside the box, even ones a child stops as they arrive, and keys while focus is inside, bound ones too, list newest first with what they carried, placed in the box.
#[gpui::test]
fn the_logger_lists_events(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(|window, cx| Logged {
        root: cx.focus_handle(),
        target: cx.focus_handle(),
        field: cx.new(|cx| TextInput::new(window, cx)),
    });
    let target = view.read_with(cx, |logged, _| logged.target.clone());
    cx.update(|window, cx| window.focus(&target, cx));
    cx.run_until_parked();
    assert!(shown("event-logger-empty", cx));
    cx.simulate_mouse_move(point(px(20.0), px(20.0)), None, Modifiers::none());
    assert!(shown("event-move to 10, 10", cx));
    cx.simulate_mouse_move(point(px(30.0), px(40.0)), None, Modifiers::none());
    assert!(shown("event-move to 20, 30 ×2", cx), "moves run together");
    cx.simulate_click(point(px(30.0), px(40.0)), Modifiers::none());
    let pressed = cx
        .debug_bounds("event-press left at 20, 30")
        .expect("a press the child stops");
    let released = cx
        .debug_bounds("event-release left at 20, 30")
        .expect("its release");
    assert!(released.top() < pressed.top(), "newest first");
    cx.simulate_event(ScrollEvent {
        position: point(px(30.0), px(40.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-24.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    assert!(shown("event-wheel 0, -24 px", cx));
    cx.simulate_keystrokes("a");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("a").expect("a key"),
    });
    assert!(shown("event-key down a", cx));
    assert!(shown("event-key up a", cx));
    cx.simulate_modifiers_change(Modifiers::shift());
    assert!(shown("event-modifiers shift", cx));
    cx.simulate_modifiers_change(Modifiers::none());
    assert!(shown("event-modifiers none", cx));
    assert!(
        !shown("event-key down shift", cx),
        "a lone modifier is no key"
    );
    let (field, root) = view.read_with(cx, |logged, cx| {
        (logged.field.focus_handle(cx), logged.root.clone())
    });
    cx.update(|window, cx| window.focus(&field, cx));
    for key in ["b", "c", "backspace", "left", "enter"] {
        press(key, cx);
    }
    let text = view.read_with(cx, |logged, cx| logged.field.read(cx).text().to_string());
    assert_eq!(text, "b", "the field took Backspace");
    for row in [
        "event-key down backspace",
        "event-key down left",
        "event-key down enter",
    ] {
        assert!(shown(row, cx), "{row}: a key the field's bindings take");
    }
    cx.update(|window, cx| window.focus(&root, cx));
    press("z", cx);
    assert!(
        !shown("event-key down z", cx),
        "a key while focus is outside"
    );
    cx.simulate_click(point(px(30.0), px(410.0)), Modifiers::none());
    assert!(
        !shown("event-press left at 20, 400", cx),
        "a press outside the box"
    );
}
