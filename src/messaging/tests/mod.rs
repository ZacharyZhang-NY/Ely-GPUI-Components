use gpui::{
    Context, Entity, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Modifiers, ParentElement,
    Render, SharedString, Styled, TestAppContext, VisualTestContext, Window, div, px,
};
use jiff::{SignedDuration, Timestamp};

use super::{
    ChannelHeader, ChannelItem, ChannelList, DirectMessageItem, PinnedMessage, PinnedMessages,
};
use crate::{data_display::Presence, primitives::FocusNext, theme::Theme};

mod calls;
mod messages;
mod people;
mod pickers;

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

/// Moves focus to the `nth` Tab stop from none.
fn tab_to(nth: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        for _ in 0..nth {
            window.focus_next(cx);
        }
    });
    settle(cx);
}

/// Channels and direct messages with design open, and the chats it opened.
struct Chatting {
    opened: Vec<SharedString>,
}

impl Render for Chatting {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity();
        let channels = [
            ("general", ChannelItem::new("general", "general")),
            ("design", ChannelItem::new("design", "design").unread(3)),
            (
                "random",
                ChannelItem::new("random", "random").unread(9).muted(true),
            ),
        ];
        let people = [
            (
                "ana",
                DirectMessageItem::new("ana", "Ana Lima", Presence::Online),
            ),
            (
                "ben",
                DirectMessageItem::new("ben", "Ben Ito", Presence::Offline).unread(2),
            ),
        ];
        div().w(px(260.0)).child(
            ChannelList::new("chats")
                .section("Channels", channels)
                .section("Direct messages", people)
                .open("design")
                .on_open(move |key, _, cx| {
                    owner.update(cx, |host, _| host.opened.push(key.clone()))
                }),
        )
    }
}

#[gpui::test]
fn each_section_opens_by_key_and_starts_on_the_chat_open(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = cx.add_window_view(|_, _| Chatting { opened: Vec::new() });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    tab_to(1, cx);
    press("down", cx);
    tab_to(2, cx);
    press("down", cx);
    assert_eq!(
        host.read_with(cx, |host, _| host.opened.clone()),
        ["random", "ben"]
    );
}

#[gpui::test]
fn enter_opens_the_chat_under_the_cursor(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = cx.add_window_view(|_, _| Chatting { opened: Vec::new() });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    tab_to(2, cx);
    press("enter", cx);
    assert_eq!(host.read_with(cx, |host, _| host.opened.clone()), ["ana"]);
}

#[test]
#[should_panic(expected = "chat general twice")]
fn a_chat_is_listed_once() {
    let _ = ChannelList::new("chats")
        .section("Channels", [("general", ChannelItem::new("a", "general"))])
        .section("Starred", [("general", ChannelItem::new("b", "general"))]);
}

/// A header with members, pins and a call, and what it asked.
struct Heading {
    asked: Vec<&'static str>,
}

impl Render for Heading {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (members, pins, call) = (cx.entity(), cx.entity(), cx.entity());
        div().w(px(480.0)).child(
            ChannelHeader::new("header", "design")
                .topic("Where the atrium takes its shape")
                .members(12, move |_, cx| {
                    members.update(cx, |host, _| host.asked.push("members"))
                })
                .pins(3, move |_, cx| {
                    pins.update(cx, |host, _| host.asked.push("pins"))
                })
                .on_call(move |_, cx| call.update(cx, |host, _| host.asked.push("call"))),
        )
    }
}

#[gpui::test]
fn the_headers_actions_each_ask_their_own(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = cx.add_window_view(|_, _| Heading { asked: Vec::new() });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    for nth in 1..=3 {
        tab_to(nth, cx);
        press("space", cx);
    }
    assert_eq!(
        host.read_with(cx, |host, _| host.asked.clone()),
        ["members", "pins", "call"]
    );
}

/// Two pins, the later first, and what was asked.
struct Pinning {
    asked: Vec<String>,
}

impl Render for Pinning {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (open, unpin) = (cx.entity(), cx.entity());
        let now = Timestamp::now();
        let pin = |key: &str, hours: i64| PinnedMessage {
            key: SharedString::from(key.to_string()),
            author: "Ana Lima".into(),
            at: now - SignedDuration::from_hours(hours),
            text: SharedString::from(format!("The {key} note")),
        };
        div().w(px(360.0)).child(
            PinnedMessages::new("pins", [pin("older", 30), pin("newer", 2)])
                .on_open(move |key, _, cx| {
                    open.update(cx, |host, _| host.asked.push(format!("open {key}")))
                })
                .on_unpin(move |key, _, cx| {
                    unpin.update(cx, |host, _| host.asked.push(format!("unpin {key}")))
                }),
        )
    }
}

fn pinning(cx: &mut TestAppContext) -> (Entity<Pinning>, &mut VisualTestContext) {
    setup(cx);
    let (host, cx) = cx.add_window_view(|_, _| Pinning { asked: Vec::new() });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (host, cx)
}

#[gpui::test]
fn the_newest_pin_comes_first_and_enter_opens_it(cx: &mut TestAppContext) {
    let (host, cx) = pinning(cx);
    tab_to(1, cx);
    press("enter", cx);
    assert_eq!(
        host.read_with(cx, |host, _| host.asked.clone()),
        ["open newer"]
    );
}

#[gpui::test]
fn a_press_on_unpin_unpins_and_opens_nothing(cx: &mut TestAppContext) {
    let (host, cx) = pinning(cx);
    let unpin = cx
        .debug_bounds("unpin older")
        .expect("the older pin's unpin draws");
    cx.simulate_click(unpin.center(), Modifiers::none());
    settle(cx);
    assert_eq!(
        host.read_with(cx, |host, _| host.asked.clone()),
        ["unpin older"]
    );
}

#[gpui::test]
fn space_on_unpin_unpins_and_opens_nothing(cx: &mut TestAppContext) {
    let (host, cx) = pinning(cx);
    tab_to(2, cx);
    press("space", cx);
    assert_eq!(
        host.read_with(cx, |host, _| host.asked.clone()),
        ["unpin newer"]
    );
}

#[test]
#[should_panic(expected = "pin a twice")]
fn a_message_is_pinned_once() {
    let pin = PinnedMessage {
        key: "a".into(),
        author: "Ana".into(),
        at: Timestamp::UNIX_EPOCH,
        text: "hi".into(),
    };
    let _ = PinnedMessages::new("pins", [pin.clone(), pin]);
}

#[test]
#[should_panic(expected = "section Channels twice")]
fn a_section_is_named_once() {
    let _ = ChannelList::new("chats")
        .section("Channels", [("a", ChannelItem::new("a", "a"))])
        .section("Channels", [("b", ChannelItem::new("b", "b"))]);
}

#[test]
#[should_panic(expected = "section Starred holds no chats")]
fn a_section_holds_a_chat() {
    let _ = ChannelList::new("chats").section("Starred", Vec::<(&str, ChannelItem)>::new());
}

/// No pins, and nothing else to focus.
struct Unpinned;

impl Render for Unpinned {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(360.0))
            .child(PinnedMessages::new("pins", Vec::new()))
    }
}

#[gpui::test]
fn nothing_pinned_takes_no_tab_stop(cx: &mut TestAppContext) {
    setup(cx);
    let (_, cx) = cx.add_window_view(|_, _| Unpinned);
    cx.update(|window, _| window.activate_window());
    settle(cx);
    tab_to(1, cx);
    assert!(cx.update(|window, cx| window.focused(cx).is_none()));
}
