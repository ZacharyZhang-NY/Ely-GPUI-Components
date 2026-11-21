use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, IntoElement, KeyBinding,
    ParentElement, Render, SharedString, Styled, TestAppContext, VisualTestContext, Window, div,
    px,
};

use crate::{
    forms::{self, TextInput},
    primitives::FocusNext,
    theme::Theme,
};

mod flows;
mod help;

type Part = fn(&Bench, Entity<Bench>) -> AnyElement;

/// A view that shows one onboarding part, keeps what it heard, and holds what the owner decides: the step, readiness, what it hid, the open article and dialog, and a find field, with the root focus an app keeps.
struct Bench {
    part: Part,
    said: Vec<String>,
    root: FocusHandle,
    hidden: bool,
    step: usize,
    ready: bool,
    all_done: bool,
    open: Option<SharedString>,
    dialog: bool,
    busy: bool,
    search: Entity<TextInput>,
}

impl Render for Bench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(360.0))
            .p_4()
            .child((self.part)(self, cx.entity()))
    }
}

fn bench(part: Part, cx: &mut TestAppContext) -> (Entity<Bench>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|window, cx| Bench {
        part,
        said: Vec::new(),
        root: cx.focus_handle(),
        hidden: false,
        step: 0,
        ready: true,
        all_done: false,
        open: None,
        dialog: false,
        busy: false,
        search: cx.new(|cx| TextInput::new(window, cx)),
    });
    settle(cx);
    (host, cx)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn say(owner: &Entity<Bench>, words: String, cx: &mut App) {
    owner.update(cx, |bench, cx| {
        bench.said.push(words);
        cx.notify();
    });
}

fn said(host: &Entity<Bench>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |bench, _| bench.said.clone())
}

fn edit(host: &Entity<Bench>, cx: &mut VisualTestContext, change: impl FnOnce(&mut Bench)) {
    host.update(cx, |bench, cx| {
        change(bench);
        cx.notify();
    });
    settle(cx);
}

/// Focus on the `stops`th Tab stop from the top.
fn tab(stops: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        (0..stops).for_each(|_| window.focus_next(cx));
    });
    settle(cx);
}

fn tap(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse_with_os(key, cx.read(|app| app.platform_info().os))
            .expect("a key"),
    });
    settle(cx);
}

fn write(text: &str, cx: &mut VisualTestContext) {
    cx.simulate_input(text);
    settle(cx);
}
