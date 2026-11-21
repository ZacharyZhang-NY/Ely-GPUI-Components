use std::{cell::RefCell, rc::Rc, time::Duration};

use gpui::{
    AnyElement, AppContext as _, Bounds, Context, Entity, Focusable, InteractiveElement,
    IntoElement, KeyUpEvent, Keystroke, Modifiers, MouseButton, ParentElement, Pixels, Render,
    ScrollDelta, ScrollEvent, ScrollSource, Styled, TestAppContext, TouchPhase, VisualTestContext,
    Window, canvas, div, point, px,
};

mod cite;
mod compose;
mod history;
mod welcome;

use super::{
    DateSeparator, ImageGrid, ImageMessage, MessageAvatar, MessageBubble, MessageEditor,
    MessageList, Role, ThinkingBlock, VideoMessage,
};
use crate::{forms, forms::TextInput, theme::Theme};

type Seen = Rc<RefCell<Vec<usize>>>;

type Pressed = Rc<RefCell<Option<bool>>>;

/// A conversation of tall messages, noting which ones each frame drew, and whether a press landed on a button.
struct Talk {
    count: usize,
    seen: Seen,
    pressed: Pressed,
}

impl Render for Talk {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (seen, pressed) = (self.seen.clone(), self.pressed.clone());
        seen.borrow_mut().clear();
        div()
            .size_full()
            .on_mouse_down(MouseButton::Left, move |_, window, _| {
                *pressed.borrow_mut() = Some(window.default_prevented())
            })
            .child(MessageList::new("talk", self.count, move |ix, _, _| {
                seen.borrow_mut().push(ix);
                div().h(px(80.0)).w_full().into_any_element()
            }))
    }
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn newest(seen: &Seen) -> usize {
    *seen.borrow().iter().max().expect("a frame draws messages")
}

