use std::ops::Range;

use gpui::{
    Context, FocusHandle, IntoElement, KeyBinding, KeyUpEvent, Keystroke, ParentElement, Render,
    SharedString, TestAppContext, VisualTestContext, Window, prelude::*,
};

use super::{Command, CommandPalette, Fit, QuickOpen, QuickSwitcher, SearchPalette, fuzzy};
use crate::{
    buttons::Button,
    forms::Choice,
    primitives::{FocusNext, FocusScope},
    theme::Theme,
};

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Presses and releases `key`, with a frame in between as on a real keyboard.
fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

#[test]
fn fuzzy_finds_letters_in_order_and_ranks_word_starts() {
    let found = fuzzy("tsb", "Toggle sidebar").expect("a fit");
    assert_eq!(found.hits, [0..1, 7..8, 11..12]);
    assert_eq!(
        fuzzy("", "anything"),
        Some(Fit {
            score: 0,
            hits: Vec::new()
        })
    );
    assert!(fuzzy("xyz", "Toggle").is_none());
    let score = |query, text| fuzzy(query, text).expect("a fit").score;
    assert!(score("os", "Open settings") > score("os", "Close tab"));
    assert!(score("set", "Settings") > score("set", "Reset"));
    assert_eq!(
        fuzzy("ÉL", "Élan").expect("a fit").hits,
        [Range { start: 0, end: 3 }]
    );
    assert_eq!(
        fuzzy("gtl", "GoToLine").expect("a fit").hits,
        [0..1, 2..3, 4..5]
    );
}

/// Opens one palette at a time and records what it handed back.
struct Host {
    root: FocusHandle,
    open: bool,
    got: Option<SharedString>,
    kind: Kind,
}

#[derive(Clone, Copy)]
enum Kind {
    Commands,
    Files,
    Switcher,
    Search,
    Disabled,
}

fn results(query: &str) -> Vec<(SharedString, Vec<Choice>)> {
    let all = [
        Choice::new("tokens", "Theme and tokens").note("Colors come from the theme"),
        Choice::new("motion", "Motion").note("Springs stay in animators"),
    ];
    let hits = all
        .into_iter()
        .filter(|found| query.is_empty() || found.label.to_lowercase().contains(query))
        .collect();
    vec![("Guides".into(), hits)]
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (view, close, opener) = (cx.entity(), cx.entity(), cx.entity());
        let close = move |_: &mut Window, cx: &mut gpui::App| {
            close.update(cx, |host, cx| {
                host.open = false;
                cx.notify();
            })
        };
        let got = move |value: &SharedString, _: &mut Window, cx: &mut gpui::App| {
            let value = value.clone();
            view.update(cx, |host, _| host.got = Some(value));
        };
        let palette = match self.kind {
            Kind::Commands => CommandPalette::new("commands", close)
                .group(
                    "View",
                    [
                        Command::new("sidebar", "Toggle sidebar").keys("secondary-b"),
                        Command::new("zoom", "Zoom in"),
                    ],
                )
                .group(
                    "Theme",
                    [
                        Command::new("dark", "Dark theme"),
                        Command::new("light", "Light theme"),
                    ],
                )
                .recent(["dark", "retired"])
                .on_run(got)
                .into_any_element(),
            Kind::Files => QuickOpen::new(
                "files",
                ["src/lib.rs", "src/navigation/menu.rs", "src/menus/mod.rs"],
                close,
            )
            .on_open(got)
            .into_any_element(),
            Kind::Switcher => QuickSwitcher::new(
                "switcher",
                ["now", "before", "older"].map(|place| Choice::new(place, place)),
                close,
            )
            .on_switch(got)
            .into_any_element(),
            Kind::Search => SearchPalette::new("search", results, close)
                .on_open(got)
                .into_any_element(),
            Kind::Disabled => SearchPalette::new(
                "search",
                |_| vec![("Guides".into(), vec![Choice::new("old", "Old").disabled()])],
                close,
            )
            .into_any_element(),
        };
        FocusScope::new(&self.root)
            .size_full()
            .child(Button::new("opener", "Open").on_click(move |_, _, cx| {
                opener.update(cx, |host, cx| {
                    host.open = true;
                    cx.notify();
                })
            }))
            .when(self.open, |host| host.child(palette))
    }
}

