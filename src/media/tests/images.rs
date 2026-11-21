use std::{cell::Cell, path::PathBuf, rc::Rc};

use gpui::{
    Bounds, Context, InteractiveElement, IntoElement, Modifiers, MouseButton, MouseDownEvent,
    MouseUpEvent, ParentElement, Pixels, Render, ScrollDelta, ScrollEvent, ScrollSource, Styled,
    TestAppContext, TouchPhase, VisualTestContext, Window, div, point, px,
};

use super::{picture, press, settle, setup};
use crate::media::{ImageThumbnail, ImageViewer};

/// A viewer 400 by 300 over a picture the test may change.
struct Viewing(PathBuf);

impl Render for Viewing {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(400.0))
            .h(px(300.0))
            .child(ImageViewer::new("viewer", self.0.clone()))
    }
}

fn viewing(path: PathBuf, cx: &mut TestAppContext) -> &mut VisualTestContext {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Viewing(path));
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx
}

/// A press on the stage, which takes focus as a user's would.
fn focus_stage(cx: &mut VisualTestContext) {
    let stage = cx
        .debug_bounds("image-viewer-stage")
        .expect("the stage draws");
    cx.simulate_click(stage.center(), Modifiers::none());
    settle(cx);
}

/// A double press at `position`.
fn press_twice(position: gpui::Point<Pixels>, cx: &mut VisualTestContext) {
    let (button, modifiers) = (MouseButton::Left, Modifiers::none());
    cx.simulate_event(MouseDownEvent {
        button,
        position,
        modifiers,
        click_count: 2,
        first_mouse: false,
    });
    cx.simulate_event(MouseUpEvent {
        button,
        position,
        modifiers,
        click_count: 2,
    });
    settle(cx);
}

fn shown(cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.debug_bounds("image-viewer-picture")
        .expect("the picture draws")
}

#[gpui::test]
fn keys_zoom_by_steps_fit_and_turn_the_picture(cx: &mut TestAppContext) {
    let cx = viewing(picture("keys", 200, 100), cx);
    assert_eq!(
        shown(cx).size,
        gpui::size(px(200.0), px(100.0)),
        "a small picture keeps its size"
    );
    let stage = cx
        .debug_bounds("image-viewer-stage")
        .expect("the stage draws");
    assert_eq!(shown(cx).center(), stage.center(), "and sits in the middle");
    focus_stage(cx);
    press("=", cx);
    assert_eq!(
        shown(cx).size,
        gpui::size(px(250.0), px(125.0)),
        "a step up to 125%"
    );
    press("r", cx);
    assert_eq!(
        shown(cx).size,
        gpui::size(px(100.0), px(200.0)),
        "a quarter turn stands it up, fitted again"
    );
    press("-", cx);
    assert_eq!(
        shown(cx).size,
        gpui::size(px(75.0), px(150.0)),
        "a step down to 75%"
    );
    press("1", cx);
    assert_eq!(
        shown(cx).size,
        gpui::size(px(100.0), px(200.0)),
        "1 shows actual size"
    );
}

#[gpui::test]
fn a_drag_pans_to_the_edge_and_keeps_going_past_the_stage(cx: &mut TestAppContext) {
    let cx = viewing(picture("pan", 400, 300), cx);
    focus_stage(cx);
    for _ in 0..3 {
        press("=", cx);
    }
    let stage = cx
        .debug_bounds("image-viewer-stage")
        .expect("the stage draws");
    let middle = stage.center();
    let none = Modifiers::none();
    cx.simulate_mouse_down(middle, MouseButton::Left, none);
    cx.simulate_mouse_move(middle - point(px(40.0), px(0.0)), MouseButton::Left, none);
    cx.simulate_mouse_move(middle - point(px(900.0), px(0.0)), MouseButton::Left, none);
    cx.simulate_mouse_up(middle - point(px(900.0), px(0.0)), MouseButton::Left, none);
    settle(cx);
    let (picture, inner) = (shown(cx), stage.size.width - px(2.0));
    assert!(
        picture.size.width > inner,
        "zoomed past the stage: {picture:?}"
    );
    assert!(
        (picture.right() - (stage.right() - px(1.0))).abs() < px(0.5),
        "the right edge stops at the stage's: {picture:?} in {stage:?}"
    );
}

