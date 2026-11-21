use std::{cell::Cell, rc::Rc};

use gpui::{
    Context, IntoElement, KeyBinding, Modifiers, ParentElement, Pixels, Render, ScrollDelta,
    ScrollEvent, ScrollSource, Styled, TestAppContext, TouchPhase, Window, canvas, div, point, px,
};

use super::{press, settle};
use crate::{
    agent::{ArtifactPanel, BrowserPreview, ComputerUseViewer, LivePreview},
    forms,
    layout::tests::narrow_width,
    primitives::{FocusNext, IconName},
    theme::{ActiveTheme, AvatarSize, Theme},
};

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
}

/// A browser with three frames, and the frames asked for.
struct Browsing {
    asked: Vec<usize>,
}

impl Render for Browsing {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        div().w(px(480.0)).child(
            BrowserPreview::new("browser", "localhost:5173", ["a.png", "b.png", "c.png"], 2)
                .on_show(move |ix, _, cx| view.update(cx, |browsing, _| browsing.asked.push(ix))),
        )
    }
}

#[gpui::test]
fn a_frame_shows_from_the_keyboard(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Browsing { asked: Vec::new() });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    assert_eq!(
        view.read_with(cx, |browsing, _| browsing.asked.clone()),
        [1]
    );
}

/// A browser asked to show a frame it no longer holds.
struct Past;

impl Render for Past {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(480.0)).child(BrowserPreview::new(
            "browser",
            "localhost",
            ["a.png", "b.png", "c.png"],
            3,
        ))
    }
}

#[gpui::test]
fn a_frame_past_the_last_shows_none(cx: &mut TestAppContext) {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Past);
    settle(cx);
}

#[test]
#[should_panic(expected = "a pointer inside the frame")]
fn a_pointer_off_the_frame_fails_loud() {
    let _ = ComputerUseViewer::new("screen", "a.png").pointer(point(1.2, 0.5));
}

/// A panel with a preview and a source, noting which one drew.
struct Panel {
    drew_source: Rc<Cell<bool>>,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let drew = self.drew_source.clone();
        div().w(px(480.0)).h(px(320.0)).child(
            ArtifactPanel::new("panel", "Notes", IconName::FileText)
                .preview(div().h(px(40.0)))
                .source(
                    div().relative().h(px(40.0)).child(
                        canvas(move |_, _, _| drew.set(true), |_, _, _, _| {})
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                    ),
                ),
        )
    }
}

#[gpui::test]
fn the_source_shows_when_asked(cx: &mut TestAppContext) {
    setup(cx);
    let drew = Rc::new(Cell::new(false));
    let seen = drew.clone();
    let (_, cx) = cx.add_window_view(|_, _| Panel { drew_source: seen });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    assert!(!drew.get(), "the preview shows first");
    cx.update(|window, cx| window.focus_next(cx));
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    assert!(drew.get(), "the source shows once asked");
}

/// A narrow browser with eight frames, its height as drawn, and the frames asked for.
struct Strip {
    height: Rc<Cell<Pixels>>,
    asked: Vec<usize>,
}

impl Render for Strip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (view, height) = (cx.entity(), self.height.clone());
        let frames = (0..8).map(|ix| format!("{ix}.png"));
        // gpui fills the window with an auto-sized root, so the measured box sits inside one.
        div().child(
            div()
                .relative()
                .w(px(280.0))
                .child(
                    BrowserPreview::new("strip", "localhost", frames, 0)
                        .on_show(move |ix, _, cx| view.update(cx, |strip, _| strip.asked.push(ix))),
                )
                .child(
                    canvas(
                        move |bounds, _, _| height.set(bounds.size.height),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                ),
        )
    }
}

