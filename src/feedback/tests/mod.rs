use std::time::Duration;

use gpui::{
    AppContext as _, Context, Entity, FocusHandle, IntoElement, KeyBinding, KeyUpEvent, Keystroke,
    Modifiers, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window, point, px,
};

mod loading;
mod states;

use super::{Alert, Notification, NotificationCenter, Toast, ToastViewport, Toaster};
use crate::{
    primitives::{FocusNext, FocusScope, IconName, Severity},
    theme::Theme,
};

/// A toast stack, an alert and a notification center, recording what they do.
struct Desk {
    root: FocusHandle,
    toaster: Entity<Toaster>,
    unread: bool,
    log: Vec<String>,
}

impl Desk {
    fn note(
        &self,
        cx: &mut Context<Self>,
        what: &'static str,
    ) -> impl Fn(&mut Window, &mut gpui::App) + 'static + use<> {
        let view = cx.entity();
        move |_, cx| view.update(cx, |desk, _| desk.log.push(what.into()))
    }
}

impl Render for Desk {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        FocusScope::new(&self.root)
            .root()
            .size_full()
            .child(ToastViewport::new("toasts", &self.toaster))
            .child(
                Alert::new("alert", Severity::Warning, "Heads up")
                    .on_dismiss(self.note(cx, "alert")),
            )
            .child(
                NotificationCenter::new("center")
                    .group(
                        "Today",
                        [Notification::new("n", IconName::Bell, "Hello").unread(self.unread)],
                    )
                    .on_read_all(self.note(cx, "read all"))
                    .on_clear(self.note(cx, "clear")),
            )
    }
}

/// The theme with reduced motion, text keys and Tab.
fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        Theme::update(cx, |theme| theme.reduced_motion = true);
        crate::forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
}

fn desk(cx: &mut TestAppContext) -> (Entity<Desk>, &mut VisualTestContext) {
    setup(cx);
    let (view, cx) = cx.add_window_view(|_, cx| Desk {
        root: cx.focus_handle(),
        toaster: cx.new(|_| Toaster::default()),
        unread: true,
        log: Vec::new(),
    });
    settle(cx);
    let root = view.read_with(cx, |desk, _| desk.root.clone());
    cx.update(|window, cx| window.focus(&root, cx));
    (view, cx)
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
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

/// Toasts paint last, so their buttons follow the alert's and the center's two in Tab order.
fn tab_to_toast(cx: &mut VisualTestContext) {
    for _ in 0..4 {
        press("tab", cx);
    }
}

fn wait(time: Duration, cx: &mut VisualTestContext) {
    cx.executor().advance_clock(time);
    settle(cx);
}

fn push(view: &Entity<Desk>, toast: Toast, cx: &mut VisualTestContext) -> u64 {
    let toaster = view.read_with(cx, |desk, _| desk.toaster.clone());
    let id = toaster.update(cx, |toaster, cx| toaster.push(toast, cx));
    settle(cx);
    id
}

/// Each toast's id, and whether it is on its way out.
fn toasts(view: &Entity<Desk>, cx: &mut VisualTestContext) -> Vec<(u64, bool)> {
    let toaster = view.read_with(cx, |desk, _| desk.toaster.clone());
    toaster.read_with(cx, |toaster, _| {
        toaster
            .live
            .iter()
            .map(|live| (live.id, live.leaving))
            .collect()
    })
}

fn log(view: &Entity<Desk>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |desk, _| desk.log.clone())
}

#[gpui::test]
fn a_toast_folds_away_after_its_time(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let id = push(&view, Toast::new("Saved"), cx);
    wait(Duration::from_millis(4900), cx);
    assert_eq!(toasts(&view, cx), [(id, false)]);
    wait(Duration::from_millis(200), cx);
    assert!(
        toasts(&view, cx).is_empty(),
        "reduced motion folds it at once"
    );
}

#[gpui::test]
fn the_owner_reads_what_is_shown(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let first = push(&view, Toast::new("Saved"), cx);
    push(&view, Toast::new("Denied").severity(Severity::Danger), cx);
    let toaster = view.read_with(cx, |desk, _| desk.toaster.clone());
    let shown = |cx: &mut VisualTestContext| toaster.read_with(cx, |toaster, _| toaster.shown());
    assert_eq!(
        shown(cx),
        [
            ("Saved".into(), None),
            ("Denied".into(), Some(Severity::Danger))
        ]
    );
    toaster.update(cx, |toaster, cx| toaster.dismiss(first, cx));
    assert_eq!(
        shown(cx),
        [("Denied".into(), Some(Severity::Danger))],
        "a leaving toast is not shown"
    );
}

