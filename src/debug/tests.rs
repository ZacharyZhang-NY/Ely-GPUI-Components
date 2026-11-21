use std::{cell::RefCell, rc::Rc};

use gpui::{
    Context, IntoElement, KeyUpEvent, Keystroke, Modifiers, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, point, px,
};

use super::{Breakpoint, BreakpointList, Disassembly, Instruction, Span, TimelineProfiler};
use crate::theme::Theme;

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Two plain instructions; it keeps the gutter presses.
struct Listing(Rc<RefCell<Vec<usize>>>);

impl Render for Listing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let store = self.0.clone();
        let plain = |address| Instruction {
            address,
            bytes: vec![0x90],
            mnemonic: "nop".into(),
            operands: "".into(),
            comment: None,
            source: None,
        };
        div().w(px(400.0)).h(px(200.0)).child(
            Disassembly::new("listing", vec![plain(0x10), plain(0x11)])
                .on_breakpoint(move |ix, _, _| store.borrow_mut().push(ix)),
        )
    }
}

#[gpui::test]
fn an_empty_gutter_takes_a_press_across_its_row(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let store = seen.clone();
    let (_, cx) = cx.add_window_view(|_, _| Listing(store));
    settle(cx);
    cx.simulate_click(point(px(10.0), px(2.0)), Modifiers::none());
    settle(cx);
    assert_eq!(*seen.borrow(), [0]);
}

/// One breakpoint; it keeps what its toggle and open report.
struct Points {
    toggles: Rc<RefCell<Vec<(usize, bool)>>>,
    opens: Rc<RefCell<Vec<usize>>>,
}

impl Render for Points {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (toggles, opens) = (self.toggles.clone(), self.opens.clone());
        let point = Breakpoint {
            path: "src/lift.rs".into(),
            line: 3,
            enabled: true,
            condition: None,
            hits: 0,
        };
        div().w(px(400.0)).child(
            BreakpointList::new("points", [point])
                .on_toggle(move |ix, on, _, _| toggles.borrow_mut().push((ix, on)))
                .on_open(move |ix, _, _| opens.borrow_mut().push(ix)),
        )
    }
}

#[gpui::test]
fn a_breakpoints_box_toggles_without_opening_it(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (toggles, opens) = (
        Rc::new(RefCell::new(Vec::new())),
        Rc::new(RefCell::new(Vec::new())),
    );
    let view = Points {
        toggles: toggles.clone(),
        opens: opens.clone(),
    };
    let (_, cx) = cx.add_window_view(|_, _| view);
    settle(cx);
    cx.simulate_click(point(px(14.0), px(12.0)), Modifiers::none());
    settle(cx);
    assert_eq!(*toggles.borrow(), [(0, false)]);
    assert!(
        opens.borrow().is_empty(),
        "the box press stays with the box"
    );
    cx.simulate_click(point(px(200.0), px(12.0)), Modifiers::none());
    settle(cx);
    assert_eq!(*opens.borrow(), [0], "the row still opens");
}

/// Two spans on one track; it keeps the spans picked.
struct Spans(Rc<RefCell<Vec<usize>>>);

impl Render for Spans {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let store = self.0.clone();
        let span = |name: &str, start, end| Span {
            track: 0,
            name: name.to_string().into(),
            start,
            end,
        };
        let spans = vec![span("a", 0.0, 4.0), span("b", 5.0, 9.0)];
        div().w(px(400.0)).child(
            TimelineProfiler::new("spans", ["Main"], spans, (0.0, 10.0))
                .on_select(move |ix, _, _| store.borrow_mut().push(ix)),
        )
    }
}

#[gpui::test]
fn tab_reaches_each_span_and_enter_picks_it(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let picked = Rc::new(RefCell::new(Vec::new()));
    let store = picked.clone();
    let (_, cx) = cx.add_window_view(|_, _| Spans(store));
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("enter");
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").expect("a key"),
    });
    settle(cx);
    assert_eq!(
        *picked.borrow(),
        [1],
        "the second Tab stands on the second span"
    );
}

/// A stack whose current frame left with a shorter stack.
struct Unwound;

impl Render for Unwound {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let frame = super::StackFrame {
            function: "main".into(),
            path: "src/main.rs".into(),
            line: 3,
            library: false,
        };
        div()
            .w(px(400.0))
            .child(super::CallStack::new("stack", [frame]).current(4))
    }
}

#[gpui::test]
fn a_current_frame_past_the_stack_marks_none(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Unwound);
    settle(cx);
}

/// A flame graph focused on a call its new profile lacks; a dump ending at the last address.
struct Edges(bool);

impl Render for Edges {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let root = super::ProfileFrame {
            name: "root".into(),
            own: 1,
            calls: vec![],
        };
        div().w(px(400.0)).h(px(100.0)).child(match self.0 {
            true => super::Flamegraph::new("flame", root)
                .focus(vec![0])
                .into_any_element(),
            false => super::HexViewer::new("hex", vec![0x42])
                .base(u64::MAX)
                .into_any_element(),
        })
    }
}

#[gpui::test]
fn a_lost_focus_and_the_last_address_draw(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    for flame in [true, false] {
        let (_, cx) = cx.add_window_view(move |_, _| Edges(flame));
        settle(cx);
    }
}

/// A timeline whose owner keeps each range it reports.
struct Zooming((f64, f64));

impl Render for Zooming {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        div().w(px(400.0)).child(
            TimelineProfiler::new("timeline", ["Main"], vec![], self.0).on_range(
                move |range, _, cx| {
                    owner.update(cx, |owner, cx| {
                        owner.0 = range;
                        cx.notify();
                    })
                },
            ),
        )
    }
}

#[gpui::test]
fn a_zoom_past_the_finest_range_stays_put(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let start = (1.0, 1.0 + f64::EPSILON);
    let (owner, cx) = cx.add_window_view(move |_, _| Zooming(start));
    settle(cx);
    for _ in 0..2 {
        cx.simulate_event(gpui::ScrollEvent {
            position: point(px(250.0), px(30.0)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(400.0))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            source: gpui::ScrollSource::Pointer,
        });
        settle(cx);
    }
    let range = owner.read_with(cx, |owner, _| owner.0);
    assert!(range.1 > range.0, "{range:?}");
}

#[gpui::test]
fn a_wheel_too_large_for_a_zoom_stays_put(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (owner, cx) = cx.add_window_view(|_, _| Zooming((0.0, 25.0)));
    settle(cx);
    for pixels in [300_000.0, -300_000.0] {
        cx.simulate_event(gpui::ScrollEvent {
            position: point(px(250.0), px(30.0)),
            delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(pixels))),
            modifiers: Modifiers::none(),
            touch_phase: gpui::TouchPhase::Moved,
            source: gpui::ScrollSource::Pointer,
        });
        settle(cx);
    }
    assert_eq!(owner.read_with(cx, |owner, _| owner.0), (0.0, 25.0));
}
