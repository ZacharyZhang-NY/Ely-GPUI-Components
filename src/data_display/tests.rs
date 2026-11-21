use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use gpui::{
    AnyElement, Bounds, Context, IntoElement, Modifiers, ParentElement, Pixels, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, point, px,
};

use super::{
    Badge, Carousel, DescriptionList, Gauge, PropertyGrid, PropertyGroup, Stars, Statistic, Tone,
    UsageBar,
};
use crate::{
    primitives::Measure,
    theme::{TextSize, Theme},
    typography::AnimatedNumber,
};

/// A property grid whose drawn height the test reads.
struct Inspector(Rc<Cell<Pixels>>);

impl Render for Inspector {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let height = self.0.clone();
        let editor = || div().h(px(20.0));
        div().w(px(320.0)).child(
            Measure::new("measure", move |bounds, _, _| {
                height.set(bounds.size.height)
            })
            .child(
                PropertyGrid::new("grid")
                    .group(
                        PropertyGroup::new("Layout")
                            .row("Width", editor())
                            .row("Height", editor()),
                    )
                    .group(PropertyGroup::new("Export").folded().row("Scale", editor())),
            ),
        )
    }
}

/// Frames 2ms apart, past reduced motion's 1ms folds.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(2));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
}

#[gpui::test]
fn a_property_group_folds_from_its_header_and_stays_folded(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
    });
    let height = Rc::new(Cell::new(Pixels::ZERO));
    let seen = height.clone();
    let (_, cx) = cx.add_window_view(|_, _| Inspector(seen));
    settle(cx);
    let open = height.get();
    let header = point(px(4.0), px(12.0));
    cx.simulate_click(header, Modifiers::none());
    settle(cx);
    let folded = height.get();
    assert!(folded < open, "the rows fold away: {open:?} to {folded:?}");
    settle(cx);
    assert_eq!(height.get(), folded, "a new frame keeps the fold");
    cx.simulate_click(header, Modifiers::none());
    settle(cx);
    assert_eq!(height.get(), open, "a second press opens it");
}

/// A one-row description list, this wide, that reports where the list and its value sit.
struct Facts(Pixels, fn() -> AnyElement, Rc<Cell<[Bounds<Pixels>; 2]>>);

fn license() -> AnyElement {
    "Apache-2.0".into_any_element()
}

impl Render for Facts {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (list, value) = (self.2.clone(), self.2.clone());
        let value = Measure::new("value", move |bounds, _, _| {
            value.set([value.get()[0], bounds])
        })
        .child((self.1)());
        div().w(self.0).child(
            Measure::new("list", move |bounds, _, _| {
                list.set([bounds, list.get()[1]])
            })
            .child(DescriptionList::new().item("License", value)),
        )
    }
}

#[gpui::test]
fn a_value_drops_below_its_label_when_the_row_runs_short(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let mut place = |width: f32| {
        let seen = Rc::new(Cell::new([Bounds::default(); 2]));
        let shared = seen.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Facts(px(width), license, shared));
        settle(cx);
        let [list, value] = seen.get();
        (value.left() - list.left(), value.top() - list.top())
    };
    let (wide_left, wide_top) = place(400.0);
    let (narrow_left, narrow_top) = place(240.0);
    assert_eq!(
        wide_left,
        px(176.0),
        "wide, the value sits beside its label"
    );
    assert_eq!(
        narrow_left,
        Pixels::ZERO,
        "narrow, the value starts under its label"
    );
    assert!(
        narrow_top > wide_top,
        "and a line lower: {wide_top:?} to {narrow_top:?}"
    );
}

#[gpui::test]
fn a_badge_value_rests_on_its_labels_line(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let mut foot = |value: fn() -> AnyElement| {
        let seen = Rc::new(Cell::new([Bounds::default(); 2]));
        let shared = seen.clone();
        let (_, cx) = cx.add_window_view(move |_, _| Facts(px(400.0), value, shared));
        settle(cx);
        let [list, value] = seen.get();
        value.bottom() - list.top()
    };
    let text = foot(license);
    let badge = foot(|| {
        Badge::new("Paid")
            .tone(Tone::Success)
            .dot()
            .into_any_element()
    });
    assert_eq!(badge, text, "a badge ends where its label's line does");
}

/// A statistic with units on both sides.
struct Revenue;

impl Render for Revenue {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Statistic::new("revenue", "Revenue", 48_210.0)
            .prefix("$")
            .suffix("ms")
    }
}