#[gpui::test]
fn the_pointer_on_the_stack_holds_the_clock(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let id = push(&view, Toast::new("Saved"), cx);
    wait(Duration::from_secs(3), cx);
    let screen = cx.update(|window, _| window.viewport_size());
    let on_toast = point(screen.width - px(120.0), screen.height - px(48.0));
    cx.simulate_mouse_move(on_toast, None, Modifiers::none());
    settle(cx);
    wait(Duration::from_secs(20), cx);
    assert_eq!(toasts(&view, cx), [(id, false)], "held while pointed at");
    cx.simulate_mouse_move(point(px(4.0), px(4.0)), None, Modifiers::none());
    settle(cx);
    wait(Duration::from_millis(1900), cx);
    assert_eq!(toasts(&view, cx), [(id, false)], "two seconds were left");
    wait(Duration::from_millis(200), cx);
    assert!(toasts(&view, cx).is_empty());
}

#[gpui::test]
fn typing_under_a_resting_pointer_keeps_the_hold(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let id = push(&view, Toast::new("Saved"), cx);
    wait(Duration::from_millis(2), cx);
    let screen = cx.update(|window, _| window.viewport_size());
    let on_toast = point(screen.width - px(120.0), screen.height - px(48.0));
    cx.simulate_mouse_move(on_toast, None, Modifiers::none());
    settle(cx);
    press("a", cx);
    wait(Duration::from_secs(20), cx);
    assert_eq!(toasts(&view, cx), [(id, false)], "a key leaves the pointer");
}

#[gpui::test]
fn a_fifth_toast_sends_the_oldest_away(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let ids: Vec<u64> = (0..5).map(|_| push(&view, Toast::new("Hi"), cx)).collect();
    let leaving: Vec<u64> = toasts(&view, cx)
        .into_iter()
        .filter(|(_, out)| *out)
        .map(|(id, _)| id)
        .collect();
    assert_eq!(leaving, [ids[0]]);
}

#[gpui::test]
fn a_toast_without_a_time_stays(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let id = push(&view, Toast::new("Offline").stay(None), cx);
    wait(Duration::from_secs(120), cx);
    assert_eq!(toasts(&view, cx), [(id, false)]);
}

#[gpui::test]
fn undo_runs_then_sends_its_toast_away(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let undo = view.update(cx, |desk, cx| desk.note(cx, "undo"));
    let id = push(&view, Toast::new("Deleted").undo(undo), cx);
    tab_to_toast(cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["undo"]);
    assert_eq!(toasts(&view, cx), [(id, true)]);
}

#[gpui::test]
fn the_alert_closes_from_its_button(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    press("tab", cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["alert"]);
}

#[gpui::test]
fn mark_all_read_shows_only_with_something_unread(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    press("tab", cx);
    press("tab", cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["read all"]);
    view.update(cx, |desk, cx| {
        desk.unread = false;
        cx.notify();
    });
    settle(cx);
    press("tab", cx);
    press("tab", cx);
    press("enter", cx);
    assert_eq!(
        log(&view, cx),
        ["read all", "clear"],
        "the root took focus back, Clear is second"
    );
}

#[gpui::test]
fn a_toast_gone_under_focus_hands_it_to_the_root(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    let undo = view.update(cx, |desk, cx| desk.note(cx, "undo"));
    push(&view, Toast::new("Deleted").undo(undo), cx);
    tab_to_toast(cx);
    press("enter", cx);
    wait(Duration::from_millis(300), cx);
    assert!(toasts(&view, cx).is_empty());
    press("tab", cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["undo", "alert"]);
}

#[gpui::test]
fn a_leaving_toast_runs_its_undo_once(cx: &mut TestAppContext) {
    let (view, cx) = desk(cx);
    cx.update(|_, cx| Theme::update(cx, |theme| theme.reduced_motion = false));
    let undo = view.update(cx, |desk, cx| desk.note(cx, "undo"));
    push(&view, Toast::new("Deleted").undo(undo), cx);
    // Animations run on the wall clock; let the toast finish rising.
    cx.executor().advance_clock(Duration::from_millis(400));
    settle(cx);
    let screen = cx.update(|window, _| window.viewport_size());
    let on_undo = point(screen.width - px(78.0), screen.height - px(49.0));
    for _ in 0..2 {
        cx.simulate_mouse_move(on_undo, None, Modifiers::none());
        cx.simulate_click(on_undo, Modifiers::none());
        settle(cx);
    }
    assert_eq!(log(&view, cx), ["undo"]);
}