fn host(kind: Kind, cx: &mut TestAppContext) -> (gpui::Entity<Host>, &mut VisualTestContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(move |_, cx| Host {
        root: cx.focus_handle(),
        open: true,
        got: None,
        kind,
    });
    settle(cx);
    (view, cx)
}

fn got(view: &gpui::Entity<Host>, cx: &mut VisualTestContext) -> (bool, Option<SharedString>) {
    view.read_with(cx, |host, _| (host.open, host.got.clone()))
}

fn reopen(view: &gpui::Entity<Host>, cx: &mut VisualTestContext) {
    settle(cx);
    view.update(cx, |host, cx| {
        host.open = true;
        cx.notify();
    });
    settle(cx);
}

#[gpui::test]
fn the_command_palette_leads_with_recent_and_runs_the_best_fit(cx: &mut TestAppContext) {
    let (view, cx) = host(Kind::Commands, cx);
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("dark".into())));
    reopen(&view, cx);
    cx.simulate_input("zi");
    settle(cx);
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("zoom".into())));
    reopen(&view, cx);
    cx.simulate_keystrokes("up");
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("light".into())));
    reopen(&view, cx);
    cx.simulate_keystrokes("escape");
    assert_eq!(got(&view, cx), (false, Some("light".into())));
}

#[gpui::test]
fn quick_open_ranks_the_whole_path(cx: &mut TestAppContext) {
    let (view, cx) = host(Kind::Files, cx);
    cx.simulate_input("navmenu");
    settle(cx);
    press("enter", cx);
    assert_eq!(
        got(&view, cx),
        (false, Some("src/navigation/menu.rs".into()))
    );
}

#[gpui::test]
fn the_switcher_opens_on_the_one_before(cx: &mut TestAppContext) {
    let (view, cx) = host(Kind::Switcher, cx);
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("before".into())));
    reopen(&view, cx);
    cx.simulate_input("old");
    settle(cx);
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("older".into())));
}

#[gpui::test]
fn search_walks_the_owners_results(cx: &mut TestAppContext) {
    let (view, cx) = host(Kind::Search, cx);
    cx.simulate_keystrokes("down");
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("motion".into())));
    reopen(&view, cx);
    cx.simulate_input("theme");
    settle(cx);
    cx.simulate_keystrokes("down");
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("tokens".into())));
}

#[gpui::test]
fn tab_stays_inside_an_open_palette(cx: &mut TestAppContext) {
    let (view, cx) = host(Kind::Commands, cx);
    cx.simulate_keystrokes("tab");
    cx.simulate_input("zi");
    settle(cx);
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("zoom".into())));
}

#[gpui::test]
#[should_panic(expected = "disabled")]
fn search_refuses_disabled_results(cx: &mut TestAppContext) {
    host(Kind::Disabled, cx);
}

#[gpui::test]
fn enter_picks_on_release_so_the_opener_stays_shut(cx: &mut TestAppContext) {
    let (view, cx) = host(Kind::Commands, cx);
    press("escape", cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    assert_eq!(got(&view, cx), (true, None));
    cx.simulate_input("zi");
    settle(cx);
    press("enter", cx);
    assert_eq!(got(&view, cx), (false, Some("zoom".into())));
}

#[gpui::test]
fn an_opening_palette_rises_in_the_box_its_scrim_centers(cx: &mut TestAppContext) {
    let (_, cx) = host(Kind::Commands, cx);
    let top = |cx: &mut VisualTestContext| cx.debug_bounds("palette-card").expect("the card").top();
    let first = top(cx);
    cx.executor()
        .advance_clock(std::time::Duration::from_millis(250));
    settle(cx);
    let lift = first - top(cx);
    assert!(
        lift > gpui::px(0.0) && lift < crate::motion::NUDGE * 0.75,
        "the box grows with the rise, so the card starts half a nudge low: {lift:?}"
    );
}