#[gpui::test]
fn the_list_follows_the_newest_until_the_reader_scrolls_away(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let seen = Seen::default();
    let noted = seen.clone();
    let (talk, cx) = cx.add_window_view(move |_, _| Talk {
        count: 40,
        seen: noted,
        pressed: Pressed::default(),
    });
    settle(cx);
    assert_eq!(newest(&seen), 39, "it opens at the newest");
    talk.update(cx, |talk, cx| {
        talk.count = 42;
        cx.notify();
    });
    settle(cx);
    assert_eq!(newest(&seen), 41, "at the newest, it follows");
    let middle = cx.update(|window, _| {
        let size = window.viewport_size();
        point(size.width / 2.0, size.height / 2.0)
    });
    cx.simulate_event(ScrollEvent {
        position: middle,
        delta: ScrollDelta::Pixels(point(px(0.0), px(1200.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
    let away = newest(&seen);
    assert!(away < 41, "scrolled up to {away}");
    talk.update(cx, |talk, cx| {
        talk.count = 45;
        cx.notify();
    });
    settle(cx);
    assert_eq!(newest(&seen), away, "away, it holds its place");
    let button = cx.update(|window, _| {
        let size = window.viewport_size();
        point(size.width / 2.0, size.height - px(28.0))
    });
    cx.simulate_click(button, Modifiers::none());
    settle(cx);
    assert_eq!(newest(&seen), 44, "the button goes back to the newest");
}

/// A sent message open to change, and what the reader asked.
struct Edit {
    field: Entity<TextInput>,
    saved: usize,
    cancelled: usize,
}

impl Render for Edit {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (save, cancel) = (cx.entity(), cx.entity());
        div().size_full().child(MessageEditor::new(
            "edit",
            &self.field,
            "Lift blends toward white.",
            move |_, cx| save.update(cx, |edit, _| edit.saved += 1),
            move |_, cx| cancel.update(cx, |edit, _| edit.cancelled += 1),
        ))
    }
}

#[gpui::test]
fn an_edit_saves_only_a_change_and_escape_cancels(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let (edit, cx) = cx.add_window_view(|window, cx| Edit {
        field: cx.new(|cx| {
            let mut input = TextInput::new(window, cx);
            input.set_text("Lift blends toward white.", cx);
            input
        }),
        saved: 0,
        cancelled: 0,
    });
    cx.update(|window, cx| {
        window.activate_window();
        let field = edit.read(cx).field.focus_handle(cx);
        window.focus(&field, cx);
    });
    settle(cx);
    cx.simulate_keystrokes("secondary-enter");
    settle(cx);
    assert_eq!(
        edit.read_with(cx, |edit, _| edit.saved),
        0,
        "nothing changed"
    );
    cx.simulate_input(" Gently.");
    settle(cx);
    cx.simulate_keystrokes("secondary-enter");
    settle(cx);
    assert_eq!(edit.read_with(cx, |edit, _| edit.saved), 1);
    cx.simulate_keystrokes("escape");
    assert_eq!(edit.read_with(cx, |edit, _| edit.cancelled), 1);
}

#[gpui::test]
fn the_editor_takes_focus_as_it_opens(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let (edit, cx) = cx.add_window_view(|window, cx| Edit {
        field: cx.new(|cx| TextInput::new(window, cx)),
        saved: 0,
        cancelled: 0,
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(
        edit.read_with(cx, |edit, _| edit.cancelled),
        1,
        "Escape reaches the editor"
    );
}

pub(super) type Measured = Rc<RefCell<Option<Bounds<Pixels>>>>;

/// A long message of yours in a narrow column, its body's box measured.
struct Narrow {
    body: Measured,
}

impl Render for Narrow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let body = self.body.clone();
        div().size_full().child(
            div().w(px(360.0)).child(
                MessageBubble::new(Role::User)
                    .avatar(MessageAvatar::new("me", Role::User, "Ada Park"))
                    .child("How far does a lift move a saturated accent on charcoal compared with the same accent on paper, and why?")
                    .child(
                        canvas(move |bounds, _, _| *body.borrow_mut() = Some(bounds), |_, _, _, _| {})
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
fn a_long_message_of_yours_wraps_within_its_column(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let body = Measured::default();
    let seen = body.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Narrow { body: seen });
    settle(cx);
    let bounds = body.borrow().expect("the body is laid out");
    assert!(
        bounds.left() >= px(0.0),
        "it starts inside the column: {bounds:?}"
    );
    assert!(
        bounds.right() <= px(360.0),
        "it ends inside the column: {bounds:?}"
    );
}

#[gpui::test]
fn a_list_that_fits_again_drops_the_way_down(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let pressed = Pressed::default();
    let noted = pressed.clone();
    let (talk, cx) = cx.add_window_view(move |_, _| Talk {
        count: 40,
        seen: Seen::default(),
        pressed: noted,
    });
    settle(cx);
    let (middle, button) = cx.update(|window, _| {
        let size = window.viewport_size();
        (
            point(size.width / 2.0, size.height / 2.0),
            point(size.width / 2.0, size.height - px(28.0)),
        )
    });
    cx.simulate_event(ScrollEvent {
        position: middle,
        delta: ScrollDelta::Pixels(point(px(0.0), px(1200.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
    cx.simulate_click(button, Modifiers::none());
    assert_eq!(*pressed.borrow(), Some(true), "away, the way down shows");
    cx.simulate_event(ScrollEvent {
        position: middle,
        delta: ScrollDelta::Pixels(point(px(0.0), px(1200.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
    talk.update(cx, |talk, cx| {
        talk.count = 2;
        cx.notify();
    });
    settle(cx);
    settle(cx);
    cx.simulate_click(button, Modifiers::none());
    assert_eq!(*pressed.borrow(), Some(false), "all in view, no way down");
}

/// A wide picture in a narrow column, its box measured.
struct Picture {
    frame: Measured,
}

impl Render for Picture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let frame = self.frame.clone();
        div().size_full().child(
            div()
                .w(px(240.0))
                .relative()
                .child(ImageMessage::new("wide", "wide.png", 960.0, 640.0))
                .child(
                    canvas(
                        move |bounds, _, _| *frame.borrow_mut() = Some(bounds),
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
fn a_picture_fits_a_narrow_column_in_its_own_shape(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let frame = Measured::default();
    let seen = frame.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Picture { frame: seen });
    settle(cx);
    let bounds = frame.borrow().expect("the column is laid out");
    assert_eq!(bounds.size.height, px(160.0), "240 wide at 3:2 is 160 tall");
}

/// A message in a narrow flex column, the column measured.
struct Column {
    frame: Measured,
    message: fn() -> AnyElement,
}

impl Render for Column {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let frame = self.frame.clone();
        div().size_full().child(
            div()
                .w(px(240.0))
                .relative()
                .flex()
                .flex_col()
                .child((self.message)())
                .child(
                    canvas(
                        move |bounds, _, _| *frame.borrow_mut() = Some(bounds),
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

fn column_height(message: fn() -> AnyElement, cx: &mut TestAppContext) -> Pixels {
    let frame = Measured::default();
    let seen = frame.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Column {
        frame: seen,
        message,
    });
    settle(cx);
    frame.borrow().expect("the column is laid out").size.height
}

#[gpui::test]
fn a_video_and_a_grid_fit_a_narrow_flex_column_in_their_shape(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let video = || {
        VideoMessage::new("video", "dunes.jpg", 960.0, 640.0, Duration::from_secs(84))
            .into_any_element()
    };
    assert_eq!(column_height(video, cx), px(160.0), "240 wide at 3:2");
    let grid = || ImageGrid::new("grid", ["a.jpg", "b.jpg"]).into_any_element();
    assert_eq!(
        column_height(grid, cx),
        px(120.0),
        "two side by side at 2:1"
    );
}

/// Reasoning folded under its time, the block's box measured.
struct Reasoning {
    frame: Measured,
}

impl Render for Reasoning {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let frame = self.frame.clone();
        div().size_full().child(
            div()
                .w(px(320.0))
                .relative()
                .child(ThinkingBlock::new(
                    "thought",
                    "Start from the definition.",
                    false,
                    std::time::Duration::from_secs(3),
                ))
                .child(
                    canvas(
                        move |bounds, _, _| *frame.borrow_mut() = Some(bounds),
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
fn the_reasoning_opens_from_the_keyboard(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let frame = Measured::default();
    let seen = frame.clone();
    let (_, cx) = cx.add_window_view(move |_, _| Reasoning { frame: seen });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    let closed = frame.borrow().expect("the block is laid out").size.height;
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("space");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("space").expect("space parses"),
    });
    settle(cx);
    let open = frame.borrow().expect("the block is laid out").size.height;
    assert!(
        open > closed,
        "Tab and Space open it: {closed:?} to {open:?}"
    );
}

struct Dated;

impl Render for Dated {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let day = jiff::civil::date(2026, 9, 28);
        div()
            .size_full()
            .child(MessageList::new("dated", 1, move |_, _, _| {
                DateSeparator::new(day, day).into_any_element()
            }))
    }
}

/// A row no wider than its words still spans the list.
#[gpui::test]
fn a_narrow_row_spans_the_list(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Dated);
    settle(cx);
    let row = cx
        .debug_bounds("date-separator-Today")
        .expect("the separator draws");
    let width = cx.update(|window, _| window.viewport_size().width);
    assert_eq!(row.size.width, width);
}
