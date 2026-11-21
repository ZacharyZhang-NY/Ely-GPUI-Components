use gpui::{
    AnyElement, Axis, Context, Entity, InteractiveElement, IntoElement, KeyBinding, Modifiers,
    MouseButton, ParentElement, Pixels, Point, Render, SharedString, Size, Styled, TestAppContext,
    VisualTestContext, Window, div, point, prelude::*, px, size,
};

use super::{Resizable, Rotatable, RovingFocus, ScrollSync, SelectionArea};
use crate::{forms, primitives::FocusNext, theme::Theme};

mod limits;

type Part = fn(&Bench, Entity<Bench>) -> AnyElement;

/// A view that shows one interaction part and holds what the owner decides: a size, an angle and a selection.
struct Bench {
    part: Part,
    size: Size<Pixels>,
    angle: f32,
    selected: Vec<SharedString>,
}

impl Render for Bench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().p(px(20.0)).child((self.part)(self, cx.entity()))
    }
}

fn bench(part: Part, cx: &mut TestAppContext) -> (Entity<Bench>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Bench {
        part,
        size: size(px(200.0), px(120.0)),
        angle: 0.0,
        selected: Vec::new(),
    });
    settle(cx);
    (host, cx)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn tab(stops: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        (0..stops).for_each(|_| window.focus_next(cx));
    });
    settle(cx);
}

fn tap(key: &str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
}

fn at(x: f32, y: f32) -> Point<Pixels> {
    point(px(x), px(y))
}

/// Presses at `from`, moves through `path` with the button held, and lets go at its end.
fn drag(
    from: Point<Pixels>,
    path: &[Point<Pixels>],
    modifiers: Modifiers,
    cx: &mut VisualTestContext,
) {
    cx.simulate_mouse_down(from, MouseButton::Left, modifiers);
    for step in path {
        cx.simulate_mouse_move(*step, Some(MouseButton::Left), modifiers);
        settle(cx);
    }
    let end = *path.last().expect("a path");
    cx.simulate_mouse_up(end, MouseButton::Left, modifiers);
    settle(cx);
}

fn resizable(bench: &Bench, owner: Entity<Bench>) -> AnyElement {
    Resizable::new(
        "box",
        bench.size,
        size(px(100.0), px(80.0)),
        size(px(300.0), px(200.0)),
        move |next, _, cx| {
            owner.update(cx, |bench, cx| {
                bench.size = next;
                cx.notify();
            })
        },
    )
    .child(div().size_full())
    .into_any_element()
}

/// The grip is the one stop; the arrows resize a step, and a drag on the right edge only widens.
#[gpui::test]
fn a_box_resizes_from_its_grip_and_its_edge_within_its_limits(cx: &mut TestAppContext) {
    let (host, cx) = bench(resizable, cx);
    tab(1, cx);
    tap("right", cx);
    tap("down", cx);
    let stepped = host.read_with(cx, |bench, _| bench.size);
    assert_eq!(stepped, size(px(208.0), px(128.0)));
    drag(
        at(225.0, 60.0),
        &[at(240.0, 80.0), at(260.0, 100.0), at(400.0, 120.0)],
        Modifiers::none(),
        cx,
    );
    let widened = host.read_with(cx, |bench, _| bench.size);
    assert_eq!(
        widened,
        size(px(300.0), px(128.0)),
        "the right edge only widens, and stops at the widest"
    );
    drag(
        at(100.0, 145.0),
        &[at(150.0, 160.0), at(180.0, 180.0), at(200.0, 200.0)],
        Modifiers::none(),
        cx,
    );
    let deepened = host.read_with(cx, |bench, _| bench.size);
    assert_eq!(
        deepened,
        size(px(300.0), px(183.0)),
        "the bottom edge only deepens, by the pointer's way from the press"
    );
}

/// The grip and the edges' handles lie inside the box, the grip in its lower right corner.
#[gpui::test]
fn a_size_past_its_limits_pegs_and_draws(cx: &mut TestAppContext) {
    let (host, cx) = bench(resizable, cx);
    host.update(cx, |bench, cx| {
        bench.size = size(px(900.0), px(10.0));
        cx.notify();
    });
    settle(cx);
    let shown = cx.debug_bounds("resizable-box").expect("the box");
    assert_eq!(shown.size, size(px(300.0), px(80.0)));
}

#[gpui::test]
fn the_grip_lies_inside_the_box(cx: &mut TestAppContext) {
    let (_, cx) = bench(resizable, cx);
    let (grip, frame) = (
        cx.debug_bounds("resizable-grip").expect("the grip"),
        cx.debug_bounds("resizable-box").expect("the box"),
    );
    assert_eq!(
        (grip.right(), grip.bottom()),
        (frame.right(), frame.bottom()),
        "the grip ends at the box's corner: {grip:?} in {frame:?}"
    );
}

