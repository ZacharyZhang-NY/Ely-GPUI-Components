use std::{cell::RefCell, rc::Rc};

use gpui::{
    AppContext as _, Bounds, Context, Entity, Focusable, IntoElement, Modifiers, ParentElement,
    Pixels, Point, Render, ScrollDelta, ScrollEvent, ScrollSource, Styled, TestAppContext,
    TouchPhase, VisualTestContext, Window, div, point, px, size,
};

use super::{DocPage, DocumentViewer, PageHit, ZenMode};
use crate::{
    editor::FindWidget,
    forms::{Input, TextInput},
    theme::{ActiveTheme, ControlSize, Theme},
};

type Noted = Rc<RefCell<Option<(usize, Point<f32>)>>>;

/// A hit on `page`, 400 points down.
fn hit(page: usize) -> PageHit {
    PageHit {
        page,
        bounds: Bounds::new(point(40.0, 400.0), size(60.0, 12.0)),
    }
}

/// Three tall pages and the hits found; a press while pinning records the note asked.
struct Reader {
    query: Entity<TextInput>,
    width: f32,
    hits: Vec<PageHit>,
    current: Option<usize>,
    noted: Noted,
}

impl Render for Reader {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pages: Vec<DocPage> = (0..3)
            .map(|ix| DocPage {
                source: format!("page-{ix}.png").into(),
                width: self.width,
                height: 1000.0,
            })
            .collect();
        let noted = self.noted.clone();
        div().size_full().child(
            DocumentViewer::new("viewer", "lift.pdf", pages)
                .find(
                    FindWidget::new("find", &self.query, self.current, self.hits.len()),
                    self.hits.clone(),
                    self.current,
                )
                .notes(Vec::new(), move |page, at, _, _| {
                    *noted.borrow_mut() = Some((page, at))
                }),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<Reader>, Noted, &mut VisualTestContext) {
    open_with(400.0, None, cx)
}

/// Pages `width` points wide, with a hit on the second, current from the first frame when `current` says.
fn open_with(
    width: f32,
    current: Option<usize>,
    cx: &mut TestAppContext,
) -> (Entity<Reader>, Noted, &mut VisualTestContext) {
    cx.update(Theme::init);
    let noted = Noted::default();
    let seen = noted.clone();
    let (reader, cx) = cx.add_window_view(move |window, cx| Reader {
        query: cx.new(|cx| TextInput::new(window, cx)),
        width,
        hits: vec![hit(1)],
        current,
        noted: seen,
    });
    settle(cx);
    (reader, noted, cx)
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// A point on the pages, below the header and the find bar, at the view's middle.
fn on_pages(cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.update(|window, _| point(window.viewport_size().width / 2.0, px(300.0)))
}

fn wheel(down: f32, cx: &mut VisualTestContext) {
    let at = on_pages(cx);
    cx.simulate_event(ScrollEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-down))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
}

/// Turns pinning on with the header's last button, then presses the pages at their middle; returns the note asked.
fn pin(cx: &mut VisualTestContext, noted: &Noted) -> (usize, Point<f32>) {
    let at = on_pages(cx);
    pin_at(at, cx, noted)
}

fn pin_at(at: Point<Pixels>, cx: &mut VisualTestContext, noted: &Noted) -> (usize, Point<f32>) {
    let toggle = cx.update(|window, cx| {
        let header = cx
            .theme()
            .control_height(ControlSize::Lg)
            .to_pixels(window.rem_size());
        point(
            window.viewport_size().width - px(17.0),
            px(1.0) + header / 2.0,
        )
    });
    cx.simulate_click(toggle, Modifiers::none());
    settle(cx);
    cx.simulate_click(at, Modifiers::none());
    settle(cx);
    noted
        .borrow_mut()
        .take()
        .expect("a press while pinning asks for a note")
}

#[gpui::test]
fn a_note_lands_where_the_press_is_on_a_scrolled_page(cx: &mut TestAppContext) {
    let (_, noted, cx) = open(cx);
    let (page, top) = pin(cx, &noted);
    wheel(150.0, cx);
    let at = on_pages(cx);
    cx.simulate_click(at, Modifiers::none());
    settle(cx);
    let (scrolled_page, scrolled) = noted.borrow_mut().take().expect("pinning stays on");
    assert_eq!((scrolled_page, page), (0, 0));
    assert_eq!(scrolled.x, top.x);
    assert_eq!(scrolled.y, top.y + 150.0, "the press counts the scroll");
}

/// Scrolls `down`, makes the hit current, then pins a note at the same window point.
fn revealed(down: f32, cx: &mut TestAppContext) -> (usize, Point<f32>) {
    let (reader, noted, cx) = open(cx);
    if down > 0.0 {
        wheel(down, cx);
    }
    reader.update(cx, |reader, cx| {
        reader.current = Some(0);
        cx.notify();
    });
    settle(cx);
    pin(cx, &noted)
}

#[gpui::test]
fn a_current_hit_comes_to_one_place_from_any_scroll(cx: &mut TestAppContext) {
    let from_top = revealed(0.0, cx);
    let from_below = revealed(300.0, cx);
    assert_eq!(from_top.0, 1, "the hit's page comes into view");
    assert_eq!(from_top, from_below);
}

#[gpui::test]
fn a_page_wider_than_the_view_starts_at_its_left_edge(cx: &mut TestAppContext) {
    let (_, noted, cx) = open_with(3000.0, None, cx);
    let (page, at) = pin_at(point(px(60.0), px(300.0)), cx, &noted);
    assert_eq!(page, 0);
    assert!(
        at.x < 100.0,
        "the page's left edge is in reach, not at {}",
        at.x
    );
}

#[gpui::test]
fn a_new_hit_at_the_same_index_moves_the_view(cx: &mut TestAppContext) {
    let (reader, noted, cx) = open(cx);
    reader.update(cx, |reader, cx| {
        reader.current = Some(0);
        cx.notify();
    });
    settle(cx);
    reader.update(cx, |reader, cx| {
        reader.hits = vec![hit(2)];
        cx.notify();
    });
    settle(cx);
    assert_eq!(pin(cx, &noted).0, 2, "the view follows the new hit");
}

#[gpui::test]
fn a_hit_current_from_the_first_frame_comes_into_view(cx: &mut TestAppContext) {
    let (_, noted, first) = open_with(400.0, Some(0), cx);
    let at_once = pin(first, &noted);
    let later = revealed(0.0, cx);
    assert_eq!(at_once, later);
}

/// A field outside a zen view, and how often Escape left zen.
struct Zen {
    outside: Entity<TextInput>,
    on: bool,
    left: usize,
}

impl Render for Zen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity();
        div().size_full().child(Input::new(&self.outside)).child(
            ZenMode::new("zen", self.on)
                .on_exit(move |_, cx| {
                    me.update(cx, |zen, cx| {
                        zen.on = false;
                        zen.left += 1;
                        cx.notify();
                    })
                })
                .child(div().child("A quiet page")),
        )
    }
}

