use std::time::Duration;

use gpui::{
    AnyElement, App, AppContext, Axis, Bounds, Context, InteractiveElement, IntoElement, Modifiers,
    MouseButton, ParentElement, Pixels, Point, Render, StatefulInteractiveElement, Styled,
    TestAppContext, VisualTestContext, Window, div, point, px,
};

use super::{Drawer, PaneGroup, SplitPane};
use crate::theme::Theme;

type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One element in a column that a padded block measures by content.
struct Narrow(Build);

impl Render for Narrow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(280.0)).flex().flex_col().child(
            div().p_5().child(
                div()
                    .flex()
                    .flex_col()
                    .child((self.0)(window, cx))
                    .child("A line long enough to wrap in a narrow card, so the column fills it."),
            ),
        )
    }
}

/// The width of `selector` in that column; a root that fills it takes all 240px.
pub(crate) fn narrow_width(
    cx: &mut TestAppContext,
    selector: &'static str,
    build: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
) -> Pixels {
    let (_, cx) = cx.add_window_view(|_, _| Narrow(Box::new(build)));
    cx.run_until_parked();
    cx.debug_bounds(selector).expect(selector).size.width
}

/// A drawer holding a body the test can find.
struct Pulled;

impl Render for Pulled {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            Drawer::new("drawer", |_, _| {})
                .child(div().debug_selector(|| "drawer-body".into()).h(px(120.0))),
        )
    }
}

fn body(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.debug_bounds("drawer-body").expect("the body")
}

/// Opens a drawer and waits out its entrance.
fn opened(cx: &mut TestAppContext, reduced: bool) -> &mut VisualTestContext {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = reduced);
    });
    let (_, cx) = cx.add_window_view(|_, _| Pulled);
    cx.run_until_parked();
    cx.executor()
        .advance_clock(Duration::from_millis(if reduced { 10 } else { 250 }));
    cx.update(|window, _| window.refresh());
    cx
}

/// Presses the grab and drags it down by `dy`.
fn pull(cx: &mut VisualTestContext, dy: f32) -> Point<Pixels> {
    let rest = body(cx).origin;
    let grab = point(rest.x + px(10.0), rest.y - px(28.0));
    cx.simulate_mouse_down(grab, MouseButton::Left, Modifiers::none());
    for step in [4.0, 8.0, dy] {
        let at = point(grab.x, grab.y + px(step));
        cx.simulate_mouse_move(at, MouseButton::Left, Modifiers::none());
    }
    point(grab.x, grab.y + px(dy))
}

#[gpui::test]
fn a_resting_drawer_reaches_the_bottom(cx: &mut TestAppContext) {
    let cx = opened(cx, true);
    let panel = cx.debug_bounds("drawer-panel").expect("the panel");
    let bottom = cx.update(|window, _| window.viewport_size().height);
    assert_eq!(
        body(cx).size.height,
        px(120.0),
        "the panel's 0.85 cap leaves a resting body whole"
    );
    assert_eq!(
        panel.bottom(),
        bottom,
        "the panel reaches the window's bottom"
    );
}

#[gpui::test]
fn a_pulled_drawer_follows_the_pointer(cx: &mut TestAppContext) {
    let cx = opened(cx, true);
    let at = pull(cx, 8.0);
    let held = body(cx).origin.y;
    cx.simulate_mouse_move(
        at + point(px(0.0), px(40.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    assert_eq!(
        body(cx).origin.y - held,
        px(40.0),
        "the panel moves with the pull"
    );
}

#[gpui::test]
fn a_released_drawer_glides_back(cx: &mut TestAppContext) {
    let cx = opened(cx, false);
    let rest = body(cx).origin.y;
    let at = pull(cx, 40.0);
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    let lift = body(cx).origin.y - rest;
    assert!(
        lift > px(20.0),
        "the panel glides back from {lift:?} below rest"
    );
}

#[gpui::test]
fn a_pane_group_leaves_a_gone_or_last_pane_as_it_is(cx: &mut TestAppContext) {
    let group = cx.new(|_| PaneGroup::new(|_, _, _| div().into_any_element()));
    group.update(cx, |group, cx| {
        assert_eq!(group.split(9, Axis::Horizontal, cx), None);
        group.close(1, cx);
        assert_eq!(group.panes(), [1], "the last pane stays");
        let second = group.split(1, Axis::Horizontal, cx).expect("pane 1 splits");
        group.close(second, cx);
        group.close(second, cx);
        group.focus(second, cx);
        assert_eq!((group.panes(), group.focused()), (vec![1], 1));
    });
}

/// Three panes given two sizes.
struct Lagging;

impl Render for Lagging {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pane = |name: &'static str| div().debug_selector(move || name.into()).size_full();
        div().w(px(300.0)).h(px(100.0)).child(
            SplitPane::new("lagging", Axis::Horizontal, px(10.0))
                .sizes(&[1.0, 3.0])
                .pane(pane("first"))
                .pane(pane("second"))
                .pane(pane("third")),
        )
    }
}

#[gpui::test]
fn sizes_that_lag_their_panes_split_evenly(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Lagging);
    cx.run_until_parked();
    let widths =
        ["first", "second", "third"].map(|name| cx.debug_bounds(name).expect(name).size.width);
    assert_eq!(widths[0], widths[2]);
    assert_eq!(widths[1], widths[2]);
}

/// Two panes that may grow a third, with sizes that may turn invalid.
struct Changing {
    grown: bool,
    invalid: bool,
}

impl Render for Changing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pane = |name: &'static str| div().debug_selector(move || name.into()).size_full();
        let sizes = if self.invalid { [0.0, 0.0] } else { [1.0, 1.0] };
        let mut split = SplitPane::new("changing", Axis::Horizontal, px(10.0))
            .sizes(&sizes)
            .pane(pane("first"))
            .pane(pane("second"));
        if self.grown {
            split = split.pane(pane("third"));
        }
        div().w(px(300.0)).h(px(100.0)).child(split)
    }
}

