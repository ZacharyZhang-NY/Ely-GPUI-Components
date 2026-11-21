use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    AnyElement, Context, FocusHandle, IntoElement, KeyUpEvent, Keystroke, ParentElement, Render,
    Styled, TestAppContext, VisualTestContext, Window, div, px,
};
use jiff::{Timestamp, tz::TimeZone};

use super::{Clock, Stopwatch, WorldClock};
use crate::{
    forms,
    primitives::{FocusNext, FocusPrev, FocusScope},
    theme::Theme,
};

mod asking;
mod codes;
mod drift;
mod notices;
mod numbers;
mod transfer;

struct Stage {
    root: FocusHandle,
    part: fn() -> AnyElement,
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.root)
            .root()
            .size_full()
            .child(div().w(px(400.0)).child((self.part)()))
    }
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

/// Stages `part`, counting the times the stage is told to redraw.
pub(super) fn stage(
    part: fn() -> AnyElement,
    cx: &mut TestAppContext,
) -> (Rc<Cell<usize>>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([
            gpui::KeyBinding::new("tab", FocusNext, None),
            gpui::KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
    });
    let (view, cx) = cx.add_window_view(|_, cx| Stage {
        root: cx.focus_handle(),
        part,
    });
    let root = view.read_with(cx, |stage, _| stage.root.clone());
    ROOT.with(|kept| kept.replace(Some(root.clone())));
    let wakes = Rc::new(Cell::new(0));
    let count = wakes.clone();
    cx.update(|window, cx| {
        window.focus(&root, cx);
        cx.observe(&view, move |_, _| count.set(count.get() + 1))
            .detach();
    });
    settle(cx);
    (wakes, cx)
}

thread_local! {
    static ROOT: std::cell::RefCell<Option<FocusHandle>> = const { std::cell::RefCell::new(None) };
}

/// Whether focus fell to the stage's root scope.
pub(super) fn at_root(cx: &mut VisualTestContext) -> bool {
    let root = ROOT.with(|root| root.borrow().clone().expect("a staged root"));
    cx.update(|window, _| root.is_focused(window))
}

pub(super) fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    let os = cx.read(|app| app.platform_info().os);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, os).expect("a key"),
    });
    settle(cx);
}

fn wait(ms: u64, cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(ms));
    settle(cx);
}

pub(super) fn shown(selector: &'static str, cx: &mut VisualTestContext) -> bool {
    cx.debug_bounds(selector).is_some()
}

fn watch() -> AnyElement {
    Stopwatch::new("watch").into_any_element()
}

/// Start runs the time on the executor's clock, Lap lists the laps newest first, Pause holds it, and Reset clears it and hands focus to Start.
#[gpui::test]
fn a_stopwatch_runs_laps_holds_and_resets(cx: &mut TestAppContext) {
    let (_, cx) = stage(watch, cx);
    press("tab", cx);
    press("enter", cx);
    wait(1_100, cx);
    assert!(shown("stopwatch-00:01.1", cx), "running");
    press("tab", cx);
    press("enter", cx);
    wait(1_200, cx);
    press("enter", cx);
    let row = |selector: &'static str, cx: &mut VisualTestContext| {
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("no row {selector}"))
    };
    assert!(
        row("lap-2-00:01.2-00:02.3", cx).origin.y < row("lap-1-00:01.1-00:01.1", cx).origin.y,
        "each lap's own time and total, the newest on top"
    );
    press("shift-tab", cx);
    press("enter", cx);
    wait(2_000, cx);
    assert!(
        shown("stopwatch-00:02.3", cx) && !shown("stopwatch-00:04.3", cx),
        "Pause, on the same button, holds it"
    );
    press("tab", cx);
    press("enter", cx);
    press("enter", cx);
    wait(300, cx);
    assert!(
        shown("stopwatch-00:00.3", cx),
        "Lap, now Reset, cleared it and handed focus to Start"
    );
}

/// The ticker wakes the stopwatch each tenth while it runs, and not at all once paused.
#[gpui::test]
fn a_running_stopwatch_wakes_each_tenth_and_rests_paused(cx: &mut TestAppContext) {
    let (wakes, cx) = stage(watch, cx);
    press("tab", cx);
    press("enter", cx);
    let started = wakes.get();
    for _ in 0..12 {
        wait(100, cx);
    }
    assert!(
        wakes.get() - started >= 12,
        "woke {} times",
        wakes.get() - started
    );
    press("enter", cx);
    let paused = wakes.get();
    for _ in 0..10 {
        wait(100, cx);
    }
    assert_eq!(wakes.get(), paused, "paused, it sleeps");
}

/// A clock on the wall clock wakes its view within each second.
#[gpui::test]
fn a_clock_ticks_each_second(cx: &mut TestAppContext) {
    let (wakes, cx) = stage(
        || Clock::new("clock").zone(TimeZone::UTC).into_any_element(),
        cx,
    );
    for second in 1..=2 {
        let before = wakes.get();
        wait(1_000, cx);
        assert!(wakes.get() > before, "no tick in second {second}");
    }
}

fn clocks() -> AnyElement {
    let now: Timestamp = "2026-09-28T03:00:00Z".parse().expect("a moment");
    let zone = |name: &str| TimeZone::get(name).expect("a zone");
    div()
        .child(
            Clock::new("clock")
                .zone(zone("Europe/Paris"))
                .now(now)
                .label("Paris"),
        )
        .child(
            WorldClock::new(
                "world",
                [
                    ("Tokyo", zone("Asia/Tokyo")),
                    ("Kolkata", zone("Asia/Kolkata")),
                ],
            )
            .home(zone("America/New_York"))
            .now(now),
        )
        .into_any_element()
}

/// Clocks shown at the owner's moment draw without a ticker.
#[gpui::test]
fn clocks_draw_at_the_owners_moment(cx: &mut TestAppContext) {
    let (wakes, cx) = stage(clocks, cx);
    let before = wakes.get();
    wait(5_000, cx);
    assert_eq!(wakes.get(), before, "nothing woke them");
}