#[gpui::test]
fn escape_leaves_zen_turned_on_from_outside_and_hands_focus_back(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
    });
    let (zen, cx) = cx.add_window_view(|window, cx| Zen {
        outside: cx.new(|cx| TextInput::new(window, cx)),
        on: false,
        left: 0,
    });
    cx.update(|window, cx| {
        window.activate_window();
        let outside = zen.read(cx).outside.focus_handle(cx);
        window.focus(&outside, cx);
    });
    settle(cx);
    zen.update(cx, |zen, cx| {
        zen.on = true;
        cx.notify();
    });
    settle(cx);
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert_eq!(zen.read_with(cx, |zen, _| zen.left), 1);
    let back = cx.update(|window, cx| zen.read(cx).outside.focus_handle(cx).is_focused(window));
    assert!(back, "focus returns to where it was");
}

#[gpui::test]
fn hits_past_the_pages_are_left_and_the_current_follows(cx: &mut TestAppContext) {
    let (reader, _, cx) = open_with(400.0, None, cx);
    reader.update(cx, |reader, cx| {
        reader.hits = vec![hit(7), hit(2), hit(9), hit(1)];
        reader.current = Some(3);
        cx.notify();
    });
    settle(cx);
    reader.update(cx, |reader, cx| {
        reader.current = Some(12);
        cx.notify();
    });
    settle(cx);
    let (_, _, cx) = open_with(400.0, Some(usize::MAX), cx);
    settle(cx);
}

/// Versions whose selected one left with a shorter history.
struct Versions(Vec<super::Version>, usize);

impl Render for Versions {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(super::VersionHistory::new(
            "history",
            self.0.clone(),
            self.1,
        ))
    }
}

#[gpui::test]
fn a_selected_version_past_the_history_selects_none(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let version = super::Version {
        at: jiff::Timestamp::UNIX_EPOCH,
        author: "Ann".into(),
        markdown: "# Plan".into(),
    };
    let (view, cx) = cx.add_window_view(|_, _| Versions(vec![version], 4));
    settle(cx);
    view.update(cx, |view, cx| {
        *view = Versions(Vec::new(), usize::MAX);
        cx.notify();
    });
    settle(cx);
}