fn drag_divider(cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.run_until_parked();
    let first = cx.debug_bounds("first").expect("first");
    let second = cx.debug_bounds("second").expect("second");
    let grab = point((first.right() + second.left()) / 2.0, px(50.0));
    cx.simulate_mouse_move(grab, None, Modifiers::none());
    cx.simulate_mouse_down(grab, MouseButton::Left, Modifiers::none());
    for step in [8.0, 8.0, 16.0] {
        cx.simulate_mouse_move(
            grab + point(px(step), px(0.0)),
            MouseButton::Left,
            Modifiers::none(),
        );
        cx.run_until_parked();
    }
    let moved = cx.debug_bounds("first").expect("first").size.width;
    assert!(moved > first.size.width, "the drag moved the divider");
    grab
}

#[gpui::test]
fn a_pane_added_mid_drag_drops_the_drag(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (host, cx) = cx.add_window_view(|_, _| Changing {
        grown: false,
        invalid: false,
    });
    let grab = drag_divider(cx);
    host.update(cx, |host, cx| {
        host.grown = true;
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_mouse_move(
        grab + point(px(24.0), px(0.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    cx.run_until_parked();
    assert!(cx.debug_bounds("third").is_some());
}

#[gpui::test]
fn invalid_sizes_after_a_drag_split_evenly(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (host, cx) = cx.add_window_view(|_, _| Changing {
        grown: false,
        invalid: false,
    });
    let grab = drag_divider(cx);
    cx.simulate_mouse_up(
        grab + point(px(16.0), px(0.0)),
        MouseButton::Left,
        Modifiers::none(),
    );
    host.update(cx, |host, cx| {
        host.invalid = true;
        cx.notify();
    });
    cx.run_until_parked();
    let first = cx.debug_bounds("first").expect("first").size.width;
    let second = cx.debug_bounds("second").expect("second").size.width;
    assert_eq!(first, second);
}

/// A ScrollArea of `inner` content, 100px tall, in a page that scrolls.
struct Nested(Pixels, gpui::ScrollHandle);

impl Render for Nested {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.1)
            .child(
                super::ScrollArea::new("area")
                    .h(px(100.0))
                    .child(div().h(self.0)),
            )
            .child(div().h(px(2000.0)))
    }
}

fn wheel_page(inner: Pixels, cx: &mut TestAppContext) -> Pixels {
    cx.update(Theme::init);
    let page = gpui::ScrollHandle::new();
    let (_, cx) = cx.add_window_view({
        let page = page.clone();
        move |_, _| Nested(inner, page)
    });
    cx.run_until_parked();
    cx.simulate_event(gpui::ScrollEvent {
        position: point(px(50.0), px(50.0)),
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-40.0))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        source: gpui::ScrollSource::Pointer,
    });
    cx.run_until_parked();
    page.offset().y
}

#[gpui::test]
fn a_scroll_area_keeps_a_wheel_it_can_use(cx: &mut TestAppContext) {
    assert_eq!(wheel_page(px(500.0), cx), px(0.0));
}

#[gpui::test]
fn a_scroll_area_with_no_room_passes_the_wheel_on(cx: &mut TestAppContext) {
    assert_eq!(wheel_page(px(50.0), cx), px(-40.0));
}
