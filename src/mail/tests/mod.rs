use gpui::{
    Context, Entity, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Modifiers, ParentElement,
    Render, Styled, TestAppContext, VisualTestContext, Window, div, px,
};
use jiff::{Timestamp, civil::date, tz::TimeZone};

use super::{Mail, MailList, Mailbox, MailboxList};
use crate::{
    primitives::{FocusNext, IconName},
    theme::Theme,
};

mod read;
mod sort;
mod write;

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

/// A view that shows one mail part and keeps what it heard.
struct Mailing {
    part: fn(Entity<Mailing>) -> gpui::AnyElement,
    heard: Vec<String>,
}

impl Render for Mailing {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(360.0)).child((self.part)(cx.entity()))
    }
}

fn mailing(
    part: fn(Entity<Mailing>) -> gpui::AnyElement,
    cx: &mut TestAppContext,
) -> (Entity<Mailing>, &mut VisualTestContext) {
    setup(cx);
    let (host, cx) = cx.add_window_view(|_, _| Mailing {
        part,
        heard: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (host, cx)
}

fn note(owner: &Entity<Mailing>, what: String, cx: &mut gpui::App) {
    owner.update(cx, |host, cx| {
        host.heard.push(what);
        cx.notify();
    });
}

fn heard(host: &Entity<Mailing>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |host, _| host.heard.clone())
}

fn boxes(owner: Entity<Mailing>) -> gpui::AnyElement {
    MailboxList::new("boxes")
        .section(
            "Mailboxes",
            [
                Mailbox::new("inbox", "Inbox", IconName::Inbox).unread(3),
                Mailbox::new("starred", "Starred", IconName::Star),
                Mailbox::new("sent", "Sent", IconName::Send),
            ],
        )
        .section(
            "Labels",
            [
                Mailbox::label("atrium", "Atrium", 0),
                Mailbox::label("clients", "Clients", 3),
            ],
        )
        .open("inbox")
        .on_open(move |key, _, cx| note(&owner, format!("open {key}"), cx))
        .into_any_element()
}

#[gpui::test]
fn an_arrow_or_enter_opens_a_box(cx: &mut TestAppContext) {
    let (host, cx) = mailing(boxes, cx);
    tab_to(1, cx);
    press("down", cx);
    tab_to(2, cx);
    press("enter", cx);
    assert_eq!(heard(&host, cx), ["open starred", "open atrium"]);
}

#[test]
#[should_panic(expected = "mailbox inbox twice")]
fn a_box_is_listed_once() {
    let _ = MailboxList::new("boxes")
        .section("A", [Mailbox::new("inbox", "Inbox", IconName::Inbox)])
        .section("B", [Mailbox::label("inbox", "Inbox again", 1)]);
}

#[test]
#[should_panic(expected = "section Labels holds no mailbox")]
fn a_section_holds_a_box() {
    let _ = MailboxList::new("boxes").section("Labels", Vec::new());
}

#[gpui::test]
#[should_panic(expected = "no chart hue 9")]
fn a_label_takes_one_of_the_chart_hues(cx: &mut TestAppContext) {
    let _ = mailing(
        |_| {
            MailboxList::new("boxes")
                .section("Labels", [Mailbox::label("odd", "Odd", 9)])
                .into_any_element()
        },
        cx,
    );
}

fn at(minute: i8) -> Timestamp {
    date(2026, 9, 26)
        .at(9, minute, 0, 0)
        .to_zoned(TimeZone::UTC)
        .expect("a UTC time")
        .timestamp()
}

fn mails(owner: Entity<Mailing>) -> gpui::AnyElement {
    let (select, star) = (owner.clone(), owner);
    MailList::new(
        "mails",
        [
            Mail::new("m1", "Ana Lima", "Friday's review", at(40)).unread(true),
            Mail::new("m2", "Ben Ito", "Samples", at(20)).attachments(2),
            Mail::new("m3", "Chloé Martin", "The brief", at(5)).starred(true),
        ],
    )
    .selected(["m1"])
    .zone(TimeZone::UTC)
    .on_select(move |keys, _, cx| {
        let keys: Vec<&str> = keys.iter().map(|key| key.as_ref()).collect();
        note(&select, format!("select {}", keys.join(" ")), cx)
    })
    .on_star(move |key, on, _, cx| note(&star, format!("star {key} {on}"), cx))
    .into_any_element()
}

#[gpui::test]
fn an_arrow_moves_the_selection_down_the_list(cx: &mut TestAppContext) {
    let (host, cx) = mailing(mails, cx);
    tab_to(1, cx);
    press("down", cx);
    assert_eq!(heard(&host, cx), ["select m2"]);
}

#[gpui::test]
fn a_star_toggles_and_leaves_the_selection(cx: &mut TestAppContext) {
    let (host, cx) = mailing(mails, cx);
    let star = cx.debug_bounds("star m3").expect("the third star draws");
    cx.simulate_click(star.center(), Modifiers::none());
    settle(cx);
    assert_eq!(heard(&host, cx), ["star m3 false"]);
}

#[gpui::test]
fn space_on_a_star_stars_and_leaves_the_selection(cx: &mut TestAppContext) {
    let (host, cx) = mailing(mails, cx);
    tab_to(2, cx);
    press("space", cx);
    tab_to(2, cx);
    press("enter", cx);
    assert_eq!(heard(&host, cx), ["star m1 true", "star m1 true"]);
}

#[gpui::test]
fn no_mail_says_so_and_takes_no_tab_stop(cx: &mut TestAppContext) {
    let (_, cx) = mailing(
        |_| MailList::new("mails", Vec::new()).into_any_element(),
        cx,
    );
    assert!(cx.debug_bounds("mail-none").is_some());
    tab_to(1, cx);
    assert!(cx.update(|window, cx| window.focused(cx).is_none()));
}

#[test]
#[should_panic(expected = "mail m1 twice")]
fn a_mail_is_listed_once() {
    let mail = Mail::new("m1", "Ana", "Hi", Timestamp::UNIX_EPOCH);
    let _ = MailList::new("mails", [mail.clone(), mail]);
}
