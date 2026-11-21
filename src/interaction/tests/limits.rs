use gpui::{
    AnyElement, Axis, Entity, IntoElement, Modifiers, MouseButton, ParentElement, Styled,
    TestAppContext, div, point, px, size,
};

use super::{Bench, at, bench, picked, settle, tab, tap};
use crate::interaction::{Resizable, ScrollSync};

/// A box whose greatest width drops to 205 once the angle turns.
fn narrowing(bench: &Bench, _: Entity<Bench>) -> AnyElement {
    let max_width = if bench.angle == 0.0 { 300.0 } else { 205.0 };
    Resizable::new(
        "box",
        bench.size,
        size(px(100.0), px(80.0)),
        size(px(max_width), px(200.0)),
        |_, _, _| {},
    )
    .child(div().size_full())
    .into_any_element()
}

fn width(cx: &mut gpui::VisualTestContext) -> gpui::Pixels {
    cx.debug_bounds("resizable-box")
        .expect("the box")
        .size
        .width
}

#[gpui::test]
fn a_local_size_fits_limits_that_shrank(cx: &mut TestAppContext) {
    let (host, cx) = bench(narrowing, cx);
    tab(1, cx);
    tap("right", cx);
    assert_eq!(width(cx), px(208.0));
    host.update(cx, |bench, cx| {
        bench.angle = 1.0;
        cx.notify();
    });
    settle(cx);
    assert_eq!(width(cx), px(205.0));
}

#[gpui::test]
fn a_new_owner_size_past_the_limits_replaces_a_local_one(cx: &mut TestAppContext) {
    let (host, cx) = bench(narrowing, cx);
    host.update(cx, |bench, cx| {
        bench.size.width = px(400.0);
        cx.notify();
    });
    settle(cx);
    assert_eq!(width(cx), px(300.0));
    tab(1, cx);
    tap("left", cx);
    assert_eq!(width(cx), px(292.0));
    host.update(cx, |bench, cx| {
        bench.size.width = px(500.0);
        cx.notify();
    });
    settle(cx);
    assert_eq!(width(cx), px(300.0));
}

/// A box whose greatest height drops to 100 once the angle turns, and the heights it reports.
fn lowering(bench: &Bench, owner: Entity<Bench>) -> AnyElement {
    let max_height = if bench.angle == 0.0 { 200.0 } else { 100.0 };
    Resizable::new(
        "box",
        bench.size,
        size(px(100.0), px(80.0)),
        size(px(300.0), px(max_height)),
        move |next, _, cx| {
            owner.update(cx, |bench, cx| {
                bench.size = next;
                bench
                    .selected
                    .push(format!("{}", f32::from(next.height)).into());
                cx.notify();
            })
        },
    )
    .child(div().size_full())
    .into_any_element()
}

#[gpui::test]
fn an_edge_drag_reports_both_measures_within_the_limits_now(cx: &mut TestAppContext) {
    let (host, cx) = bench(lowering, cx);
    let none = Modifiers::none();
    cx.simulate_mouse_down(at(217.0, 60.0), MouseButton::Left, none);
    for x in [230.0, 240.0] {
        cx.simulate_mouse_move(at(x, 60.0), Some(MouseButton::Left), none);
        settle(cx);
    }
    assert!(!picked(&host, cx).is_empty(), "the drag began");
    host.update(cx, |bench, cx| {
        bench.angle = 1.0;
        bench.selected.clear();
        cx.notify();
    });
    settle(cx);
    cx.simulate_mouse_move(at(250.0, 60.0), Some(MouseButton::Left), none);
    settle(cx);
    assert_eq!(picked(&host, cx).last().map(String::as_str), Some("100"));
}

/// Panes that drop to two once the angle turns.
fn synced(bench: &Bench, _: Entity<Bench>) -> AnyElement {
    let count = if bench.angle == 0.0 { 3 } else { 2 };
    (0..count)
        .fold(
            ScrollSync::new("sync", Axis::Vertical)
                .w(px(300.0))
                .h(px(200.0)),
            |sync, _| {
                sync.pane(
                    div()
                        .flex()
                        .flex_col()
                        .children((0..40).map(|_| div().h(px(30.0)))),
                )
            },
        )
        .into_any_element()
}

#[gpui::test]
fn a_leader_pane_that_left_syncs_nothing(cx: &mut TestAppContext) {
    let (host, cx) = bench(synced, cx);
    cx.simulate_event(gpui::ScrollEvent {
        position: at(270.0, 100.0),
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-300.0))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        source: gpui::ScrollSource::Pointer,
    });
    settle(cx);
    host.update(cx, |bench, cx| {
        bench.angle = 1.0;
        cx.notify();
    });
    settle(cx);
}
