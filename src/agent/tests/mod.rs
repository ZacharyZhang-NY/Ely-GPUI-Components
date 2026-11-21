use gpui::{
    Context, Entity, IntoElement, KeyBinding, KeyUpEvent, Keystroke, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div,
};

mod changes;
mod control;
mod environment;
mod previews;

use super::{AgentProgress, Permission, PermissionPrompt, ToolApprovalDialog};
use crate::{forms, primitives::FocusNext, theme::Theme};

/// An approval dialog while open, and the answers it gave.
struct Asking {
    open: bool,
    answers: Vec<bool>,
}

impl Render for Asking {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        div().size_full().children(self.open.then(|| {
            ToolApprovalDialog::new(
                "approve",
                "write_file",
                r#"{ "path": "lift.rs" }"#,
                move |approved, _, cx| {
                    view.update(cx, |asking, cx| {
                        asking.answers.push(approved);
                        asking.open = false;
                        cx.notify();
                    })
                },
            )
        }))
    }
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

/// Presses and releases `key`, a frame apart.
pub(super) fn press(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    settle(cx);
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

fn asking(cx: &mut TestAppContext) -> (Entity<Asking>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (view, cx) = cx.add_window_view(|_, _| Asking {
        open: true,
        answers: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    (view, cx)
}

#[gpui::test]
fn escape_denies_the_call(cx: &mut TestAppContext) {
    let (view, cx) = asking(cx);
    press("escape", cx);
    assert_eq!(
        view.read_with(cx, |asking, _| asking.answers.clone()),
        [false]
    );
}

#[gpui::test]
fn run_approves_the_call_once(cx: &mut TestAppContext) {
    let (view, cx) = asking(cx);
    press("tab", cx);
    press("tab", cx);
    press("tab", cx);
    press("enter", cx);
    assert_eq!(
        view.read_with(cx, |asking, _| asking.answers.clone()),
        [true]
    );
}

/// A permission prompt and the answers it gave.
struct Prompting {
    answers: Vec<Permission>,
}

impl Render for Prompting {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        div().w_full().child(
            PermissionPrompt::new("permission", "Run commands here?", move |answer, _, cx| {
                view.update(cx, |prompting, _| prompting.answers.push(answer))
            })
            .detail("cargo test"),
        )
    }
}

#[gpui::test]
fn the_prompt_walks_deny_always_then_once(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        forms::bind_keys(cx);
    });
    let (view, cx) = cx.add_window_view(|_, _| Prompting {
        answers: Vec::new(),
    });
    cx.update(|window, _| window.activate_window());
    settle(cx);
    for _ in 0..3 {
        cx.update(|window, cx| window.focus_next(cx));
    }
    press("enter", cx);
    assert_eq!(
        view.read_with(cx, |prompting, _| prompting.answers.clone()),
        [Permission::Once]
    );
}

/// A progress bar for a task of one step, `done` or not; live counts may run past it.
struct Single {
    done: usize,
}

impl Render for Single {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w_full()
            .child(AgentProgress::new("single", "The one step", self.done, 1))
    }
}

#[gpui::test]
fn progress_past_its_total_fills_the_bar(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Single { done: 6 });
    settle(cx);
}

#[gpui::test]
fn a_task_of_one_step_draws_before_and_after(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (view, cx) = cx.add_window_view(|_, _| Single { done: 0 });
    settle(cx);
    view.update(cx, |single, cx| {
        single.done = 1;
        cx.notify();
    });
    settle(cx);
}