#[gpui::test]
fn a_double_press_toggles_actual_size_and_cmd_scroll_zooms_about_the_pointer(
    cx: &mut TestAppContext,
) {
    let cx = viewing(picture("wheel", 800, 400), cx);
    let (stage, fitted) = (
        cx.debug_bounds("image-viewer-stage")
            .expect("the stage draws"),
        shown(cx),
    );
    press_twice(stage.center(), cx);
    assert_eq!(
        shown(cx).size,
        gpui::size(px(800.0), px(400.0)),
        "a double press shows actual size"
    );
    press_twice(stage.center(), cx);
    assert_eq!(shown(cx), fitted, "and another fits it again");
    let pointer = point(stage.left() + px(100.0), stage.center().y);
    cx.simulate_event(ScrollEvent {
        position: pointer,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-100.0))),
        modifiers: Modifiers::command(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
    let grown = shown(cx);
    let expected = fitted.size.width * std::f32::consts::E;
    assert!(
        (grown.size.width - expected).abs() < px(1.0),
        "{grown:?}, not {expected:?} wide"
    );
    let under = |picture: Bounds<Pixels>| (pointer.x - picture.left()) / picture.size.width;
    assert!(
        (under(grown) - under(fitted)).abs() < 0.005,
        "the spot under the pointer stays"
    );
}

#[gpui::test]
fn a_new_picture_starts_fitted_and_unturned(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Viewing(picture("first", 200, 100)));
    cx.update(|window, _| window.activate_window());
    settle(cx);
    focus_stage(cx);
    press("=", cx);
    press("r", cx);
    assert_eq!(shown(cx).size, gpui::size(px(100.0), px(200.0)), "turned");
    view.update(cx, |view, cx| {
        view.0 = picture("second", 120, 60);
        cx.notify();
    });
    settle(cx);
    assert_eq!(
        shown(cx).size,
        gpui::size(px(120.0), px(60.0)),
        "the new one fits, as it lies"
    );
}

/// A viewer in a page that counts the scrolls reaching it.
struct Scrolled(PathBuf, Rc<Cell<usize>>);

impl Render for Scrolled {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let heard = self.1.clone();
        div()
            .id("page")
            .w(px(400.0))
            .h(px(300.0))
            .on_scroll(move |_, _, _| heard.set(heard.get() + 1))
            .child(ImageViewer::new("viewer", self.0.clone()))
    }
}

#[gpui::test]
fn a_scroll_pans_an_overhanging_picture_and_passes_on_otherwise(cx: &mut TestAppContext) {
    setup(cx);
    let heard = Rc::new(Cell::new(0));
    let (seen, path) = (heard.clone(), picture("scroll", 800, 400));
    let (_, cx) = cx.add_window_view(|_, _| Scrolled(path, seen));
    settle(cx);
    let stage = cx
        .debug_bounds("image-viewer-stage")
        .expect("the stage draws");
    let scroll = |cx: &mut VisualTestContext| {
        cx.simulate_event(ScrollEvent {
            position: stage.center(),
            delta: ScrollDelta::Pixels(point(px(-50.0), px(0.0))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
            source: ScrollSource::Pointer,
        });
        settle(cx);
    };
    let fitted = shown(cx);
    scroll(cx);
    assert_eq!(
        (shown(cx), heard.get()),
        (fitted, 1),
        "a fitted picture lets the page scroll"
    );
    press_twice(stage.center(), cx);
    let actual = shown(cx);
    scroll(cx);
    assert_eq!(
        shown(cx).origin.x,
        actual.origin.x - px(50.0),
        "an overhanging one moves"
    );
    assert_eq!(heard.get(), 1, "and keeps the scroll");
}

#[gpui::test]
fn a_picture_that_cannot_open_says_so(cx: &mut TestAppContext) {
    let cx = viewing(std::env::temp_dir().join("ely-media-missing.png"), cx);
    assert!(cx.debug_bounds("image-viewer-failed").is_some());
    assert!(cx.debug_bounds("image-viewer-picture").is_none());
    cx.update(|window, cx| window.focus_next(cx));
    let focused = cx.update(|window, cx| window.focused(cx));
    assert!(focused.is_none(), "with nothing to show, nothing takes Tab");
}

/// Two thumbnails, the second with no handler, and how often the first opened.
struct Strip(Rc<Cell<usize>>, PathBuf);

impl Render for Strip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let opened = self.0.clone();
        div()
            .flex()
            .gap_2()
            .child(
                div().w(px(120.0)).child(
                    ImageThumbnail::new("open", self.1.clone(), 1.5)
                        .label("0:42")
                        .on_click(move |_, _| opened.set(opened.get() + 1)),
                ),
            )
            .child(
                div()
                    .w(px(120.0))
                    .child(ImageThumbnail::new("still", self.1.clone(), 1.5)),
            )
    }
}

#[gpui::test]
fn only_a_thumbnail_with_a_handler_takes_tab_and_opens_on_enter(cx: &mut TestAppContext) {
    setup(cx);
    let opened = Rc::new(Cell::new(0));
    let (seen, path) = (opened.clone(), picture("thumb", 60, 40));
    let (_, cx) = cx.add_window_view(|_, _| Strip(seen, path));
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    let first = cx.update(|window, cx| window.focused(cx));
    cx.update(|window, cx| window.focus_next(cx));
    let wrapped = cx.update(|window, cx| window.focused(cx));
    assert!(
        first.is_some() && first == wrapped,
        "the still one takes no Tab"
    );
    press("enter", cx);
    assert_eq!(opened.get(), 1);
}
