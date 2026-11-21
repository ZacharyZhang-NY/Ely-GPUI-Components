use gpui::{
    Context, IntoElement, KeyBinding, KeyUpEvent, Keystroke, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, px,
};

use super::DiffViewer;

mod picking;
use crate::{forms, primitives::FocusNext, theme::Theme};

/// A diff whose first stretch folds, and folds asked open.
struct Folded {
    opened: Vec<usize>,
}

impl Render for Folded {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let old: String = (0..20).map(|ix| format!("line {ix}\n")).collect();
        let new = old.replace("line 18", "line eighteen");
        div().w(px(480.0)).h(px(320.0)).child(
            DiffViewer::new("diff", "a.rs", &old, &new)
                .headless()
                .on_open(move |ix, _, cx| view.update(cx, |folded, _| folded.opened.push(ix))),
        )
    }
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[gpui::test]
fn a_fold_opens_from_the_keyboard(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, _| Folded { opened: Vec::new() });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    cx.update(|window, cx| window.focus_next(cx));
    settle(cx);
    cx.simulate_keystrokes("enter");
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").expect("a key"),
    });
    settle(cx);
    assert_eq!(view.read_with(cx, |folded, _| folded.opened.clone()), [0]);
}

/// A blame older than its code: owners short and past.
struct Behind;

impl Render for Behind {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let blame = super::Blame {
            commit: "a1".into(),
            author: "Ann".into(),
            when: "today".into(),
            subject: "first".into(),
            age: 0.0,
        };
        div().w(px(600.0)).child(
            super::BlameView::new("blame", "one\ntwo\nthree\nfour", [0, 3, 0], [blame]).current(3),
        )
    }
}

#[gpui::test]
fn a_blame_that_lags_its_code_draws_every_line(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Behind);
    cx.run_until_parked();
}

/// Every section of a changes list, each with one file.
struct Sorted;

impl Render for Sorted {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        use crate::lists::GitStatus;
        let file = |path: &str, status| super::Changed {
            path: path.to_string().into(),
            status,
            lines: Some((1, 0)),
        };
        div().w(px(420.0)).child(
            super::ChangesList::new(
                "changes",
                [file("a.rs", GitStatus::Modified)],
                [file("b.rs", GitStatus::Modified)],
            )
            .conflicted([file("c.rs", GitStatus::Conflicted)])
            .untracked([file("d.rs", GitStatus::Untracked)])
            .ignored([super::Changed {
                lines: None,
                ..file("target/", GitStatus::Untracked)
            }]),
        )
    }
}

#[gpui::test]
fn every_section_draws_with_its_own_words(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Sorted);
    settle(cx);
}

/// Lines and hunks pressed, from stretches made elsewhere.
#[derive(Default)]
struct Picking {
    lines: Vec<((usize, usize), bool)>,
    hunks: Vec<usize>,
}

impl Render for Picking {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (lines, hunks) = (cx.entity(), cx.entity());
        let stretches = std::rc::Rc::new(super::diff("a\nb\n", "a\nc\n", 1));
        div().w(px(480.0)).h(px(320.0)).child(
            DiffViewer::from_stretches("picking", "a.txt", stretches)
                .selected([(0, 2)])
                .on_line(move |place, shift, _, cx| {
                    lines.update(cx, |picking, _| picking.lines.push((place, shift)))
                })
                .hunk_action("Stage", move |hunk, _, cx| {
                    hunks.update(cx, |picking, _| picking.hunks.push(hunk))
                }),
        )
    }
}

#[gpui::test]
fn changed_lines_and_hunks_answer_presses(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, _| Picking::default());
    cx.update(|window, _| window.activate_window());
    settle(cx);
    let press = |cx: &mut VisualTestContext, key: &'static str| {
        cx.simulate_keystrokes(key);
        settle(cx);
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
                .expect("a key"),
        });
        settle(cx);
    };
    // Past two layout segments: hunk button, then lines.
    let next = |cx: &mut VisualTestContext| {
        cx.update(|window, cx| window.focus_next(cx));
        settle(cx);
    };
    (0..3).for_each(|_| next(cx));
    press(cx, "enter");
    next(cx);
    press(cx, "enter");
    next(cx);
    press(cx, "enter");
    let (lines, hunks) = view.read_with(cx, |picking, _| {
        (picking.lines.clone(), picking.hunks.clone())
    });
    assert_eq!(hunks, [0], "the hunk's button");
    assert_eq!(
        lines,
        [((0, 1), false), ((0, 2), false)],
        "removed, then added"
    );
}

/// Rows pressed, with their sections.
#[derive(Default)]
struct Rows {
    pressed: Vec<(String, super::ChangeSection, super::ChangeAction)>,
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        use crate::lists::GitStatus;
        let view = cx.entity();
        let file = |path: &str| super::Changed {
            path: path.to_string().into(),
            status: GitStatus::Modified,
            lines: Some((1, 0)),
        };
        div().w(px(420.0)).child(
            super::ChangesList::new("rows", [file("a.rs")], [file("a.rs")])
                .selected(super::ChangeSection::Unstaged, "a.rs")
                .on_action(move |path, section, action, _, cx| {
                    view.update(cx, |rows, _| {
                        rows.pressed.push((path.to_string(), section, action))
                    })
                }),
        )
    }
}

#[gpui::test]
fn a_row_says_which_section_it_is_in(cx: &mut TestAppContext) {
    use super::{ChangeAction, ChangeSection};
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, _| Rows::default());
    cx.update(|window, _| window.activate_window());
    settle(cx);
    let mut rows = Vec::new();
    // Each row is a stop; icons hide until hover.
    for _ in 0..12 {
        cx.update(|window, cx| window.focus_next(cx));
        settle(cx);
        cx.simulate_keystrokes("enter");
        settle(cx);
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse("enter").expect("a key"),
        });
        settle(cx);
        rows = view.read_with(cx, |rows, _| rows.pressed.clone());
        if rows
            .iter()
            .any(|(_, section, _)| *section == ChangeSection::Unstaged)
        {
            break;
        }
    }
    assert!(
        rows.contains(&("a.rs".into(), ChangeSection::Staged, ChangeAction::Open)),
        "{rows:?}"
    );
    assert!(
        rows.contains(&("a.rs".into(), ChangeSection::Unstaged, ChangeAction::Open)),
        "{rows:?}"
    );
}
