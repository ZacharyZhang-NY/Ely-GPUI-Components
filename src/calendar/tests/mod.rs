use gpui::{
    Context, Entity, IntoElement, KeyBinding, KeyUpEvent, Keystroke, Modifiers, MouseButton,
    MouseDownEvent, MouseUpEvent, ParentElement, Render, Styled, TestAppContext, VisualTestContext,
    Window, div, px,
};
use jiff::{Timestamp, civil::date, tz::TimeZone};

use super::{CalendarMonthView, Event, YearView};

mod editing;
mod people;
mod times;
use crate::{primitives::FocusNext, theme::Theme};

/// A view that shows one calendar part, keeps the month it was told, and what it heard.
struct Planning {
    part: fn(&Planning, Entity<Planning>) -> gpui::AnyElement,
    month: jiff::civil::Date,
    heard: Vec<String>,
}

impl Render for Planning {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(900.0)).child((self.part)(self, cx.entity()))
    }
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn planning(
    part: fn(&Planning, Entity<Planning>) -> gpui::AnyElement,
    cx: &mut TestAppContext,
) -> (Entity<Planning>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Planning {
        part,
        month: date(2026, 9, 1),
        heard: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (host, cx)
}

fn note(owner: &Entity<Planning>, what: String, cx: &mut gpui::App) {
    owner.update(cx, |host, cx| {
        host.heard.push(what);
        cx.notify();
    });
}

fn heard(host: &Entity<Planning>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |host, _| host.heard.clone())
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

fn at(day: i8, hour: i8) -> Timestamp {
    date(2026, 9, day)
        .at(hour, 0, 0, 0)
        .to_zoned(TimeZone::UTC)
        .expect("a UTC time")
        .timestamp()
}

/// Five meetings on the 16th, and a trip across the weekend after it.
fn events() -> Vec<Event> {
    let mut events: Vec<Event> = (0..5)
        .map(|ix| {
            let hour = 9 + ix as i8;
            Event::timed(
                format!("m{ix}"),
                format!("Meeting {ix}"),
                at(16, hour),
                at(16, hour + 1),
                ix,
            )
        })
        .collect();
    events.push(Event::all_day(
        "trip",
        "Trip",
        date(2026, 9, 18),
        date(2026, 9, 22),
        5,
    ));
    events
}

fn month(_: &Planning, owner: Entity<Planning>) -> gpui::AnyElement {
    let (days, keys, months) = (owner.clone(), owner.clone(), owner);
    CalendarMonthView::new("month", date(2026, 9, 1), events())
        .zone(TimeZone::UTC)
        .today(date(2026, 9, 16))
        .on_day(move |day, _, cx| note(&days, format!("open {day}"), cx))
        .on_event(move |key, _, cx| note(&keys, format!("event {key}"), cx))
        .on_month(move |first, _, cx| note(&months, format!("month {first}"), cx))
        .into_any_element()
}

#[gpui::test]
fn arrows_move_the_day_and_enter_opens_it(cx: &mut TestAppContext) {
    let (host, cx) = planning(month, cx);
    tab_to(4, cx);
    press("right", cx);
    press("down", cx);
    press("enter", cx);
    assert_eq!(
        heard(&host, cx),
        ["open 2026-09-24"],
        "from today, the 16th"
    );
}

#[gpui::test]
fn page_down_turns_the_month(cx: &mut TestAppContext) {
    let (host, cx) = planning(month, cx);
    tab_to(4, cx);
    press("pagedown", cx);
    press("enter", cx);
    assert_eq!(heard(&host, cx), ["month 2026-10-01", "open 2026-10-16"]);
}

#[gpui::test]
fn a_crowded_day_opens_from_its_more(cx: &mut TestAppContext) {
    let (host, cx) = planning(month, cx);
    let more = cx
        .debug_bounds("more 2026-09-16")
        .expect("the 16th holds more than fits");
    cx.simulate_click(more.center(), Modifiers::none());
    settle(cx);
    assert_eq!(heard(&host, cx), ["open 2026-09-16"]);
}

#[gpui::test]
fn a_chip_names_its_event_and_leaves_the_day(cx: &mut TestAppContext) {
    let (host, cx) = planning(month, cx);
    let chip = cx
        .debug_bounds("event-chip m0")
        .expect("the first meeting shows");
    let origin = chip.origin + gpui::point(px(4.0), px(4.0));
    for click_count in [1, 2] {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: origin,
            modifiers: Modifiers::none(),
            click_count,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent {
            button: MouseButton::Left,
            position: origin,
            modifiers: Modifiers::none(),
            click_count,
        });
        settle(cx);
    }
    assert_eq!(
        heard(&host, cx),
        ["event m0", "event m0"],
        "a double press on a chip does not open its day"
    );
}