#[gpui::test]
fn units_rest_on_the_bottom_of_a_statistics_number(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
    });
    let (_, cx) = cx.add_window_view(|_, _| Revenue);
    settle(cx);
    let number = cx
        .debug_bounds("animated-number revenue")
        .expect("the number");
    for unit in ["statistic-prefix revenue", "statistic-suffix revenue"] {
        let bounds = cx.debug_bounds(unit).expect("a unit");
        assert_eq!(
            bounds.bottom(),
            number.bottom(),
            "{unit} rests on the number's bottom"
        );
    }
}

/// A lone digit whose line is no whole device pixel.
struct Nine;

impl Render for Nine {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        AnimatedNumber::new("nine", 9.0).size(TextSize::Xs)
    }
}

#[gpui::test]
fn a_rolled_digit_sits_square_in_its_cell(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
    });
    let (_, cx) = cx.add_window_view(|_, _| Nine);
    settle(cx);
    let cell = cx.debug_bounds("rolling-cell").expect("the cell");
    let nine = cx.debug_bounds("rolling-9").expect("the nine");
    assert_eq!(nine.top(), cell.top(), "the nine fills its cell");
}

/// A three-slide carousel, 200 wide, that turns every five seconds; each slide reports where it sits.
struct Deck(Rc<RefCell<[Pixels; 3]>>);

impl Render for Deck {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let slides = (0..3).map(|ix| {
            let seen = self.0.clone();
            Measure::new(("slide", ix), move |bounds, _, _| {
                seen.borrow_mut()[ix] = bounds.origin.x
            })
            .size_full()
        });
        Carousel::new("deck")
            .autoplay(Duration::from_secs(5))
            .w(px(200.0))
            .h(px(100.0))
            .children(slides)
    }
}

fn deck(cx: &mut TestAppContext) -> (Rc<RefCell<[Pixels; 3]>>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
    });
    let places = Rc::new(RefCell::new([Pixels::ZERO; 3]));
    let seen = places.clone();
    let (_, cx) = cx.add_window_view(|_, _| Deck(seen));
    settle(cx);
    (places, cx)
}

/// The slide in view: the one at the viewport's left edge, inside its border.
fn front(places: &Rc<RefCell<[Pixels; 3]>>) -> Option<usize> {
    places.borrow().iter().position(|x| *x == px(1.0))
}

fn wait(time: Duration, cx: &mut VisualTestContext) {
    cx.executor().advance_clock(time);
    settle(cx);
}

#[gpui::test]
fn autoplay_turns_the_carousel_and_holds_while_pointed_at(cx: &mut TestAppContext) {
    let (places, cx) = deck(cx);
    // The pointer starts at the corner, over the deck, which gpui now counts as a hover.
    cx.simulate_mouse_move(point(px(600.0), px(400.0)), None, Modifiers::none());
    settle(cx);
    assert_eq!(front(&places), Some(0));
    wait(Duration::from_secs(5), cx);
    assert_eq!(front(&places), Some(1), "five seconds turn it once");
    cx.simulate_mouse_move(point(px(100.0), px(50.0)), None, Modifiers::none());
    settle(cx);
    wait(Duration::from_secs(12), cx);
    assert_eq!(front(&places), Some(1), "held while pointed at");
    cx.simulate_mouse_move(point(px(600.0), px(400.0)), None, Modifiers::none());
    settle(cx);
    wait(Duration::from_secs(5), cx);
    assert_eq!(front(&places), Some(2));
}

#[gpui::test]
fn a_key_under_a_resting_pointer_keeps_the_carousel_held(cx: &mut TestAppContext) {
    let (places, cx) = deck(cx);
    cx.simulate_mouse_move(point(px(100.0), px(50.0)), None, Modifiers::none());
    settle(cx);
    cx.simulate_keystrokes("a");
    settle(cx);
    wait(Duration::from_secs(12), cx);
    assert_eq!(front(&places), Some(0), "still held");
}

#[gpui::test]
fn left_from_the_first_slide_wraps_to_the_last(cx: &mut TestAppContext) {
    let (places, cx) = deck(cx);
    cx.simulate_click(point(px(100.0), px(50.0)), Modifiers::none());
    settle(cx);
    cx.simulate_keystrokes("left");
    settle(cx);
    assert_eq!(front(&places), Some(2));
    cx.simulate_keystrokes("right");
    settle(cx);
    assert_eq!(front(&places), Some(0));
}

/// Readings past their scale: a gauge over its max, stars over theirs, more parts than hues.
struct Over;

impl Render for Over {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let parts = (0..10).fold(UsageBar::new(100.0), |bar, ix| {
            bar.part(format!("p{ix}"), 12.0)
        });
        div()
            .child(Gauge::new("cpu", "CPU", 140.0, 0.0, 100.0))
            .child(Stars::new(6.5))
            .child(parts)
    }
}

#[gpui::test]
fn readings_past_their_scale_draw(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Over);
    cx.run_until_parked();
}
