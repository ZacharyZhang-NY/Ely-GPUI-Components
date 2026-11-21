use gpui::{
    AnyElement, App, Context, Entity, IntoElement, KeyBinding, ParentElement, Render, Styled,
    TestAppContext, VisualTestContext, Window, div, px,
};

use crate::{forms, primitives::FocusNext, theme::Theme};

pub(super) type Part = fn(&Bench, Entity<Bench>) -> AnyElement;

/// A view that shows one account part, keeps what it heard, and holds what the owner decides: whether a link went, and how many codes failed.
pub(super) struct Bench {
    part: Part,
    said: Vec<String>,
    pub(super) sent: bool,
    pub(super) failed: usize,
}

impl Render for Bench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(360.0)).child((self.part)(self, cx.entity()))
    }
}

pub(super) fn bench(
    part: Part,
    cx: &mut TestAppContext,
) -> (Entity<Bench>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (host, cx) = cx.add_window_view(|_, _| Bench {
        part,
        said: Vec::new(),
        sent: false,
        failed: 0,
    });
    settle(cx);
    (host, cx)
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

pub(super) fn say(owner: &Entity<Bench>, words: String, cx: &mut App) {
    owner.update(cx, |bench, cx| {
        bench.said.push(words);
        cx.notify();
    });
}

pub(super) fn said(host: &Entity<Bench>, cx: &mut VisualTestContext) -> Vec<String> {
    host.read_with(cx, |bench, _| bench.said.clone())
}

/// Focus on the `stops`th Tab stop from the top.
pub(super) fn tab(stops: usize, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.blur(cx);
        (0..stops).for_each(|_| window.focus_next(cx));
    });
    settle(cx);
}

pub(super) fn tap(key: &'static str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    let os = cx.read(|app| app.platform_info().os);
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse_with_os(key, os).expect("a key"),
    });
    settle(cx);
}

pub(super) fn write(text: &str, cx: &mut VisualTestContext) {
    cx.simulate_input(text);
    settle(cx);
}

mod access;
mod billing;
mod sign_in;