fn rotatable(bench: &Bench, owner: Entity<Bench>) -> AnyElement {
    Rotatable::new(
        "dial",
        "icons/arrow-up.svg",
        px(100.0),
        bench.angle,
        move |next, _, cx| {
            owner.update(cx, |bench, cx| {
                bench.angle = next;
                cx.notify();
            })
        },
    )
    .into_any_element()
}

/// The ring is the one stop; Left and Right turn it a notch, round the circle.
#[gpui::test]
fn a_dial_turns_by_notches_from_the_keyboard(cx: &mut TestAppContext) {
    let (host, cx) = bench(rotatable, cx);
    tab(1, cx);
    tap("left", cx);
    assert_eq!(host.read_with(cx, |bench, _| bench.angle), 345.0);
    tap("right", cx);
    tap("right", cx);
    assert_eq!(host.read_with(cx, |bench, _| bench.angle), 15.0);
}

/// The knob sits on top of a ring centered at 70, 70; a drag to 86 degrees with Shift snaps to 90.
#[gpui::test]
fn a_dial_turns_toward_its_dragged_knob(cx: &mut TestAppContext) {
    let (host, cx) = bench(rotatable, cx);
    drag(
        at(70.0, 26.0),
        &[at(90.0, 40.0), at(110.0, 55.0), at(130.0, 66.0)],
        Modifiers::shift(),
        cx,
    );
    assert_eq!(host.read_with(cx, |bench, _| bench.angle), 90.0);
}

/// Without Shift the same drag turns the dial to the pointer's own angle.
#[gpui::test]
fn a_free_drag_turns_to_the_pointer(cx: &mut TestAppContext) {
    let (host, cx) = bench(rotatable, cx);
    drag(
        at(70.0, 26.0),
        &[at(90.0, 40.0), at(110.0, 55.0), at(130.0, 66.0)],
        Modifiers::none(),
        cx,
    );
    let angle = host.read_with(cx, |bench, _| bench.angle);
    assert!(
        (angle - 86.19).abs() < 0.05,
        "turned to 86 degrees: {angle}"
    );
}

fn area(bench: &Bench, owner: Entity<Bench>) -> AnyElement {
    ["a", "b", "c"]
        .into_iter()
        .fold(
            SelectionArea::new("tiles", move |keys, _, cx| {
                let keys = keys.to_vec();
                owner.update(cx, |bench, cx| {
                    bench.selected = keys;
                    cx.notify();
                })
            })
            .flex()
            .gap(px(10.0))
            .p(px(20.0))
            .w(px(300.0)),
            |area, key| area.item(key, div().size(px(40.0))),
        )
        .selected(bench.selected.clone())
        .into_any_element()
}

fn picked(host: &Entity<Bench>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |bench, _| {
        bench.selected.iter().map(ToString::to_string).collect()
    })
}

/// Tiles sit at x 41, 93 and 145 inside the box, which starts at 20; a band from empty space below takes the two it crosses.
#[gpui::test]
fn a_band_selects_what_it_crosses(cx: &mut TestAppContext) {
    let (host, cx) = bench(area, cx);
    let none = Modifiers::none();
    cx.simulate_mouse_down(at(100.0, 100.0), MouseButton::Left, none);
    for step in [at(110.0, 95.0), at(150.0, 70.0), at(160.0, 60.0)] {
        cx.simulate_mouse_move(step, Some(MouseButton::Left), none);
        settle(cx);
    }
    // gpui now clears debug bounds each frame, and the band leaves on release.
    assert!(cx.debug_bounds("selection-band").is_some());
    cx.simulate_mouse_up(at(160.0, 60.0), MouseButton::Left, none);
    settle(cx);
    assert_eq!(picked(&host, cx), ["b", "c"]);
}

/// A press selects a tile alone, Command adds another, a press on empty space clears, and with focus Command-A takes all and Escape clears.
#[gpui::test]
fn presses_and_keys_pick_tiles(cx: &mut TestAppContext) {
    let (host, cx) = bench(area, cx);
    cx.simulate_click(at(60.0, 60.0), Modifiers::none());
    settle(cx);
    assert_eq!(picked(&host, cx), ["a"]);
    cx.simulate_click(at(112.0, 60.0), Modifiers::command());
    settle(cx);
    assert_eq!(picked(&host, cx), ["a", "b"]);
    cx.simulate_click(at(250.0, 100.0), Modifiers::none());
    settle(cx);
    assert!(
        picked(&host, cx).is_empty(),
        "a press on empty space clears"
    );
    tap("cmd-a", cx);
    assert_eq!(picked(&host, cx), ["a", "b", "c"]);
    tap("escape", cx);
    assert!(picked(&host, cx).is_empty());
}

