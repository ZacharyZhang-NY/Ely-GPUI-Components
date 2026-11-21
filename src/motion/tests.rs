use std::{cell::Cell, rc::Rc};

use gpui::{
    Context, InteractiveElement, IntoElement, Modifiers, ParentElement, Render, ScrollDelta,
    ScrollEvent, ScrollSource, StatefulInteractiveElement, Styled, TestAppContext, TouchPhase,
    VisualTestContext, Window, div, point, px,
};

use super::{LazyLoad, LoadingOverlay, ProgressBar};
use crate::theme::Theme;

/// A pressable box under a loading veil, and a lazy row far down a scroll.
struct Bench {
    loading: bool,
    presses: usize,
    built: Rc<Cell<usize>>,
}

impl Render for Bench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (view, built) = (cx.entity(), self.built.clone());
        div()
            .child(
                LoadingOverlay::new("veil", self.loading).child(
                    div()
                        .id("under")
                        .w(px(200.0))
                        .h(px(100.0))
                        .on_click(move |_, _, cx| view.update(cx, |bench, _| bench.presses += 1)),
                ),
            )
            .child(
                div()
                    .id("scroll")
                    .h(px(100.0))
                    .overflow_y_scroll()
                    .child(div().h(px(1000.0)))
                    .child(LazyLoad::new("row", px(20.0), move |_, _| {
                        built.set(built.get() + 1);
                        div().h(px(20.0)).child("row")
                    })),
            )
    }
}

fn bench(loading: bool, cx: &mut TestAppContext) -> (gpui::Entity<Bench>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
    });
    let (view, cx) = cx.add_window_view(|_, _| Bench {
        loading,
        presses: 0,
        built: Rc::new(Cell::new(0)),
    });
    cx.run_until_parked();
    (view, cx)
}