#[gpui::test]
fn a_narrow_strip_scrolls_to_its_last_frame(cx: &mut TestAppContext) {
    setup(cx);
    let height = Rc::new(Cell::new(Pixels::ZERO));
    let seen = height.clone();
    let (view, cx) = cx.add_window_view(|_, _| Strip {
        height: seen,
        asked: Vec::new(),
    });
    settle(cx);
    let thumb = cx.update(|window, cx| {
        cx.theme()
            .avatar_size(AvatarSize::Lg)
            .to_pixels(window.rem_size())
    });
    let row = height.get() - px(9.0) - thumb / 2.0;
    cx.simulate_event(ScrollEvent {
        position: point(px(140.0), row),
        delta: ScrollDelta::Pixels(point(px(-2000.0), px(0.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
    let last = point(px(280.0 - 24.0), row);
    cx.simulate_mouse_move(last, None, Modifiers::none());
    cx.simulate_click(last, Modifiers::none());
    settle(cx);
    assert_eq!(view.read_with(cx, |strip, _| strip.asked.clone()), [7]);
}

/// A panel with every control at `width`, noting where its body starts.
struct Crowded {
    width: Pixels,
    top: Rc<Cell<Pixels>>,
}

impl Render for Crowded {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let top = self.top.clone();
        div().w(self.width).h(px(320.0)).child(
            ArtifactPanel::new("crowded", "Notes on lift", IconName::FileText)
                .version(1, 3, |_, _, _| {})
                .preview(
                    div().relative().h(px(40.0)).child(
                        canvas(move |bounds, _, _| top.set(bounds.top()), |_, _, _, _| {})
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full(),
                    ),
                )
                .source(div().h(px(40.0)))
                .on_copy(|_, _| {})
                .on_download(|_, _| {}),
        )
    }
}

#[gpui::test]
fn a_narrow_panel_wraps_its_header(cx: &mut TestAppContext) {
    setup(cx);
    let top = Rc::new(Cell::new(Pixels::ZERO));
    let seen = top.clone();
    let (view, cx) = cx.add_window_view(|_, _| Crowded {
        width: px(640.0),
        top: seen,
    });
    settle(cx);
    let wide = top.get();
    view.update(cx, |crowded, cx| {
        crowded.width = px(280.0);
        cx.notify();
    });
    settle(cx);
    assert!(
        top.get() > wide,
        "the header wraps below at 280px: {:?} vs {wide:?}",
        top.get()
    );
}

#[gpui::test]
fn tab_brings_a_hidden_frame_into_view(cx: &mut TestAppContext) {
    setup(cx);
    let height = Rc::new(Cell::new(Pixels::ZERO));
    let seen = height.clone();
    let (view, cx) = cx.add_window_view(|_, _| Strip {
        height: seen,
        asked: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    for _ in 0..8 {
        cx.update(|window, cx| window.focus_next(cx));
        settle(cx);
    }
    settle(cx);
    let thumb = cx.update(|window, cx| {
        cx.theme()
            .avatar_size(AvatarSize::Lg)
            .to_pixels(window.rem_size())
    });
    let last = point(px(280.0 - 24.0), height.get() - px(9.0) - thumb / 2.0);
    cx.simulate_mouse_move(last, None, Modifiers::none());
    cx.simulate_click(last, Modifiers::none());
    settle(cx);
    assert_eq!(
        view.read_with(cx, |strip, _| strip.asked.clone()),
        [7],
        "the eighth frame sits in view"
    );
}

/// A 4:3 screen in a 480px column, and its height as drawn.
struct Screen {
    height: Rc<Cell<Pixels>>,
}

impl Render for Screen {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let height = self.height.clone();
        div().child(
            div()
                .relative()
                .w(px(480.0))
                .child(
                    ComputerUseViewer::new("screen", "a.png")
                        .ratio(4.0 / 3.0)
                        .pointer(point(0.5, 0.95)),
                )
                .child(
                    canvas(
                        move |bounds, _, _| height.set(bounds.size.height),
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                ),
        )
    }
}

#[gpui::test]
fn a_screen_takes_its_pictures_shape(cx: &mut TestAppContext) {
    setup(cx);
    let height = Rc::new(Cell::new(Pixels::ZERO));
    let seen = height.clone();
    let (_, cx) = cx.add_window_view(|_, _| Screen { height: seen });
    settle(cx);
    let expected = px(478.0 * 3.0 / 4.0 + 2.0);
    assert!(
        (height.get() - expected).abs() < px(1.0),
        "478px inside the border at 4:3, plus the border: {:?} vs {expected:?}",
        height.get()
    );
}

#[gpui::test]
fn previews_fill_a_column_their_block_measures_by_content(cx: &mut TestAppContext) {
    setup(cx);
    let widths = [
        narrow_width(cx, "preview-root", |_, _| {
            BrowserPreview::new("browser", "localhost", ["a.png"], 0).into_any_element()
        }),
        narrow_width(cx, "preview-root", |_, _| {
            ComputerUseViewer::new("screen", "a.png").into_any_element()
        }),
        narrow_width(cx, "preview-root", |_, _| {
            LivePreview::new("live", "localhost", div()).into_any_element()
        }),
    ];
    assert_eq!(
        widths,
        [px(240.0); 3],
        "each preview spans the card inside its padding"
    );
}
