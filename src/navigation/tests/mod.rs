mod pages;
mod stale;
mod tabs;

use gpui::{
    AppContext as _, Context, IntoElement, KeyUpEvent, Keystroke, Render, SharedString,
    TestAppContext, VisualTestContext, Window, px,
};

use super::{
    BackForwardNavigation, Breadcrumb, Crumb, GoToLine, LoadMore, NavigationMenu, Steps, Wizard,
};
use crate::{
    forms::{Choice, TextInput},
    layout::tests::narrow_width,
    theme::Theme,
};

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
    });
}

fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os)).unwrap(),
    });
}

struct Path {
    picked: Option<(usize, SharedString)>,
}

impl Render for Path {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        Breadcrumb::new(
            "path",
            [
                Crumb::new("home", "Home"),
                Crumb::new("docs", "Docs").siblings([
                    Choice::new("docs", "Docs"),
                    Choice::new("music", "Music"),
                    Choice::new("photos", "Photos"),
                ]),
                Crumb::new("notes", "notes.md"),
            ],
        )
        .on_select(move |level, value, _, cx| {
            let value = value.clone();
            view.update(cx, |view, _| view.picked = Some((level, value)));
        })
    }
}

#[gpui::test]
fn a_level_lists_its_siblings_and_picks_one(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Path { picked: None });
    cx.update(|window, cx| {
        window.focus_next(cx);
        window.focus_next(cx);
    });
    press("down", cx);
    press("down", cx);
    press("enter", cx);
    let picked = view.read_with(cx, |view, _| view.picked.clone());
    assert_eq!(picked, Some((1, SharedString::from("music"))));
}

struct Flow {
    step: usize,
    done: bool,
}

impl Render for Flow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (stepper, finisher) = (cx.entity(), cx.entity());
        let steps = [
            Choice::new("account", "Account"),
            Choice::new("plan", "Plan"),
            Choice::new("confirm", "Confirm"),
        ];
        Wizard::new(
            "flow",
            steps,
            self.step,
            move |to, _, cx| {
                stepper.update(cx, |view, cx| {
                    view.step = to;
                    cx.notify();
                })
            },
            move |_, cx| finisher.update(cx, |view, _| view.done = true),
        )
    }
}

#[gpui::test]
fn the_wizard_walks_forward_back_and_finishes(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Flow {
        step: 0,
        done: false,
    });
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.step), 1);
    cx.update(|window, cx| {
        window.blur(cx);
        window.focus_next(cx);
        window.focus_next(cx);
    });
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.step), 0);
    view.update(cx, |view, cx| {
        view.step = 2;
        cx.notify();
    });
    cx.update(|window, cx| {
        window.blur(cx);
        for _ in 0..4 {
            window.focus_next(cx);
        }
    });
    press("enter", cx);
    assert!(view.read_with(cx, |view, _| view.done));
}

/// On the last step the first stop is the first finished step; picking it hands focus to Next, so Enter walks on from there.
#[gpui::test]
fn a_picked_step_hands_focus_to_next(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Flow {
        step: 2,
        done: false,
    });
    cx.update(|window, cx| {
        window.blur(cx);
        window.focus_next(cx);
    });
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.step), 0);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.step), 1);
}

struct Site {
    entries: Vec<(&'static str, Vec<Choice>)>,
    went: Option<SharedString>,
}

impl Site {
    fn new() -> Self {
        let entries = vec![
            (
                "Product",
                vec![
                    Choice::new("tour", "Tour").disabled(),
                    Choice::new("price", "Pricing"),
                ],
            ),
            (
                "Docs",
                vec![Choice::new("guide", "Guide"), Choice::new("api", "API")],
            ),
        ];
        Self {
            entries,
            went: None,
        }
    }
}

impl Render for Site {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        self.entries
            .iter()
            .fold(NavigationMenu::new("site"), |menu, (label, links)| {
                menu.entry(*label, links.clone())
            })
            .on_select(move |value, _, cx| {
                let value = value.clone();
                view.update(cx, |view, _| view.went = Some(value));
            })
    }
}

#[gpui::test]
fn the_menu_opens_switches_and_picks_from_the_keyboard(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Site::new());
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("down right down enter");
    let went = view.read_with(cx, |view, _| view.went.clone());
    assert_eq!(went, Some(SharedString::from("api")));
}