#[test]
#[should_panic(expected = "event m0 twice")]
fn an_event_is_listed_once() {
    let twin = Event::timed("m0", "Again", at(1, 9), at(1, 10), 0);
    let _ = CalendarMonthView::new("month", date(2026, 9, 1), [events()[0].clone(), twin]);
}

fn year(_: &Planning, owner: Entity<Planning>) -> gpui::AnyElement {
    let (days, years) = (owner.clone(), owner);
    YearView::new("year", 2026, events())
        .zone(TimeZone::UTC)
        .today(date(2026, 9, 16))
        .on_day(move |day, _, cx| note(&days, format!("open {day}"), cx))
        .on_year(move |year, _, cx| note(&years, format!("year {year}"), cx))
        .into_any_element()
}

#[gpui::test]
fn a_year_walks_by_day_and_month_into_the_next(cx: &mut TestAppContext) {
    let (host, cx) = planning(year, cx);
    tab_to(4, cx);
    for key in [
        "pagedown", "pagedown", "pagedown", "pagedown", "right", "enter",
    ] {
        press(key, cx);
    }
    assert_eq!(heard(&host, cx), ["year 2027", "open 2027-01-17"]);
}

/// A month view whose owner keeps the month and hands each turn back.
fn echoing(host: &Planning, owner: Entity<Planning>) -> gpui::AnyElement {
    let (days, months) = (owner.clone(), owner);
    CalendarMonthView::new("month", host.month, events())
        .zone(TimeZone::UTC)
        .today(date(2026, 9, 16))
        .on_day(move |day, _, cx| note(&days, format!("open {day}"), cx))
        .on_month(move |first, _, cx| {
            months.update(cx, |host, cx| {
                host.month = first;
                cx.notify();
            })
        })
        .into_any_element()
}

#[gpui::test]
fn the_owners_echo_of_a_turn_keeps_the_day(cx: &mut TestAppContext) {
    let (host, cx) = planning(echoing, cx);
    tab_to(4, cx);
    press("pagedown", cx);
    press("enter", cx);
    assert_eq!(heard(&host, cx), ["open 2026-10-16"]);
}

/// A year view whose owner keeps the year and hands each turn back.
fn echoing_year(host: &Planning, owner: Entity<Planning>) -> gpui::AnyElement {
    let (days, years) = (owner.clone(), owner);
    YearView::new("year", host.month.year(), events())
        .zone(TimeZone::UTC)
        .today(date(2026, 9, 16))
        .on_day(move |day, _, cx| note(&days, format!("open {day}"), cx))
        .on_year(move |year, _, cx| {
            years.update(cx, |host, cx| {
                host.month = date(year, 1, 1);
                cx.notify();
            })
        })
        .into_any_element()
}

#[gpui::test]
fn the_owners_echo_of_a_year_keeps_the_day(cx: &mut TestAppContext) {
    let (host, cx) = planning(echoing_year, cx);
    tab_to(4, cx);
    for key in ["pagedown", "pagedown", "pagedown", "pagedown", "enter"] {
        press(key, cx);
    }
    assert_eq!(heard(&host, cx), ["open 2027-01-16"]);
}

/// A month view at the last month the calendar shows.
fn last_month(_: &Planning, owner: Entity<Planning>) -> gpui::AnyElement {
    CalendarMonthView::new("month", date(9998, 12, 1), Vec::new())
        .zone(TimeZone::UTC)
        .today(date(9998, 12, 31))
        .on_day(move |day, _, cx| note(&owner, format!("open {day}"), cx))
        .into_any_element()
}

#[gpui::test]
fn the_calendar_ends_a_year_short_of_jiffs_last_day(cx: &mut TestAppContext) {
    let (host, cx) = planning(last_month, cx);
    tab_to(4, cx);
    for key in ["right", "pagedown", "down", "enter"] {
        press(key, cx);
    }
    assert_eq!(heard(&host, cx), ["open 9998-12-31"]);
}
