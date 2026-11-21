use gpui::{
    Context, IntoElement, KeyUpEvent, Keystroke, Modifiers, ParentElement, Render, SharedString,
    Styled, TestAppContext, VisualTestContext, Window, div, point, px,
};
use jiff::Timestamp;

use super::settle;
use crate::{
    chat::{Conversation, ConversationItem},
    forms,
    theme::Theme,
};

/// One conversation's row, and what it asked: opened or renamed.
struct Row {
    opened: Vec<SharedString>,
    renamed: Vec<SharedString>,
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (open, rename) = (cx.entity(), cx.entity());
        let conversation = Conversation {
            key: "lift".into(),
            title: "What a lift does".into(),
            at: Timestamp::UNIX_EPOCH,
            pinned: false,
        };
        div().w(px(300.0)).child(
            ConversationItem::new("row", conversation)
                .on_select(move |key, _, cx| open.update(cx, |row, _| row.opened.push(key.clone())))
                .actions(
                    move |key, _, cx| rename.update(cx, |row, _| row.renamed.push(key.clone())),
                    |_, _, _| {},
                    |_, _, _| {},
                ),
        )
    }
}

fn row(cx: &mut TestAppContext) -> (gpui::Entity<Row>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let (row, cx) = cx.add_window_view(|_, _| Row {
        opened: Vec::new(),
        renamed: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (row, cx)
}

/// Presses and releases `key`, a frame apart.
fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

#[gpui::test]
fn a_rows_actions_open_from_the_keyboard(cx: &mut TestAppContext) {
    let (row, cx) = row(cx);
    cx.simulate_mouse_move(point(px(10.0), px(200.0)), None, Modifiers::none());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    settle(cx);
    press("enter", cx);
    press("enter", cx);
    row.read_with(cx, |row, _| {
        assert_eq!(
            row.renamed,
            ["lift"],
            "Tab reaches the menu of the row in focus"
        );
        assert!(row.opened.is_empty());
    });
}

#[gpui::test]
fn a_press_on_the_menu_leaves_the_row_shut(cx: &mut TestAppContext) {
    let (row, cx) = row(cx);
    let (title, menu) = (point(px(60.0), px(14.0)), point(px(285.0), px(14.0)));
    cx.simulate_click(title, Modifiers::none());
    settle(cx);
    cx.simulate_mouse_move(menu, None, Modifiers::none());
    settle(cx);
    cx.simulate_click(menu, Modifiers::none());
    settle(cx);
    let opened = row.read_with(cx, |row, _| row.opened.clone());
    assert_eq!(opened, ["lift"], "only the press on the title opens it");
}