fn press_box(cx: &mut VisualTestContext) {
    let at = point(px(100.0), px(50.0));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn the_veil_stops_presses_only_while_loading(cx: &mut TestAppContext) {
    let (view, cx) = bench(true, cx);
    press_box(cx);
    assert_eq!(view.read_with(cx, |bench, _| bench.presses), 0);
    view.update(cx, |bench, cx| {
        bench.loading = false;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    press_box(cx);
    assert_eq!(view.read_with(cx, |bench, _| bench.presses), 1);
}

#[gpui::test]
fn a_lazy_row_builds_once_it_scrolls_into_view(cx: &mut TestAppContext) {
    let (view, cx) = bench(false, cx);
    let built = view.read_with(cx, |bench, _| bench.built.clone());
    assert_eq!(built.get(), 0);
    cx.simulate_event(ScrollEvent {
        position: point(px(50.0), px(150.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-2000.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    assert!(built.get() >= 1, "the row was built");
}

#[test]
#[should_panic(expected = "not 0..=1")]
fn a_value_past_whole_is_refused() {
    let _ = ProgressBar::new("bar", 1.2);
}

/// A sweeping bar at the top of a tall column, and where the next child starts.
struct Column {
    after: Rc<Cell<f32>>,
}

impl Render for Column {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let after = self.after.clone();
        div()
            .flex()
            .flex_col()
            .h(px(100.0))
            .child(ProgressBar::indeterminate("sweep"))
            .child(
                crate::primitives::Measure::new("after", move |bounds, _, _| {
                    after.set(f32::from(bounds.origin.y))
                })
                .h(px(10.0)),
            )
    }
}

#[gpui::test]
fn a_sweeping_bar_keeps_its_thickness_in_a_tall_column(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let after = Rc::new(Cell::new(-1.0));
    let (_, cx) = cx.add_window_view({
        let after = after.clone();
        move |_, _| Column { after }
    });
    cx.run_until_parked();
    let thickness = cx.update(|window, cx| {
        f32::from(
            crate::theme::ActiveTheme::theme(cx)
                .progress_thickness()
                .to_pixels(window.rem_size()),
        )
    });
    assert_eq!(after.get(), thickness, "the bar took only its thickness");
}

/// A transition, a presence list and a reorderable list, recording what they report.
struct Stage {
    shown: bool,
    rows: Vec<(&'static str, bool)>,
    order: Vec<&'static str>,
    /// The top of what sits below the transition.
    drawn: Rc<Cell<usize>>,
    /// Each reorder row's keyed state, by row.
    seen: Rc<std::cell::RefCell<std::collections::HashMap<&'static str, gpui::EntityId>>>,
    log: Vec<String>,
}

/// A 20px row that keeps keyed state and says which entity holds it.
#[derive(IntoElement)]
struct Marker {
    key: &'static str,
    seen: Rc<std::cell::RefCell<std::collections::HashMap<&'static str, gpui::EntityId>>>,
}

impl gpui::RenderOnce for Marker {
    fn render(self, window: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let state = window.use_keyed_state(self.key, cx, |_, _| ());
        self.seen.borrow_mut().insert(self.key, state.entity_id());
        div().h(px(20.0)).child(self.key)
    }
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drawn = self.drawn.clone();
        let (gone, moved) = (cx.entity(), cx.entity());
        let presence = self.rows.iter().fold(
            super::AnimatePresence::new("presence").on_gone(move |key, _, cx| {
                gone.update(cx, |stage, _| stage.log.push(format!("gone {key}")))
            }),
            |list, (key, shown)| list.row(*key, *shown, div().h(px(20.0)).child(*key)),
        );
        let reorder = self.order.iter().fold(
            super::Reorder::new("reorder").on_reorder(move |from, to, _, cx| {
                moved.update(cx, |stage, cx| {
                    let row = stage.order.remove(from);
                    stage.order.insert(to, row);
                    stage.log.push(format!("moved {from} to {to}"));
                    cx.notify();
                })
            }),
            |list, key| {
                list.row(
                    *key,
                    Marker {
                        key,
                        seen: self.seen.clone(),
                    },
                )
            },
        );
        div()
            .w(px(300.0))
            .child(reorder)
            .child(super::Transition::new("fade", self.shown).child(div().h(px(10.0))))
            .child(
                crate::primitives::Measure::new("below", move |bounds, _, _| {
                    drawn.set(f32::from(bounds.origin.y) as usize)
                })
                .h(px(1.0)),
            )
            .child(presence)
    }
}

fn stage(cx: &mut TestAppContext) -> (gpui::Entity<Stage>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
    });
    let (view, cx) = cx.add_window_view(|_, _| Stage {
        shown: true,
        rows: vec![("a", true), ("b", true)],
        order: vec!["one", "two", "three", "four", "five"],
        drawn: Rc::new(Cell::new(0)),
        seen: Rc::default(),
        log: Vec::new(),
    });
    settle(cx);
    (view, cx)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(2));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

#[gpui::test]
fn a_hidden_transition_leaves_then_stops_drawing(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    let below = view.read_with(cx, |stage, _| stage.drawn.clone());
    assert_eq!(below.get(), 110, "the reorder list, then the shown content");
    view.update(cx, |stage, cx| {
        stage.shown = false;
        cx.notify();
    });
    settle(cx);
    assert_eq!(below.get(), 100, "the content left the layout");
}

#[gpui::test]
fn a_row_marked_gone_is_reported_once(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    view.update(cx, |stage, cx| {
        stage.rows[1].1 = false;
        cx.notify();
    });
    settle(cx);
    settle(cx);
    let log = view.read_with(cx, |stage, _| stage.log.clone());
    assert_eq!(log, ["gone b"]);
}

#[gpui::test]
fn dragging_a_row_down_moves_it_past_the_rows_it_passes(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    let at = |y: f32| point(px(100.0), px(y));
    cx.simulate_mouse_move(at(10.0), None, Modifiers::none());
    cx.simulate_mouse_down(at(10.0), gpui::MouseButton::Left, Modifiers::none());
    for y in [16.0, 30.0, 45.0, 50.0] {
        cx.simulate_mouse_move(at(y), Some(gpui::MouseButton::Left), Modifiers::none());
        cx.run_until_parked();
    }
    cx.simulate_mouse_up(at(50.0), gpui::MouseButton::Left, Modifiers::none());
    settle(cx);
    let (order, log) = view.read_with(cx, |stage, _| (stage.order.clone(), stage.log.clone()));
    assert_eq!(log, ["moved 0 to 2"]);
    assert_eq!(order, ["two", "three", "one", "four", "five"]);
}

#[gpui::test]
fn a_moved_row_keeps_its_state(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    let seen = view.read_with(cx, |stage, _| stage.seen.clone());
    let before = seen.borrow()["one"];
    view.update(cx, |stage, cx| {
        stage.order.reverse();
        cx.notify();
    });
    settle(cx);
    assert_eq!(
        seen.borrow()["one"],
        before,
        "the same state after the move"
    );
}

#[gpui::test]
fn a_list_that_shrinks_under_a_drag_lets_it_go(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    let at = |y: f32| point(px(100.0), px(y));
    cx.simulate_mouse_move(at(90.0), None, Modifiers::none());
    cx.simulate_mouse_down(at(90.0), gpui::MouseButton::Left, Modifiers::none());
    for y in [84.0, 60.0] {
        cx.simulate_mouse_move(at(y), Some(gpui::MouseButton::Left), Modifiers::none());
        cx.run_until_parked();
    }
    view.update(cx, |stage, cx| {
        stage.order.truncate(2);
        cx.notify();
    });
    settle(cx);
    cx.simulate_mouse_up(at(20.0), gpui::MouseButton::Left, Modifiers::none());
    settle(cx);
    let log = view.read_with(cx, |stage, _| stage.log.clone());
    assert!(log.is_empty(), "no move for a row that left: {log:?}");
}

#[gpui::test]
fn a_row_hidden_from_the_start_is_gone_at_once(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    cx.update(|_, cx| Theme::update(cx, |theme| theme.reduced_motion = false));
    view.update(cx, |stage, cx| {
        stage.rows.push(("c", false));
        cx.notify();
    });
    settle(cx);
    let log = view.read_with(cx, |stage, _| stage.log.clone());
    assert_eq!(log, ["gone c"], "no fold to wait for");
}

/// A shaken and a flashed box around keyed state, and a press target under a ripple and confetti.
struct Effects {
    key: usize,
    presses: usize,
    seen: Rc<std::cell::RefCell<std::collections::HashMap<&'static str, gpui::EntityId>>>,
}

impl Render for Effects {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let marker = |key| Marker {
            key,
            seen: self.seen.clone(),
        };
        div()
            .child(super::Shake::new("shake", self.key).child(marker("shaken")))
            .child(super::Flash::new("flash", self.key).child(marker("flashed")))
            .child(
                div()
                    .relative()
                    .w(px(200.0))
                    .h(px(100.0))
                    .child(super::Ripple::new("ripple").size_full().child(
                        div().id("target").size_full().on_click(move |_, _, cx| {
                            view.update(cx, |effects, _| effects.presses += 1)
                        }),
                    ))
                    .child(super::Confetti::new("confetti", self.key)),
            )
    }
}

#[gpui::test]
fn effects_keep_their_content_and_let_presses_through(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
    });
    let (view, cx) = cx.add_window_view(|_, _| Effects {
        key: 0,
        presses: 0,
        seen: Rc::default(),
    });
    settle(cx);
    let seen = view.read_with(cx, |effects, _| effects.seen.clone());
    let before = seen.borrow().clone();
    view.update(cx, |effects, cx| {
        effects.key += 1;
        cx.notify();
    });
    settle(cx);
    assert_eq!(
        *seen.borrow(),
        before,
        "a shake and a flash keep what they hold"
    );
    let at = point(px(100.0), px(90.0));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_click(at, Modifiers::none());
    settle(cx);
    assert_eq!(view.read_with(cx, |effects, _| effects.presses), 1);
}

/// Rows rising in a sized box.
struct Rising;

impl Render for Rising {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows = ["Inbox", "Drafts", "Sent"].map(|name| {
            div()
                .debug_selector(move || name.into())
                .px_4()
                .py_2()
                .child(name)
        });
        let column = div().flex().flex_col().child(
            div()
                .debug_selector(|| "risen".into())
                .w(px(300.0))
                .child(super::Stagger::new("rise").children(rows)),
        );
        div().size_full().flex().child(div().flex_1().child(column))
    }
}

#[gpui::test]
fn a_stagger_box_takes_in_its_first_row_rise(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Rising);
    cx.run_until_parked();
    let bounds = |cx: &mut VisualTestContext, name| cx.debug_bounds(name).expect(name);
    let (held, first, last) = (bounds(cx, "risen"), bounds(cx, "Inbox"), bounds(cx, "Sent"));
    assert!(
        first.top() - held.top() > super::NUDGE,
        "the first row starts risen"
    );
    assert_eq!(last.bottom(), held.bottom(), "the box ends with its rows");
}