#[gpui::test]
fn menu_keys_pass_over_disabled_links(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Site::new());
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("down enter");
    let went = view.read_with(cx, |view, _| view.went.clone());
    assert_eq!(went, Some(SharedString::from("price")));
    cx.simulate_keystrokes("down down enter");
    let went = view.read_with(cx, |view, _| view.went.clone());
    assert_eq!(went, Some(SharedString::from("price")));
}

#[gpui::test]
fn an_open_menu_follows_lists_that_shrink(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Site::new());
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_keystrokes("down right down");
    view.update(cx, |view, cx| {
        view.entries[1].1.pop();
        cx.notify();
    });
    cx.simulate_keystrokes("enter");
    let went = view.read_with(cx, |view, _| view.went.clone());
    assert_eq!(went, Some(SharedString::from("guide")));
    cx.simulate_keystrokes("down right");
    view.update(cx, |view, cx| {
        view.entries.pop();
        cx.notify();
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("enter");
    let went = view.read_with(cx, |view, _| view.went.clone());
    assert_eq!(went, Some(SharedString::from("guide")));
}

struct Trail {
    at: usize,
}

impl Render for Trail {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let places = ["home", "docs", "api"].map(|place| Choice::new(place, place));
        BackForwardNavigation::new("trail", places, self.at).on_go(move |to, _, cx| {
            view.update(cx, |view, cx| {
                view.at = to;
                cx.notify();
            })
        })
    }
}

#[gpui::test]
fn back_steps_once_and_the_list_jumps_anywhere(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| Trail { at: 2 });
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.at), 1);
    cx.update(|window, cx| {
        window.blur(cx);
        for _ in 0..3 {
            window.focus_next(cx);
        }
    });
    press("down", cx);
    press("up", cx);
    press("enter", cx);
    assert_eq!(view.read_with(cx, |view, _| view.at), 2);
}

struct Jump {
    field: gpui::Entity<TextInput>,
    to: Option<(usize, Option<usize>)>,
}

impl Render for Jump {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        GoToLine::new("jump", &self.field, 240).on_jump(move |line, column, _, cx| {
            view.update(cx, |view, _| view.to = Some((line, column)));
        })
    }
}

#[gpui::test]
fn enter_jumps_to_a_line_and_column(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|window, cx| Jump {
        field: cx.new(|cx| TextInput::new(window, cx)),
        to: None,
    });
    cx.update(|window, cx| window.focus_next(cx));
    cx.simulate_input("12:4");
    cx.simulate_keystrokes("enter");
    assert_eq!(view.read_with(cx, |view, _| view.to), Some((12, Some(4))));
}

/// A list's end that loads more, counting what it asked.
struct More {
    loading: bool,
    shown: usize,
    asks: usize,
}

impl Render for More {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        LoadMore::new("more", self.loading)
            .shown(self.shown, 30)
            .on_load(move |_, cx| view.update(cx, |more, _| more.asks += 1))
    }
}

#[gpui::test]
fn load_more_asks_once_per_press_and_goes_once_all_show(cx: &mut TestAppContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, _| More {
        loading: false,
        shown: 10,
        asks: 0,
    });
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    assert_eq!(view.read_with(cx, |more, _| more.asks), 1);
    view.update(cx, |more, cx| {
        more.loading = true;
        cx.notify();
    });
    press("enter", cx);
    assert_eq!(view.read_with(cx, |more, _| more.asks), 1, "busy");
    view.update(cx, |more, cx| {
        more.loading = false;
        more.shown = 30;
        cx.notify();
    });
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    assert_eq!(
        view.read_with(cx, |more, _| more.asks),
        1,
        "all shown, no button"
    );
}

#[gpui::test]
fn steps_fill_a_column_their_block_measures_by_content(cx: &mut TestAppContext) {
    setup(cx);
    let width = narrow_width(cx, "steps-root", |_, _| {
        Steps::new("steps", [Choice::new("a", "A"), Choice::new("b", "B")], 0).into_any_element()
    });
    assert_eq!(
        width,
        px(240.0),
        "the steps span the card inside its padding"
    );
}