/// With "a" selected, a band over "c" with Shift adds it.
#[gpui::test]
fn a_band_with_shift_adds_to_the_selection(cx: &mut TestAppContext) {
    let (host, cx) = bench(area, cx);
    host.update(cx, |bench, cx| {
        bench.selected = vec!["a".into()];
        cx.notify();
    });
    settle(cx);
    drag(
        at(200.0, 100.0),
        &[at(190.0, 90.0), at(170.0, 70.0), at(160.0, 60.0)],
        Modifiers::shift(),
        cx,
    );
    assert_eq!(picked(&host, cx), ["a", "c"]);
}

/// The box is one stop: the arrows walk the cursor and Space adds or drops the tile it is on.
#[gpui::test]
fn the_keyboard_walks_and_picks_tiles(cx: &mut TestAppContext) {
    let (host, cx) = bench(area, cx);
    tab(1, cx);
    tap("right", cx);
    tap("space", cx);
    tap("right", cx);
    tap("space", cx);
    assert_eq!(picked(&host, cx), ["b", "c"]);
    tap("left", cx);
    tap("space", cx);
    assert_eq!(picked(&host, cx), ["c"]);
}

/// A key pressed and let go, as a press on a focused item needs.
fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

fn roving(bench: &Bench, owner: Entity<Bench>) -> AnyElement {
    let _ = bench;
    ["a", "b", "c"]
        .into_iter()
        .fold(
            RovingFocus::new("tools", Axis::Horizontal, move |key, _, cx| {
                let key = key.clone();
                owner.update(cx, |bench, cx| {
                    bench.selected.push(key);
                    cx.notify();
                })
            }),
            |group, key| group.item(key, div().size(px(24.0))),
        )
        .into_any_element()
}

/// The group is one stop: Right walks focus on, End jumps to the last, and a Tab away and back returns to the item last focused.
#[gpui::test]
fn a_group_keeps_one_stop_and_the_arrows_walk_it(cx: &mut TestAppContext) {
    let (host, cx) = bench(roving, cx);
    tab(1, cx);
    tap("right", cx);
    press("space", cx);
    tap("end", cx);
    press("space", cx);
    tab(1, cx);
    press("space", cx);
    assert_eq!(
        picked(&host, cx),
        ["b", "c", "c"],
        "one stop, and it stays on the last item focused"
    );
}

fn synced(_: &Bench, _: Entity<Bench>) -> AnyElement {
    let lines = |side: &'static str| {
        div().flex().flex_col().children((0..40).map(move |n| {
            div().h(px(30.0)).when(n == 0, |line| {
                line.debug_selector(move || format!("{side}-first"))
            })
        }))
    };
    ScrollSync::new("sync", Axis::Vertical)
        .w(px(300.0))
        .h(px(200.0))
        .pane(lines("left"))
        .pane(lines("right"))
        .into_any_element()
}

/// A wheel on the left pane takes the right one to the same place.
#[gpui::test]
fn a_wheel_on_one_pane_takes_the_other_along(cx: &mut TestAppContext) {
    let (_, cx) = bench(synced, cx);
    let before = cx
        .debug_bounds("right-first")
        .expect("the right pane")
        .origin
        .y;
    cx.simulate_event(gpui::ScrollEvent {
        position: at(80.0, 100.0),
        delta: gpui::ScrollDelta::Pixels(point(px(0.0), px(-300.0))),
        modifiers: Modifiers::none(),
        touch_phase: gpui::TouchPhase::Moved,
        source: gpui::ScrollSource::Pointer,
    });
    settle(cx);
    let after = cx
        .debug_bounds("right-first")
        .expect("the right pane")
        .origin
        .y;
    assert_eq!(
        before - after,
        px(300.0),
        "the right pane moved with the left"
    );
}

/// A press moves the stop to the item pressed and leaves focus where it was: items 26 wide from 20, so b spans 46 to 72.
#[gpui::test]
fn a_press_moves_the_stop_and_leaves_focus(cx: &mut TestAppContext) {
    let (host, cx) = bench(roving, cx);
    cx.simulate_click(at(59.0, 33.0), Modifiers::none());
    settle(cx);
    let focused = cx.update(|window, cx| window.focused(cx).is_some());
    assert!(!focused, "a press takes no focus");
    tab(1, cx);
    press("space", cx);
    assert_eq!(picked(&host, cx), ["b", "b"]);
}
