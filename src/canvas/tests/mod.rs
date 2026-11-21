use gpui::{
    AnyElement, Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement, Render,
    ScrollDelta, ScrollEvent, ScrollSource, Styled, TestAppContext, TouchPhase, VisualTestContext,
    Window, div, point, px,
};

use super::{Frame, InfiniteCanvas, MiniMap, Tool, Viewport, ZoomControls};
use crate::{primitives::FocusNext, theme::Theme};

mod boards;
mod panels;
mod stale;
mod tools;

/// A view that shows one canvas part and keeps the viewports and words it heard.
struct Stage {
    part: fn(&Stage, Entity<Stage>) -> AnyElement,
    tool: Tool,
    heard: Vec<Viewport>,
    said: Vec<String>,
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(400.0))
            .h(px(300.0))
            .child((self.part)(self, cx.entity()))
    }
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn stage(
    part: fn(&Stage, Entity<Stage>) -> AnyElement,
    cx: &mut TestAppContext,
) -> (Entity<Stage>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        crate::forms::bind_keys(cx);
        cx.bind_keys([gpui::KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Stage {
        part,
        tool: Tool::Select,
        heard: Vec::new(),
        said: Vec::new(),
    });
    settle(cx);
    (host, cx)
}

fn hear(owner: &Entity<Stage>, view: Viewport, cx: &mut gpui::App) {
    owner.update(cx, |stage, cx| {
        stage.heard.push(view);
        cx.notify();
    });
}

fn say(owner: &Entity<Stage>, words: String, cx: &mut gpui::App) {
    owner.update(cx, |stage, cx| {
        stage.said.push(words);
        cx.notify();
    });
}

fn said(host: &Entity<Stage>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |stage, _| stage.said.clone())
}

fn heard(host: &Entity<Stage>, cx: &mut VisualTestContext) -> Vec<Viewport> {
    host.read_with(cx, |stage, _| stage.heard.clone())
}

fn plane(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    InfiniteCanvas::new("plane", Viewport::new(0.0, 0.0, 1.0))
        .on_viewport(move |view, _, cx| hear(&owner, view, cx))
        .into_any_element()
}

#[gpui::test]
fn command_and_the_wheel_zoom_about_the_pointer(cx: &mut TestAppContext) {
    let (host, cx) = stage(plane, cx);
    cx.simulate_event(ScrollEvent {
        position: point(px(100.0), px(50.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-240.0))),
        modifiers: Modifiers::command(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
    let view = *heard(&host, cx).last().expect("a zoom");
    assert!(
        (view.zoom - std::f32::consts::E).abs() < 0.01,
        "a turn of 240 zooms by e: {}",
        view.zoom
    );
    let held = view.to_canvas((100.0, 50.0));
    assert!(
        (held.0 - 100.0).abs() < 0.01 && (held.1 - 50.0).abs() < 0.01,
        "the point under the pointer stays: {held:?}"
    );
}

#[gpui::test]
fn a_drag_on_empty_space_pans(cx: &mut TestAppContext) {
    let (host, cx) = stage(plane, cx);
    let at = |x: f32, y: f32| point(px(x), px(y));
    cx.simulate_mouse_move(at(200.0, 150.0), None, Modifiers::none());
    cx.simulate_mouse_down(at(200.0, 150.0), MouseButton::Left, Modifiers::none());
    for (x, y) in [(190.0, 145.0), (170.0, 135.0), (150.0, 130.0)] {
        cx.simulate_mouse_move(at(x, y), Some(MouseButton::Left), Modifiers::none());
        settle(cx);
    }
    cx.simulate_mouse_up(at(150.0, 130.0), MouseButton::Left, Modifiers::none());
    settle(cx);
    let view = *heard(&host, cx).last().expect("a pan");
    assert_eq!(
        (view.x, view.y),
        (50.0, 20.0),
        "the content followed the pointer from the press"
    );
}

fn zoom(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    ZoomControls::new("zoom", 1.0)
        .on_zoom(move |zoom, _, cx| hear(&owner, Viewport::new(0.0, 0.0, zoom), cx))
        .into_any_element()
}

#[gpui::test]
fn the_zoom_buttons_step_and_the_percent_resets(cx: &mut TestAppContext) {
    let (host, cx) = stage(zoom, cx);
    for nth in [1, 2, 3] {
        cx.update(|window, cx| {
            window.blur(cx);
            (0..nth).for_each(|_| window.focus_next(cx));
        });
        cx.simulate_keystrokes("space");
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse("space").expect("a key"),
        });
        settle(cx);
    }
    let zooms: Vec<f32> = heard(&host, cx).iter().map(|view| view.zoom).collect();
    assert_eq!(zooms, [0.75, 1.0, 1.5]);
}

