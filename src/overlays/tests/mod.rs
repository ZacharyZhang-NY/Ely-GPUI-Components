use std::{cell::Cell, rc::Rc, time::Duration};

use gpui::{
    AppContext as _, Bounds, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    KeyBinding, KeyUpEvent, Keystroke, Modifiers, ParentElement, Pixels, Render, ScrollDelta,
    ScrollEvent, ScrollSource, Styled, TestAppContext, TouchPhase, VisualTestContext, Window,
    canvas, div, point, px,
};

mod guides;

use super::{AlertDialog, ConfirmDialog, Dialog, HoverCard, Popover, PromptDialog};
use crate::{
    buttons::Button,
    forms::TextInput,
    primitives::{FocusNext, FocusScope, IconName, Severity},
    theme::{ActiveTheme, ControlSize, Theme},
};

#[derive(Clone, Copy, PartialEq)]
enum Open {
    Plain,
    Alert,
    Confirm,
    Prompt,
    Tall,
    Nested,
}

/// A popover, a hover card and one dialog at a time, recording what they do.
struct Stage {
    root: FocusHandle,
    before: FocusHandle,
    field: Entity<TextInput>,
    open: Option<Open>,
    log: Vec<String>,
    tall: Rc<Cell<Bounds<Pixels>>>,
}

impl Stage {
    fn note(
        &self,
        cx: &mut Context<Self>,
        what: &'static str,
    ) -> impl Fn(&mut Window, &mut gpui::App) + Clone + 'static {
        let view = cx.entity();
        move |_, cx| view.update(cx, |stage, _| stage.log.push(what.into()))
    }

    fn shut(
        &self,
        cx: &mut Context<Self>,
    ) -> impl Fn(&mut Window, &mut gpui::App) + Clone + 'static {
        let view = cx.entity();
        move |_, cx| {
            view.update(cx, |stage, cx| {
                stage.open = None;
                cx.notify();
            })
        }
    }
}

impl Render for Stage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (copy, mail, follow) = (
            self.note(cx, "copy"),
            self.note(cx, "mail"),
            self.note(cx, "follow"),
        );
        let popover = Popover::new("share", "Share", move |_, _| {
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(div().h(px(60.0)))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("copy", "Copy link")
                                .on_click(move |_, window, cx| copy(window, cx)),
                        )
                        .child(
                            Button::new("mail", "Email")
                                .on_click(move |_, window, cx| mail(window, cx)),
                        ),
                )
        });
        let beneath = self.note(cx, "beneath");
        let card = HoverCard::new(
            "ada",
            div().id("ada-name").w(px(120.0)).h(px(24.0)).child("Ada"),
            move |_, _| {
                let follow = follow.clone();
                Button::new("follow", "Follow").on_click(move |_, window, cx| follow(window, cx))
            },
        );
        let (shut, confirmed, done) = (
            self.shut(cx),
            self.note(cx, "confirmed"),
            self.note(cx, "done"),
        );
        let sent = cx.entity();
        let overlay = self.open.map(|open| match open {
            Open::Nested => Dialog::new("outer", "Outer", shut.clone())
                .child(Dialog::new("inner", "Inner", shut.clone()))
                .into_any_element(),
            Open::Plain => Dialog::new("plain", "Plain", shut)
                .detail("A line of detail")
                .into_any_element(),
            Open::Alert => AlertDialog::new("alert", Severity::Warning, "Alert", "Look", shut)
                .into_any_element(),
            Open::Confirm => ConfirmDialog::new("confirm", "Delete", "Gone for good", shut)
                .confirm("Delete")
                .destructive()
                .on_confirm(move |window, cx| confirmed(window, cx))
                .into_any_element(),
            Open::Prompt => PromptDialog::new("prompt", "Rename", &self.field, shut)
                .check(|text| match text {
                    "" => Err("Needs a name".into()),
                    text if text.contains('/') => Err("No slashes".into()),
                    _ => Ok(()),
                })
                .on_submit(move |text, _, cx| {
                    let text = format!("sent {text}");
                    sent.update(cx, |stage, _| stage.log.push(text));
                })
                .into_any_element(),
            Open::Tall => Dialog::new("tall", "Tall", shut)
                .child(div().relative().h(px(2000.0)).child({
                    let tall = self.tall.clone();
                    canvas(move |bounds, _, _| tall.set(bounds), |_, _, _, _| {})
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                }))
                .action(move |_| {
                    Button::new("tall-done", "Done").on_click(move |_, window, cx| done(window, cx))
                })
                .into_any_element(),
        });
        FocusScope::new(&self.root)
            .root()
            .size_full()
            .child(popover)
            .child(div().h(px(52.0)))
            .child(
                Button::new("beneath", "Beneath")
                    .on_click(move |_, window, cx| beneath(window, cx)),
            )
            .child(div().h(px(80.0)))
            .child(Button::new("elsewhere", "Elsewhere"))
            .child(div().h(px(12.0)))
            .child(card)
            .child(div().id("before").track_focus(&self.before))
            .children(overlay)
    }
}