fn map(_: &Stage, owner: Entity<Stage>) -> AnyElement {
    MiniMap::new(
        "map",
        [Frame::new(0.0, 0.0, 400.0, 280.0)],
        Viewport::new(0.0, 0.0, 1.0),
        (100.0, 70.0),
    )
    .on_viewport(move |view, _, cx| hear(&owner, view, cx))
    .into_any_element()
}

#[gpui::test]
/// The map draws inside its 1px edge, so its point 150, 105 lies at 151, 106.
fn a_press_on_the_map_centers_the_view_there(cx: &mut TestAppContext) {
    let (host, cx) = stage(map, cx);
    cx.simulate_click(point(px(151.0), px(106.0)), Modifiers::none());
    settle(cx);
    let view = *heard(&host, cx).last().expect("a move");
    let middle = view.to_canvas((50.0, 35.0));
    assert!(
        (middle.0 - 300.0).abs() < 1.0 && (middle.1 - 210.0).abs() < 1.0,
        "the view centers on the point pressed: {middle:?}"
    );
}

fn wheel(at: (f32, f32), dy: f32, modifiers: Modifiers) -> ScrollEvent {
    ScrollEvent {
        position: point(px(at.0), px(at.1)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(dy))),
        modifiers,
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    }
}

#[gpui::test]
fn a_plain_wheel_moves_the_content_as_a_scroll_box_does(cx: &mut TestAppContext) {
    let (host, cx) = stage(plane, cx);
    cx.simulate_event(wheel((100.0, 50.0), -50.0, Modifiers::none()));
    settle(cx);
    let view = *heard(&host, cx).last().expect("a scroll");
    assert_eq!(
        view.to_view((0.0, 0.0)),
        (0.0, -50.0),
        "the content went up with the wheel"
    );
}

#[gpui::test]
fn wheels_and_moves_add_up(cx: &mut TestAppContext) {
    let (host, cx) = stage(plane, cx);
    for dy in [-10.0, -10.0, -10.0, 0.0] {
        cx.simulate_event(wheel((100.0, 50.0), dy, Modifiers::none()));
    }
    settle(cx);
    let view = *heard(&host, cx).last().expect("a scroll");
    assert_eq!(
        view.y, 30.0,
        "three wheels add up, and an end that moves nothing leaves it"
    );
    let at = |x: f32, y: f32| point(px(x), px(y));
    cx.simulate_mouse_move(at(200.0, 150.0), None, Modifiers::none());
    cx.simulate_mouse_down(at(200.0, 150.0), MouseButton::Left, Modifiers::none());
    for x in [190.0, 180.0, 170.0, 160.0] {
        cx.simulate_mouse_move(at(x, 150.0), Some(MouseButton::Left), Modifiers::none());
    }
    cx.simulate_mouse_up(at(160.0, 150.0), MouseButton::Left, Modifiers::none());
    settle(cx);
    let view = *heard(&host, cx).last().expect("a pan");
    assert_eq!(
        (view.x, view.y),
        (40.0, 30.0),
        "the drag pans from the press by the pointer's whole way"
    );
}