pub(super) fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
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

pub(super) fn click(x: f32, y: f32, cx: &mut VisualTestContext) {
    let at = point(px(x), px(y));
    cx.simulate_mouse_move(at, None, Modifiers::none());
    cx.simulate_click(at, Modifiers::none());
    settle(cx);
}

fn stage(cx: &mut TestAppContext) -> (Entity<Stage>, &mut VisualTestContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
        cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
    });
    let (view, cx) = cx.add_window_view(|window, cx| Stage {
        root: cx.focus_handle(),
        before: cx.focus_handle(),
        field: cx.new(|cx| TextInput::new(window, cx)),
        open: None,
        log: Vec::new(),
        tall: Rc::new(Cell::new(Bounds::default())),
    });
    settle(cx);
    (view, cx)
}

fn log(view: &Entity<Stage>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |stage, _| stage.log.clone())
}

fn show(view: &Entity<Stage>, open: Open, cx: &mut VisualTestContext) {
    let before = view.read_with(cx, |stage, _| stage.before.clone());
    cx.update(|window, cx| window.focus(&before, cx));
    view.update(cx, |stage, cx| {
        stage.open = Some(open);
        cx.notify();
    });
    settle(cx);
}

#[gpui::test]
fn tab_walks_inside_an_open_popover(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    for _ in 0..3 {
        press("tab", cx);
    }
    press("enter", cx);
    assert_eq!(log(&view, cx), ["copy"]);
}

#[gpui::test]
fn escape_closes_the_popover_and_hands_focus_back(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    cx.update(|window, cx| window.focus_next(cx));
    press("enter", cx);
    press("escape", cx);
    press("enter", cx);
    press("tab", cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["copy"], "focus came back to the trigger");
}

#[gpui::test]
fn a_press_on_a_button_outside_closes_the_popover(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    click(12.0, 12.0, cx);
    click(20.0, 200.0, cx);
    click(40.0, 127.0, cx);
    assert!(log(&view, cx).is_empty());
}

#[gpui::test]
fn the_card_opens_after_a_rest_and_holds_while_on_it(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    cx.simulate_mouse_move(point(px(20.0), px(240.0)), None, Modifiers::none());
    settle(cx);
    cx.executor().advance_clock(Duration::from_millis(600));
    settle(cx);
    click(40.0, 290.0, cx);
    assert_eq!(log(&view, cx), ["follow"]);
}

#[gpui::test]
fn escape_closes_a_dialog_and_hands_focus_back(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Plain, cx);
    press("escape", cx);
    assert!(view.read_with(cx, |stage, _| stage.open.is_none()));
    let before = view.read_with(cx, |stage, _| stage.before.clone());
    assert!(cx.update(|window, _| before.is_focused(window)));
}

#[gpui::test]
fn a_dialog_its_owner_drops_hands_focus_back(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Plain, cx);
    assert!(!before_focused(&view, cx), "the dialog holds focus");
    view.update(cx, |stage, cx| {
        stage.open = None;
        cx.notify();
    });
    settle(cx);
    assert!(before_focused(&view, cx), "focus went back");
}

#[gpui::test]
fn nested_dialogs_dropped_together_hand_focus_back(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Nested, cx);
    assert!(!before_focused(&view, cx), "the dialogs hold focus");
    view.update(cx, |stage, cx| {
        stage.open = None;
        cx.notify();
    });
    settle(cx);
    assert!(before_focused(&view, cx), "focus went back past both");
}

#[gpui::test]
fn the_scrim_closes_a_dialog_but_not_an_alert(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Alert, cx);
    click(8.0, 8.0, cx);
    assert!(view.read_with(cx, |stage, _| stage.open == Some(Open::Alert)));
    view.update(cx, |stage, cx| {
        stage.open = None;
        cx.notify();
    });
    settle(cx);
    show(&view, Open::Plain, cx);
    click(8.0, 8.0, cx);
    assert!(view.read_with(cx, |stage, _| stage.open.is_none()));
}

#[gpui::test]
fn confirm_runs_from_the_keyboard(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Confirm, cx);
    press("tab", cx);
    press("tab", cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["confirmed"]);
    assert!(view.read_with(cx, |stage, _| stage.open.is_none()));
}

#[gpui::test]
fn the_prompt_sends_only_what_passes(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Prompt, cx);
    cx.simulate_input("a/b");
    settle(cx);
    press("enter", cx);
    assert!(log(&view, cx).is_empty(), "a slash went through");
    press("secondary-a", cx);
    cx.simulate_input("notes");
    settle(cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["sent notes"]);
}

#[gpui::test]
fn the_prompt_opens_with_its_text_selected(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    let field = view.read_with(cx, |stage, _| stage.field.clone());
    field.update(cx, |input, cx| input.set_text("Draft", cx));
    show(&view, Open::Prompt, cx);
    cx.simulate_input("notes");
    settle(cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["sent notes"]);
}

fn before_focused(view: &Entity<Stage>, cx: &mut VisualTestContext) -> bool {
    let before = view.read_with(cx, |stage, _| stage.before.clone());
    cx.update(|window, _| before.is_focused(window))
}

#[gpui::test]
fn every_dialog_button_hands_focus_back(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Alert, cx);
    press("tab", cx);
    press("enter", cx);
    assert!(before_focused(&view, cx), "alert button");
    show(&view, Open::Confirm, cx);
    press("tab", cx);
    press("enter", cx);
    assert!(before_focused(&view, cx), "confirm cancel");
    show(&view, Open::Confirm, cx);
    press("tab", cx);
    press("tab", cx);
    press("enter", cx);
    assert!(before_focused(&view, cx), "confirm confirm");
    show(&view, Open::Prompt, cx);
    cx.simulate_input("notes");
    settle(cx);
    press("enter", cx);
    assert!(before_focused(&view, cx), "prompt submit");
}

#[gpui::test]
fn a_press_on_the_panel_stays_off_what_lies_beneath(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    click(12.0, 12.0, cx);
    click(30.0, 94.0, cx);
    assert!(
        log(&view, cx).is_empty(),
        "the press went through the panel"
    );
    click(40.0, 127.0, cx);
    assert_eq!(log(&view, cx), ["copy"], "the panel stayed open");
}

#[gpui::test]
fn enter_on_a_prompt_button_runs_that_button_alone(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    show(&view, Open::Prompt, cx);
    cx.simulate_input("notes");
    settle(cx);
    press("tab", cx);
    press("enter", cx);
    assert!(log(&view, cx).is_empty(), "cancel submitted");
    show(&view, Open::Prompt, cx);
    cx.simulate_input("notes");
    settle(cx);
    press("tab", cx);
    press("tab", cx);
    press("enter", cx);
    assert_eq!(log(&view, cx), ["sent notes"]);
}

#[gpui::test]
fn a_tall_dialog_keeps_its_actions_in_the_window(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    cx.update(|_, cx| Theme::update(cx, |theme| theme.reduced_motion = true));
    show(&view, Open::Tall, cx);
    cx.executor().advance_clock(Duration::from_millis(2));
    settle(cx);
    let (size, margin, width, button) = cx.update(|window, cx| {
        let (theme, rem) = (cx.theme(), window.rem_size());
        (
            window.viewport_size(),
            theme.titlebar_height().to_pixels(rem),
            theme.dialog_width().to_pixels(rem),
            theme.control_height(ControlSize::Md).to_pixels(rem),
        )
    });
    let padding = px(24.0);
    let x = size.width / 2.0 + width / 2.0 - padding - px(12.0);
    let y = size.height - margin - padding - button / 2.0;
    click(f32::from(x), f32::from(y), cx);
    assert_eq!(
        log(&view, cx),
        ["done"],
        "the button sits at the card's foot, inside the window"
    );
}

#[gpui::test]
fn a_tall_dialog_body_keeps_its_height_and_scrolls(cx: &mut TestAppContext) {
    let (view, cx) = stage(cx);
    cx.update(|_, cx| Theme::update(cx, |theme| theme.reduced_motion = true));
    show(&view, Open::Tall, cx);
    cx.executor().advance_clock(Duration::from_millis(2));
    settle(cx);
    let tall = view.read_with(cx, |stage, _| stage.tall.clone());
    let before = tall.get();
    assert_eq!(before.size.height, px(2000.0), "the body keeps its height");
    let middle = cx
        .update(|window, _| window.viewport_size())
        .map(|side| side / 2.0);
    cx.simulate_event(ScrollEvent {
        position: point(middle.width, middle.height),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-300.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
    assert_eq!(
        tall.get().top(),
        before.top() - px(300.0),
        "a scroll moves the body"
    );
}

#[test]
#[should_panic(expected = "an icon is for its own button")]
fn a_popover_opened_by_its_owner_takes_no_icon() {
    let _ = Popover::with_opener("own", |_| div(), |_, _, _| div()).icon(IconName::Clock);
}

/// A dialog open from the first frame.
struct Rising;

impl Render for Rising {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            Dialog::new("rising", "Rising", |_, _| {})
                .child(div().debug_selector(|| "rising-body".into()).h(px(40.0))),
        )
    }
}

#[gpui::test]
fn an_opening_dialog_rises_in_the_box_its_scrim_centers(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Rising);
    cx.run_until_parked();
    let top = |cx: &mut VisualTestContext| cx.debug_bounds("rising-body").expect("the body").top();
    let first = top(cx);
    cx.executor().advance_clock(Duration::from_millis(250));
    settle(cx);
    let lift = first - top(cx);
    assert!(
        lift > px(0.0) && lift < crate::motion::NUDGE * 0.75,
        "the box grows with the rise, so the card starts half a nudge low: {lift:?}"
    );
}
